use std::{
    any::{Any, TypeId},
    array,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use amity::flip_queue::FlipQueue;
use arcana_error::Error;
use arcana_id::Stid;
use futures::future::{AbortHandle, Abortable};
use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock};

use super::{
    AssetId,
    asset::Asset,
    build::AssetBuilder,
    loader::{AssetData, Loader},
};
use crate::{AssetError, NotFound};

const ASSETS_ARRAY_SIZE: usize = 97;

fn assets_array_index(id: AssetId) -> usize {
    (id.get().wrapping_mul(0x9E3779B9) as usize) % ASSETS_ARRAY_SIZE
}

/// Shared typed asset cache. Requests require an entered Tokio runtime.
#[derive(Clone)]
pub struct Assets {
    inner: Arc<AssetsInner>,
}

struct AssetsInner {
    loaders: Box<[Arc<dyn Loader>]>,
    types: RwLock<HashMap<Stid, TypedAssetsArray>>,
    to_build: FlipQueue<BuildRequest>,
}

struct BuildRequest {
    typed: Arc<dyn AnyTypedAssets>,
    id: AssetId,
    generation: u64,
}

struct TypedAssetsArray {
    version: u64,
    type_id: TypeId,
    array: [Arc<dyn AnyTypedAssets>; ASSETS_ARRAY_SIZE],
}

impl Assets {
    pub fn new(loaders: impl IntoIterator<Item = Arc<dyn Loader>>) -> Self {
        Assets {
            inner: Arc::new(AssetsInner {
                loaders: loaders.into_iter().collect(),
                types: RwLock::new(HashMap::new()),
                to_build: FlipQueue::new(),
            }),
        }
    }

    /// Requests a typed asset; pending requests progress through async loading and building.
    pub fn get<A: Asset>(&self, id: AssetId) -> Poll<Result<A, Error>> {
        self.typed_entry::<A>(id)
            .poll_asset(id, self, None)
            .map_err(Error::from)
    }

    /// Requests a typed asset and registers a waiter for completion or invalidation.
    pub fn poll<A: Asset>(&self, id: AssetId, cx: &mut Context) -> Poll<Result<A, Error>> {
        self.typed_entry::<A>(id)
            .poll_asset(id, self, Some(cx))
            .map_err(Error::from)
    }

    /// Invalidates cached requests across every clone. Previously returned assets remain owned
    /// by their callers. Loading tasks are aborted and pending waiters are notified.
    pub fn invalidate_all(&self) {
        self.remove_types(|_, _| true);
    }

    /// Retains only the listed asset types with matching versions.
    #[doc(hidden)]
    pub fn drop_all_except(&self, keep: &HashMap<Stid, u64>) {
        self.remove_types(|stid, typed| keep.get(stid) != Some(&typed.version));
    }

    fn remove_types(&self, remove: impl Fn(&Stid, &TypedAssetsArray) -> bool) {
        let mut types = self.inner.types.write();
        let keys: Vec<_> = types
            .iter()
            .filter(|(id, typed)| remove(id, typed))
            .map(|(id, _)| *id)
            .collect();
        let removed: Vec<_> = keys
            .into_iter()
            .map(|id| types.remove(&id).unwrap())
            .collect();
        // Retirement extracts payloads without running plugin destructors or waking callers.
        let cleanup: Vec<_> = removed
            .iter()
            .flat_map(|typed| typed.array.iter().map(|bucket| bucket.retire()))
            .collect();
        let mut queued = Vec::new();
        self.inner
            .to_build
            .drain_locking(|batch| queued.extend(batch));
        let mut discarded = Vec::new();
        for request in queued {
            if request.typed.is_retired() {
                discarded.push(request);
            } else {
                self.inner.to_build.push_sync(request);
            }
        }
        drop(types);
        for release in cleanup {
            release();
        }
        drop(discarded);
        drop(removed);
    }

    pub fn build_assets(&self, builder: &mut AssetBuilder) {
        let mut requests = Vec::new();
        self.inner
            .to_build
            .drain_locking(|batch| requests.extend(batch));
        for request in requests {
            request
                .typed
                .build_asset(request.id, request.generation, builder);
        }
    }

