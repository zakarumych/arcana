use std::{
    any::{Any, TypeId},
    sync::Arc,
    task::{Context, Poll, Waker},
};

use amity::flip_queue::FlipQueue;
use hashbrown::HashMap;
use parking_lot::{lock_api::MappedRwLockReadGuard, Mutex, RwLock};

use super::{
    asset::Asset,
    error::Error,
    id::AssetId,
    loader::{AssetData, Loader},
};

const ASSETS_ARRAY_SIZE: usize = 32;

fn assets_array_index(id: AssetId) -> usize {
    let v = id.value().get();
    let [a, b, c, d, e, f, g, h] = v.to_le_bytes();
    (a as usize * 3
        + b as usize * 5
        + c as usize * 7
        + d as usize * 11
        + e as usize * 13
        + f as usize * 17
        + g as usize * 19
        + h as usize * 23)
        % ASSETS_ARRAY_SIZE
}

/// High-level assets manager.
///
/// It can be used to request assets by ID.
#[derive(Clone)]
pub struct Assets {
    /// Loaders to load assets from.
    loaders: Arc<[Box<dyn Loader>]>,

    /// This array is indexed by `assets_array_index(id)`.
    /// This helps to distribute asset access across multiple locks.
    types: Arc<[RwLock<HashMap<TypeId, Arc<dyn AnyTypedAssets>>>; ASSETS_ARRAY_SIZE]>,

    /// Device to create GPU resources.
    device: mev::Device,
}

impl Assets {
    /// Returns asset by ID.
    /// If asset is not in cache, it will be loaded asynchronously.
    /// When finally loaded and built, this function will return reference to the asset.
    /// If asset is not found or failed to load, this function will return None.
    pub fn get<A>(&self, id: AssetId) -> Poll<Option<&A>>
    where
        A: Asset,
    {
        unimplemented!()
    }

    /// Returns asset by ID.
    /// If asset is not in cache, it will be loaded asynchronously.
    /// When finally loaded and built, this function will return reference to the asset.
    /// If asset is not found or failed to load, this function will return None.
    ///
    /// This function is similar to `get` but it allows to pass a context
    /// to wake up the task when asset is ready.
    pub fn poll<A>(&self, id: AssetId, cx: &mut Context) -> Poll<Option<&A>>
    where
        A: Asset,
    {
        unimplemented!()
    }

    /// Drops all assets of a given type.
    pub fn drop_all_of<A>(&self)
    where
        A: Asset,
    {
        for types in &self.types[..] {
            let mut types_write = types.write();
            types_write.remove(&TypeId::of::<A>());
        }
    }

    /// Returns device to create GPU resources.
    pub fn gpu_device(&self) -> &mev::Device {
        &self.device
    }

    fn typed<A>(&self, id: AssetId) -> Arc<TypedAssets<A>>
    where
        A: Asset,
    {
        let index = assets_array_index(id);
        let type_id = TypeId::of::<A>();

        let types = &self.types[index];
        let types_read = types.read();

        if let Some(typed_assets) = types_read.get(&type_id) {
            return unsafe { typed_assets.clone().downcast_arc_unchecked() };
        }

        drop(types_read);
        self.new_typed(index)
    }

    #[cold]
    fn new_typed<A>(&self, index: usize) -> Arc<TypedAssets<A>>
    where
        A: Asset,
    {
        let types = &self.types[index];
        let type_id = TypeId::of::<A>();

        let new_typed = Arc::new(TypedAssets::<A> {
            cache: Mutex::new(HashMap::new()),
        });

        let mut types_write = types.write();
        match types_write.entry(type_id) {
            hashbrown::hash_map::Entry::Occupied(entry) => {
                // Another thread already inserted the value, use it.
                unsafe { entry.get().clone().downcast_arc_unchecked() }
            }
            hashbrown::hash_map::Entry::Vacant(entry) => {
                // We are the first thread to insert the value.
                entry.insert(new_typed.clone());
                new_typed
            }
        }
    }
}

trait AnyTypedAssets: Any + Send + Sync {}

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

impl<A> AnyTypedAssets for TypedAssets<A> where A: Asset {}

enum AssetState<A> {
    Loading { wakers: Vec<Waker> },
    NotFound,
    Error { error: Error },
    Ready { asset: A },
}

struct TypedAssets<A> {
    cache: Mutex<HashMap<AssetId, AssetState<A>>>,
}

impl<A> TypedAssets<A>
where
    A: Asset,
{
    fn get(self: &Arc<Self>, id: AssetId, assets: &Assets, cx: Option<&mut Context>) -> Option<A> {
        match self.cache.lock().entry(id) {
            hashbrown::hash_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                AssetState::Loading { wakers } => {
                    if let Some(cx) = cx {
                        wakers.push(cx.waker().clone());
                    }
                    return None;
                }
                AssetState::Ready { asset } => {
                    return Some(asset.clone());
                }
                AssetState::NotFound => {
                    // Will never be ready.
                    return None;
                }
                AssetState::Error { .. } => {
                    // Will never be ready.
                    return None;
                }
            },
            hashbrown::hash_map::Entry::Vacant(entry) => {
                // Need to load the asset.

                let me = Arc::clone(self);
                let assets = assets.clone();

                tokio::spawn(async move {
                    let result = load_from_any(&assets.loaders[..], id).await;

                    let data = {
                        let mut cache = me.cache.lock();
                        let Some(state) = cache.get_mut(&id) else {
                            // Removed, oh, well.
                            return;
                        };

                        match state {
                            AssetState::Loading { wakers } => match result {
                                Ok(Some(data)) => data,
                                Ok(None) => {
                                    *state = AssetState::NotFound;
                                    return;
                                }
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

                    let result = A::build(data.bytes, &assets).await;

                    let mut cache = me.cache.lock();
                    let Some(state) = cache.get_mut(&id) else {
                        // Removed, oh, well.
                        return;
                    };

                    match state {
                        AssetState::Loading { wakers } => {
                            for waker in wakers.drain(..) {
                                waker.wake();
                            }
                            match result {
                                Ok(asset) => {
                                    *state = AssetState::Ready { asset };
                                }
                                Err(error) => {
                                    *state = AssetState::Error { error };
                                }
                            }
                        }
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

                None
            }
        }
    }
}

async fn load_from_any(
    loaders: &[Box<dyn Loader>],
    id: AssetId,
) -> Result<Option<AssetData>, Error> {
    for loader in loaders {
        match loader.load(id).await {
            Ok(None) => {
                // Try next loader.
            }
            Ok(Some(data)) => {
                return Ok(Some(data));
            }
            Err(error) => {
                return Err(error);
            }
        }
    }

    Ok(None)
}
