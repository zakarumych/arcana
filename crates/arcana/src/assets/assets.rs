use std::{
    any::{Any, TypeId},
    array,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use amity::flip_queue::FlipQueue;
use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock};

use crate::type_id;

use super::{
    asset::Asset,
    build::AssetBuilder,
    error::{Error, NotFound},
    id::AssetId,
    loader::{AssetData, Loader},
};

const ASSETS_ARRAY_SIZE: usize = 32;

fn assets_array_index(id: AssetId) -> usize {
    let v = id.value().get();

    // Simple hash function to distribute assets across multiple locks.
    let v = v.wrapping_mul(0x9E3779B9);

    (v as usize) % ASSETS_ARRAY_SIZE
}

/// High-level assets manager.
///
/// It can be used to request assets by ID.
#[derive(Clone)]
pub struct Assets {
    inner: Arc<AssetsInner>,
}

struct AssetsInner {
    /// Loaders to load assets from.
    loaders: Box<[Box<dyn Loader>]>,

    /// Arrays are indexed by `assets_array_index(id)`.
    /// This helps to distribute asset access across multiple locks.
    types: RwLock<HashMap<TypeId, [Arc<dyn AnyTypedAssets>; ASSETS_ARRAY_SIZE]>>,

    // Queue of assets to build.
    to_build: FlipQueue<(TypeId, AssetId)>,
}

impl Assets {
    pub fn new(loaders: impl IntoIterator<Item = Box<dyn Loader>>) -> Self {
        Assets {
            inner: Arc::new(AssetsInner {
                loaders: loaders.into_iter().collect(),
                types: RwLock::new(HashMap::new()),
                to_build: FlipQueue::new(),
            }),
        }
    }

    /// Returns asset by ID.
    /// If asset is not in cache, it will be loaded asynchronously.
    /// When finally loaded and built, this function will return reference to the asset.
    /// If asset is not found or failed to load, this function will return None.
    pub fn get<A>(&self, id: AssetId) -> Poll<Result<A, Error>>
    where
        A: Asset,
    {
        self.typed_entry::<A>(id).poll_asset(id, self, None)
    }

    /// Returns asset by ID.
    /// If asset is not in cache, it will be loaded asynchronously.
    /// When finally loaded and built, this function will return reference to the asset.
    /// If asset is not found or failed to load, this function will return None.
    ///
    /// This function is similar to `get` but it allows to pass a context
    /// to wake up the task when asset is ready.
    pub fn poll<A>(&self, id: AssetId, cx: &mut Context) -> Poll<Result<A, Error>>
    where
        A: Asset,
    {
        self.typed_entry::<A>(id).poll_asset(id, self, Some(cx))
    }

    /// Drops all assets except assets of listed types.
    ///
    /// This function is not intended for game code.
    /// Editor will use it before switching plugins.
    /// Only Engine-provided asset types should be kept.
    #[doc(hidden)]
    pub fn drop_all_except(&self, keep: &[TypeId]) {
        let mut types_write = self.inner.types.write();
        types_write.retain(|type_id, map| {
            if keep.contains(type_id) {
                return true;
            }

            // Cancel all loading assets.
            // This is important to make ongoing tasks to not insert new assets.
            for typed_assets in map.iter() {
                typed_assets.cancel();
            }
            false
        });
    }

    pub fn build_assets(&self, builder: &mut AssetBuilder) {
        self.inner.to_build.drain_locking(|to_build| {
            for (type_id, id) in to_build {
                if let Some(typed) = self.typed_get(type_id, id) {
                    typed.build_asset(id, builder);
                }
            }
        });
    }

    fn typed_get(&self, type_id: TypeId, id: AssetId) -> Option<Arc<dyn AnyTypedAssets>> {
        let index = assets_array_index(id);

        let types_read = self.inner.types.read();

        let typed_assets = types_read.get(&type_id)?;
        Some(typed_assets[index].clone())
    }

    fn typed_entry<A>(&self, id: AssetId) -> Arc<TypedAssets<A>>
    where
        A: Asset,
    {
        #[cold]
        fn new_typed<A>(assets: &Assets, index: usize) -> Arc<TypedAssets<A>>
        where
            A: Asset,
        {
            let type_id = TypeId::of::<A>();

            let new_typed_array = array::from_fn(|_| {
                Arc::new(TypedAssets::<A> {
                    cache: Mutex::new(HashMap::new()),
                }) as Arc<dyn AnyTypedAssets>
            });

            let new_typed: Arc<TypedAssets<A>> =
                unsafe { new_typed_array[index].clone().downcast_arc_unchecked() };

            let mut types_write = assets.inner.types.write();
            match types_write.entry(type_id) {
                hashbrown::hash_map::Entry::Occupied(entry) => {
                    // Another thread already inserted the value, use it.
                    unsafe { entry.get()[index].clone().downcast_arc_unchecked() }
                }
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    // We are the first thread to insert the value.
                    entry.insert(new_typed_array);
                    new_typed
                }
            }
        }

        let index = assets_array_index(id);
        let type_id = TypeId::of::<A>();

        let types_read = self.inner.types.read();

        if let Some(typed_assets) = types_read.get(&type_id) {
            return unsafe { typed_assets[index].clone().downcast_arc_unchecked() };
        }

        drop(types_read);
        new_typed(self, index)
    }
}

trait AnyTypedAssets: Any + Send + Sync {
    fn build_asset(&self, id: AssetId, builder: &mut AssetBuilder);
    fn cancel(&self);
}