    fn typed_entry<A: Asset>(&self, id: AssetId) -> Arc<TypedAssets<A>> {
        let index = assets_array_index(id);
        let stid = Stid::of::<A>();
        let version = A::version();
        let types = self.inner.types.read();
        if let Some(typed) = types.get(&stid) {
            return typed.checked_bucket::<A>(index, version);
        }
        drop(types);
        let mut types = self.inner.types.write();
        let typed = types.entry(stid).or_insert_with(|| TypedAssetsArray {
            version,
            type_id: TypeId::of::<A>(),
            array: array::from_fn(|_| {
                Arc::new(TypedAssets::<A> {
                    cache: Mutex::new(TypedCache {
                        retired: false,
                        next_generation: 0,
                        states: HashMap::new(),
                    }),
                }) as Arc<dyn AnyTypedAssets>
            }),
        });
        typed.checked_bucket::<A>(index, version)
    }
}

impl TypedAssetsArray {
    fn checked_bucket<A: Asset>(&self, index: usize, version: u64) -> Arc<TypedAssets<A>> {
        assert_eq!(
            self.version, version,
            "asset version mismatch; invalidate the cache before replacing asset types"
        );
        assert_eq!(
            self.type_id,
            TypeId::of::<A>(),
            "stable asset ID belongs to a different concrete type"
        );
        let bucket = self.array[index].clone();
        assert_eq!((*bucket).type_id(), TypeId::of::<TypedAssets<A>>());
        unsafe { Arc::from_raw(Arc::into_raw(bucket) as *const TypedAssets<A>) }
    }
}

trait AnyTypedAssets: Any + Send + Sync {
    fn build_asset(&self, id: AssetId, generation: u64, builder: &mut AssetBuilder);
    fn retire(&self) -> Box<dyn FnOnce() + Send>;
    fn is_retired(&self) -> bool;
}

enum AssetState<A: Asset> {
    Loading {
        generation: u64,
        abort: AbortHandle,
        wakers: Vec<Waker>,
    },
    Loaded {
        generation: u64,
        asset: A::Loaded,
        wakers: Vec<Waker>,
    },
    Building {
        generation: u64,
        wakers: Vec<Waker>,
    },
    Error {
        error: AssetError,
    },
    Ready {
        asset: Arc<A>,
    },
}

struct TypedCache<A: Asset> {
    retired: bool,
    next_generation: u64,
    states: HashMap<AssetId, AssetState<A>>,
}

struct TypedAssets<A: Asset> {
    cache: Mutex<TypedCache<A>>,
}

