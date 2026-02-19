use std::{
    future::ready,
    path::{Path, PathBuf},
    sync::Arc,
};

use arcana::{
    assets::{AssetData, AssetError, AssetId, Loader, NotFound},
    error::{Error, WithContext},
};
use futures::future::BoxFuture;
use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::blobs::{BlobId, Blobs};

/// Loader of local imported assets.
///
/// Provided by [`AssetStore`](super::store::AssetStore).
#[derive(Clone)]
pub struct AssetRegistry {
    inner: Arc<Inner>,
}

struct Item {
    blob: BlobId,
    version: u64,
}

struct Inner {
    artifacts: Mutex<HashMap<AssetId, Item>>,
    blobs: Blobs,
}

impl AssetRegistry {
    pub fn new(base: PathBuf) -> Result<Self, Error> {
        Ok(AssetRegistry {
            inner: Arc::new(Inner {
                blobs: Blobs::new(base)
                    .with_context("Failed to open blobs path for asset registry")?,
                artifacts: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Adds new asset to the registry.
    pub fn add_asset_from_file(
        &self,
        id: AssetId,
        version: u64,
        file: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        let inner = &*self.inner;
        let blob = inner.blobs.insert_file(file)?;
        let mut artifacts = inner.artifacts.lock();
        artifacts.insert(id, Item { blob, version });
        Ok(())
    }

    /// Adds new asset to the registry.
    pub fn add_asset_from_tmp_file(
        &self,
        id: AssetId,
        version: u64,
        tmp_file: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        let inner = &*self.inner;
        let blob = inner.blobs.insert_tmp_file(tmp_file)?;
        let mut artifacts = inner.artifacts.lock();
        artifacts.insert(id, Item { blob, version });
        Ok(())
    }
}

impl Loader for AssetRegistry {
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, AssetError>> {
        let inner = &*self.inner;
        let artifacts = inner.artifacts.lock();
        Box::pin(ready(match artifacts.get(&id) {
            None => Err(AssetError::new(NotFound)),
            Some(item) => match inner.blobs.read(item.blob) {
                Ok(bytes) => Ok(AssetData {
                    bytes: bytes.into_boxed_slice(),
                    version: item.version,
                }),
                Err(error) => Err(AssetError::new(error)),
            },
        }))
    }

    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, AssetError>> {
        let inner = &*self.inner;
        let artifacts = inner.artifacts.lock();
        Box::pin(ready(match artifacts.get(&id) {
            None => Err(AssetError::new(NotFound)),
            Some(item) if item.version <= version => Ok(None),
            Some(item) => match inner.blobs.read(item.blob) {
                Ok(bytes) => Ok(Some(AssetData {
                    bytes: bytes.into_boxed_slice(),
                    version: item.version,
                })),
                Err(error) => Err(AssetError::new(error)),
            },
        }))
    }
}