impl dyn AnyTypedAssets {
    unsafe fn downcast_arc_unchecked<A>(self: Arc<Self>) -> Arc<TypedAssets<A>>
    where
        A: Asset,
    {
        debug_assert_eq!(
            <Self as Any>::type_id(&*self),
            TypeId::of::<TypedAssets<A>>()
        );
        unsafe { Arc::from_raw(Arc::into_raw(self) as *const TypedAssets<A>) }
    }
}

impl<A> AnyTypedAssets for TypedAssets<A>
where
    A: Asset,
{
    fn build_asset(&self, id: AssetId, builder: &mut AssetBuilder) {
        let mut cache = self.cache.lock();

        match cache.remove(&id) {
            Some(AssetState::Loaded { asset, wakers }) => {
                let result = A::build(asset, builder);

                match result {
                    Ok(asset) => {
                        cache.insert(id, AssetState::Ready { asset });
                    }
                    Err(error) => {
                        cache.insert(id, AssetState::Error { error });
                    }
                }

                for waker in wakers {
                    waker.wake();
                }
            }
            Some(_) => {} // Ignore other states.
            None => {}    // Ignore removed assets.
        }
    }

    fn cancel(&self) {
        let mut cache = self.cache.lock();
        for (_, state) in cache.drain() {
            match state {
                AssetState::Loading { wakers } => {
                    for waker in wakers {
                        waker.wake();
                    }
                }
                _ => {}
            }
        }
    }
}

enum AssetState<A: Asset> {
    /// Asset is being loaded.
    Loading { wakers: Vec<Waker> },

    /// Asset will be ready after next initialization phase.
    Loaded {
        asset: A::Loaded,
        wakers: Vec<Waker>,
    },

    /// Asset loading failed.
    Error { error: Error },

    /// Asset is ready.
    Ready { asset: A },
}

struct TypedAssets<A: Asset> {
    cache: Mutex<HashMap<AssetId, AssetState<A>>>,
}

impl<A> TypedAssets<A>
where
    A: Asset,
{
    fn poll_asset(
        self: &Arc<Self>,
        id: AssetId,
        assets: &Assets,
        cx: Option<&mut Context>,
    ) -> Poll<Result<A, Error>> {
        match self.cache.lock().entry(id) {
            hashbrown::hash_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                AssetState::Loading { wakers } => {
                    if let Some(cx) = cx {
                        wakers.retain(|w| !w.will_wake(cx.waker()));
                        wakers.push(cx.waker().clone());
                    }
                    return Poll::Pending;
                }
                AssetState::Loaded { wakers, .. } => {
                    if let Some(cx) = cx {
                        wakers.retain(|w| !w.will_wake(cx.waker()));
                        wakers.push(cx.waker().clone());
                    }
                    return Poll::Pending;
                }
                AssetState::Ready { asset } => {
                    return Poll::Ready(Ok(asset.clone()));
                }
                AssetState::Error { error } => {
                    // Will never be ready.
                    return Poll::Ready(Err(error.clone()));
                }
            },
            hashbrown::hash_map::Entry::Vacant(entry) => {
                // Need to load the asset.

                let me = Arc::clone(self);
                let assets = assets.clone();

                tokio::spawn(async move {
                    let result = load_from_any(&assets.inner.loaders[..], id).await;

                    let data = {
                        let mut cache = me.cache.lock();
                        let Some(state) = cache.get_mut(&id) else {
                            // Removed, oh, well.
                            return;
                        };

                        match state {
                            AssetState::Loading { wakers } => match result {
                                Ok(data) => data,
                                Err(error) => {
                                    for waker in wakers.drain(..) {
                                        waker.wake();
                                    }
                                    *state = AssetState::Error { error };
                                    return;
                                }
                            },
                            _ => {
                                // Already loaded.
                                // This can happen if asset was removed after this task is started
                                // and then loaded again.
                                // In this case, we just ignore the result.
                                return;
                            }
                        }
                    };

                    let result = A::load(data.bytes, &assets).await;

                    let mut cache = me.cache.lock();
                    let Some(state) = cache.get_mut(&id) else {
                        // Removed, oh, well.
                        return;
                    };

                    match state {
                        AssetState::Loading { wakers } => match result {
                            Ok(asset) => {
                                *state = AssetState::Loaded {
                                    asset,
                                    wakers: std::mem::take(wakers),
                                };

                                drop(cache);
                                assets.inner.to_build.push((type_id::<A>(), id));
                            }
                            Err(error) => {
                                *state = AssetState::Error { error };
                            }
                        },
                        _ => {
                            // Already loaded.
                            // This can happen if asset was removed after this task is started
                            // and then loaded again.
                            // In this case, we just ignore the result.
                        }
                    }
                });

                let mut wakers = Vec::new();
                if let Some(cx) = cx {
                    wakers.push(cx.waker().clone());
                }
                entry.insert(AssetState::Loading { wakers });

                Poll::Pending
            }
        }
    }
}

async fn load_from_any(loaders: &[Box<dyn Loader>], id: AssetId) -> Result<AssetData, Error> {
    let mut not_found_error = None;

    for loader in loaders {
        match loader.load(id).await {
            Err(error) if error.is::<NotFound>() => {
                not_found_error = Some(error);
            }
            Err(error) => {
                return Err(error);
            }
            Ok(data) => {
                return Ok(data);
            }
        }
    }

    Err(not_found_error.unwrap_or_else(|| Error::new(NotFound)))
}