fn wake_all(wakers: Vec<Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

impl<A: Asset> AnyTypedAssets for TypedAssets<A> {
    fn build_asset(&self, id: AssetId, generation: u64, builder: &mut AssetBuilder) {
        self.build_with(id, generation, |asset| A::build(asset, builder));
    }

    fn retire(&self) -> Box<dyn FnOnce() + Send> {
        let mut cache = self.cache.lock();
        cache.retired = true;
        let states = std::mem::take(&mut cache.states);
        drop(cache);
        Box::new(move || {
            for (_, state) in states {
                match state {
                    AssetState::Loading { abort, wakers, .. } => {
                        abort.abort();
                        wake_all(wakers);
                    }
                    AssetState::Loaded { asset, wakers, .. } => {
                        drop(asset);
                        wake_all(wakers);
                    }
                    AssetState::Building { wakers, .. } => wake_all(wakers),
                    state => drop(state),
                }
            }
        })
    }

    fn is_retired(&self) -> bool {
        self.cache.lock().retired
    }
}

impl<A: Asset> TypedAssets<A> {
    fn build_with(
        &self,
        id: AssetId,
        generation: u64,
        build: impl FnOnce(A::Loaded) -> Result<A, AssetError>,
    ) {
        let mut cache = self.cache.lock();
        if cache.retired
            || !matches!(cache.states.get(&id), Some(AssetState::Loaded { generation: current, .. }) if *current == generation)
        {
            return;
        }
        let Some(AssetState::Loaded { asset, wakers, .. }) = cache.states.remove(&id) else {
            unreachable!()
        };
        cache
            .states
            .insert(id, AssetState::Building { generation, wakers });
        drop(cache);
        let result = build(asset);
        let mut cache = self.cache.lock();
        if cache.retired
            || !matches!(cache.states.get(&id), Some(AssetState::Building { generation: current, .. }) if *current == generation)
        {
            drop(cache);
            drop(result);
            return;
        }
        let Some(AssetState::Building { wakers, .. }) = cache.states.remove(&id) else {
            unreachable!()
        };
        cache.states.insert(
            id,
            match result {
                Ok(asset) => AssetState::Ready {
                    asset: Arc::new(asset),
                },
                Err(error) => AssetState::Error { error },
            },
        );
        drop(cache);
        wake_all(wakers);
    }

    fn poll_asset(
        self: &Arc<Self>,
        id: AssetId,
        assets: &Assets,
        cx: Option<&mut Context>,
    ) -> Poll<Result<A, AssetError>> {
        let mut cache = self.cache.lock();
        if cache.retired {
            drop(cache);
            if let Some(cx) = cx {
                cx.waker().wake_by_ref();
            }
            return Poll::Pending;
        }
        if let Some(state) = cache.states.get_mut(&id) {
            return match state {
                AssetState::Loading { wakers, .. }
                | AssetState::Loaded { wakers, .. }
                | AssetState::Building { wakers, .. } => {
                    if let Some(cx) = cx {
                        if !wakers.iter().any(|w| w.will_wake(cx.waker())) {
                            wakers.push(cx.waker().clone());
                        }
                    }
                    Poll::Pending
                }
                AssetState::Ready { asset } => {
                    let asset = asset.clone();
                    drop(cache);
                    Poll::Ready(Ok((*asset).clone()))
                }
                AssetState::Error { error } => Poll::Ready(Err(error.clone())),
            };
        }
        let runtime = match tokio::runtime::Handle::try_current() {
            Ok(runtime) => runtime,
            Err(error) => return Poll::Ready(Err(AssetError::new(error))),
        };
        let generation = cache.next_generation;
        cache.next_generation = generation
            .checked_add(1)
            .expect("asset request generation exhausted");
        let (abort, registration) = AbortHandle::new_pair();
        let wakers = cx.into_iter().map(|cx| cx.waker().clone()).collect();
        cache.states.insert(
            id,
            AssetState::Loading {
                generation,
                abort,
                wakers,
            },
        );
        drop(cache);
        let me = Arc::clone(self);
        let assets = assets.clone();
        runtime.spawn(Abortable::new(async move {
            let result = match load_from_any(&assets.inner.loaders, id).await {
                Ok(data) => A::load(data.bytes, &assets).await,
                Err(error) => Err(error),
            };
            let mut cache = me.cache.lock();
            if cache.retired || !matches!(cache.states.get(&id), Some(AssetState::Loading { generation: current, .. }) if *current == generation) {
                drop(cache);
                drop(result);
                return;
            }
            let Some(AssetState::Loading { wakers, abort, .. }) = cache.states.remove(&id) else { unreachable!() };
            let waiters = match result {
                Ok(asset) => {
                    cache.states.insert(id, AssetState::Loaded { generation, asset, wakers });
                    assets.inner.to_build.push_sync(BuildRequest { typed: me.clone(), id, generation });
                    Vec::new()
                }
                Err(error) => {
                    cache.states.insert(id, AssetState::Error { error });
                    wakers
                }
            };
            drop(cache);
            drop(abort);
            wake_all(waiters);
        }, registration));
        Poll::Pending
    }
}

async fn load_from_any(loaders: &[Arc<dyn Loader>], id: AssetId) -> Result<AssetData, AssetError> {
    let mut not_found_error = None;
    for loader in loaders {
        match loader.load(id).await {
            Err(error) if error.is::<NotFound>() => not_found_error = Some(error),
            Err(error) => return Err(error),
            Ok(data) => return Ok(data),
        }
    }
    Err(not_found_error.unwrap_or_else(|| AssetError::new(NotFound)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AssetVersion;
    use arcana_id::HasStid;
    use futures::{
        future::BoxFuture,
        task::{ArcWake, waker},
    };
    use std::{
        num::NonZeroU64,
        sync::atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Debug)]
    struct TestAsset;
    impl HasStid for TestAsset {
        fn stid() -> Stid {
            Stid::new(NonZeroU64::new(7).unwrap())
        }
        fn stid_dyn(&self) -> Stid {
            Self::stid()
        }
    }
    impl AssetVersion for TestAsset {
        fn version() -> u64 {
            1
        }
    }
    impl Asset for TestAsset {
        type Loaded = Self;
        async fn load(_: Box<[u8]>, _: &Assets) -> Result<Self, AssetError> {
            Ok(Self)
        }
        fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
            Ok(loaded)
        }
    }
    #[derive(Clone)]
    struct CollidingAsset;
    impl HasStid for CollidingAsset {
        fn stid() -> Stid {
            TestAsset::stid()
        }
        fn stid_dyn(&self) -> Stid {
            Self::stid()
        }
    }
    impl AssetVersion for CollidingAsset {
        fn version() -> u64 {
            1
        }
    }
    impl Asset for CollidingAsset {
        type Loaded = Self;
        async fn load(_: Box<[u8]>, _: &Assets) -> Result<Self, AssetError> {
            Ok(Self)
        }
        fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
            Ok(loaded)
        }
    }
    struct WakeCount(AtomicUsize);
    impl ArcWake for WakeCount {
        fn wake_by_ref(me: &Arc<Self>) {
            me.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn id() -> AssetId {
        AssetId::new(NonZeroU64::new(1).unwrap())
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }
    fn pending(assets: &Assets, count: &Arc<WakeCount>) {
        let w = waker(count.clone());
        assert!(
            assets
                .poll::<TestAsset>(id(), &mut Context::from_waker(&w))
                .is_pending()
        );
    }
    struct ImmediateLoader;
    impl Loader for ImmediateLoader {
        fn load(&self, _: AssetId) -> BoxFuture<'_, Result<AssetData, AssetError>> {
            Box::pin(async {
                Ok(AssetData {
                    bytes: Box::new([]),
                    version: 0,
                })
            })
        }
        fn update(
            &self,
            _: AssetId,
            _: u64,
        ) -> BoxFuture<'_, Result<Option<AssetData>, AssetError>> {
            Box::pin(async { Ok(None) })
        }
    }
    struct PendingLoader;
    impl Loader for PendingLoader {
        fn load(&self, _: AssetId) -> BoxFuture<'_, Result<AssetData, AssetError>> {
            Box::pin(futures::future::pending())
        }
        fn update(
            &self,
            _: AssetId,
            _: u64,
        ) -> BoxFuture<'_, Result<Option<AssetData>, AssetError>> {
            Box::pin(async { Ok(None) })
        }
    }

    #[test]
    fn missing_runtime_does_not_leave_loading_entry() {
        let assets = Assets::new([]);
        assert!(matches!(assets.get::<TestAsset>(id()), Poll::Ready(Err(_))));
        assert!(
            assets
                .typed_entry::<TestAsset>(id())
                .cache
                .lock()
                .states
                .is_empty()
        );
        let rt = runtime();
        let _entered = rt.enter();
        assert!(assets.get::<TestAsset>(id()).is_pending());
    }

    #[test]
    fn loading_failure_wakes_waiter() {
        let assets = Assets::new([]);
        let count = Arc::new(WakeCount(AtomicUsize::new(0)));
        runtime().block_on(async {
            pending(&assets, &count);
            tokio::task::yield_now().await;
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(matches!(assets.get::<TestAsset>(id()), Poll::Ready(Err(_))));
        });
    }

    #[test]
    fn invalidation_retires_pending_bucket_and_updates_clones() {
        let assets = Assets::new([Arc::new(PendingLoader) as Arc<dyn Loader>]);
        let clone = assets.clone();
        let count = Arc::new(WakeCount(AtomicUsize::new(0)));
        runtime().block_on(async {
            pending(&assets, &count);
            let old = assets.typed_entry::<TestAsset>(id());
            let aborted = match old.cache.lock().states.get(&id()).unwrap() {
                AssetState::Loading { abort, .. } => abort.clone(),
                _ => panic!(),
            };
            assets.invalidate_all();
            assert!(aborted.is_aborted());
            assert!(old.cache.lock().retired);
            assert!(old.cache.lock().states.is_empty());
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            let fresh = clone.typed_entry::<TestAsset>(id());
            assert!(!Arc::ptr_eq(&old, &fresh));
            assert!(clone.get::<TestAsset>(id()).is_pending());
            assert!(old.cache.lock().states.is_empty());
        });
    }

    #[test]
    fn invalidate_loaded_removes_build_requests_and_wakes_waiter() {
        let assets = Assets::new([Arc::new(ImmediateLoader) as Arc<dyn Loader>]);
        let count = Arc::new(WakeCount(AtomicUsize::new(0)));
        runtime().block_on(async {
            pending(&assets, &count);
            tokio::task::yield_now().await;
            assert!(matches!(
                assets
                    .typed_entry::<TestAsset>(id())
                    .cache
                    .lock()
                    .states
                    .get(&id()),
                Some(AssetState::Loaded { .. })
            ));
            assets.invalidate_all();
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            let mut requests = Vec::new();
            assets
                .inner
                .to_build
                .drain_locking(|batch| requests.extend(batch));
            assert!(requests.is_empty());
        });
    }

    #[test]
    fn build_can_reenter_and_stale_request_preserves_state() {
        let assets = Assets::new([Arc::new(ImmediateLoader) as Arc<dyn Loader>]);
        runtime().block_on(async {
            assert!(assets.get::<TestAsset>(id()).is_pending());
            tokio::task::yield_now().await;
            let typed = assets.typed_entry::<TestAsset>(id());
            typed.build_with(id(), 99, |_| panic!("stale request built"));
            assert!(matches!(
                typed.cache.lock().states.get(&id()),
                Some(AssetState::Loaded { .. })
            ));
            typed.build_with(id(), 0, |asset| {
                assert!(assets.get::<TestAsset>(id()).is_pending());
                Ok(asset)
            });
            typed.build_with(id(), 0, |_| panic!("duplicate request built"));
            assert!(matches!(assets.get::<TestAsset>(id()), Poll::Ready(Ok(_))));
        });
    }

    #[test]
    fn invalidation_during_build_discards_result() {
        let assets = Assets::new([Arc::new(ImmediateLoader) as Arc<dyn Loader>]);
        let count = Arc::new(WakeCount(AtomicUsize::new(0)));
        runtime().block_on(async {
            pending(&assets, &count);
            tokio::task::yield_now().await;
            let typed = assets.typed_entry::<TestAsset>(id());
            typed.build_with(id(), 0, |asset| {
                assets.invalidate_all();
                Ok(asset)
            });
            assert!(typed.cache.lock().states.is_empty());
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(assets.get::<TestAsset>(id()).is_pending());
        });
    }

    #[test]
    fn retention_keeps_matching_types_and_retires_other_versions() {
        let assets = Assets::new([]);
        let old = assets.typed_entry::<TestAsset>(id());
        assets.drop_all_except(&HashMap::from([(TestAsset::stid(), 1)]));
        assert!(Arc::ptr_eq(&old, &assets.typed_entry::<TestAsset>(id())));
        assets.drop_all_except(&HashMap::from([(TestAsset::stid(), 2)]));
        assert!(old.is_retired());
        assert!(!Arc::ptr_eq(&old, &assets.typed_entry::<TestAsset>(id())));
    }

    #[test]
    #[should_panic(expected = "different concrete type")]
    fn stable_id_collision_is_rejected() {
        let assets = Assets::new([]);
        assets.typed_entry::<TestAsset>(id());
        assets.typed_entry::<CollidingAsset>(id());
    }

    #[test]
    #[should_panic(expected = "asset version mismatch")]
    fn fast_path_checks_version() {
        let assets = Assets::new([]);
        assets.typed_entry::<TestAsset>(id());
        assets
            .inner
            .types
            .write()
            .get_mut(&TestAsset::stid())
            .unwrap()
            .version = 2;
        assets.typed_entry::<TestAsset>(id());
    }

    struct ReentrantWake {
        assets: Assets,
        count: AtomicUsize,
    }
    impl ArcWake for ReentrantWake {
        fn wake_by_ref(me: &Arc<Self>) {
            me.count.fetch_add(1, Ordering::SeqCst);
            let _ = me.assets.get::<TestAsset>(id());
        }
    }

    #[test]
    fn invalidation_waiter_can_request_fresh_asset() {
        let assets = Assets::new([Arc::new(PendingLoader) as Arc<dyn Loader>]);
        let waiter = Arc::new(ReentrantWake {
            assets: assets.clone(),
            count: AtomicUsize::new(0),
        });
        runtime().block_on(async {
            let w = waker(waiter.clone());
            assert!(
                assets
                    .poll::<TestAsset>(id(), &mut Context::from_waker(&w))
                    .is_pending()
            );
            assets.invalidate_all();
            assert_eq!(waiter.count.load(Ordering::SeqCst), 1);
            assert!(matches!(
                assets
                    .typed_entry::<TestAsset>(id())
                    .cache
                    .lock()
                    .states
                    .get(&id()),
                Some(AssetState::Loading { .. })
            ));
        });
    }

    #[derive(Clone)]
    struct DecodeFailure;
    impl HasStid for DecodeFailure {
        fn stid() -> Stid {
            Stid::new(NonZeroU64::new(8).unwrap())
        }
        fn stid_dyn(&self) -> Stid {
            Self::stid()
        }
    }
    impl AssetVersion for DecodeFailure {
        fn version() -> u64 {
            1
        }
    }
    impl Asset for DecodeFailure {
        type Loaded = Self;
        async fn load(_: Box<[u8]>, _: &Assets) -> Result<Self, AssetError> {
            Err(AssetError::new(NotFound))
        }
        fn build(_: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
            unreachable!()
        }
    }

    #[test]
    fn decode_failure_wakes_waiter() {
        let assets = Assets::new([Arc::new(ImmediateLoader) as Arc<dyn Loader>]);
        let count = Arc::new(WakeCount(AtomicUsize::new(0)));
        runtime().block_on(async {
            let w = waker(count.clone());
            assert!(
                assets
                    .poll::<DecodeFailure>(id(), &mut Context::from_waker(&w))
                    .is_pending()
            );
            tokio::task::yield_now().await;
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(matches!(
                assets.get::<DecodeFailure>(id()),
                Poll::Ready(Err(_))
            ));
        });
    }

    struct ReentrantClone {
        assets: Assets,
    }
    impl Clone for ReentrantClone {
        fn clone(&self) -> Self {
            let typed = self.assets.typed_entry::<Self>(id());
            assert!(
                typed.cache.try_lock().is_some(),
                "asset clone holds cache lock"
            );
            Self {
                assets: self.assets.clone(),
            }
        }
    }
    impl HasStid for ReentrantClone {
        fn stid() -> Stid {
            Stid::new(NonZeroU64::new(9).unwrap())
        }
        fn stid_dyn(&self) -> Stid {
            Self::stid()
        }
    }
    impl AssetVersion for ReentrantClone {
        fn version() -> u64 {
            1
        }
    }
    impl Asset for ReentrantClone {
        type Loaded = Self;
        async fn load(_: Box<[u8]>, assets: &Assets) -> Result<Self, AssetError> {
            Ok(Self {
                assets: assets.clone(),
            })
        }
        fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
            Ok(loaded)
        }
    }

    #[test]
    fn ready_clone_runs_without_cache_lock() {
        let assets = Assets::new([Arc::new(ImmediateLoader) as Arc<dyn Loader>]);
        runtime().block_on(async {
            assert!(assets.get::<ReentrantClone>(id()).is_pending());
            tokio::task::yield_now().await;
            assets
                .typed_entry::<ReentrantClone>(id())
                .build_with(id(), 0, Ok);
            assert!(matches!(
                assets.get::<ReentrantClone>(id()),
                Poll::Ready(Ok(_))
            ));
            assets.invalidate_all();
        });
    }
}
