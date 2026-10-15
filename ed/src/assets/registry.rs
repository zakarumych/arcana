use std::{
    future::ready,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use arcana::{
    assets::{AssetData, AssetError, AssetId, Loader, NotFound},
    error::{Error, WithContext},
    hash::HashMap,
};
use futures::future::BoxFuture;
use parking_lot::Mutex;

use crate::blobs::{BlobId, Blobs};

/// Loader of local imported assets.
///
/// Provided by [`AssetStore`](super::store::AssetStore).
#[derive(Clone)]
pub struct AssetRegistry {
    inner: Arc<Inner>,
}

#[derive(PartialEq, Eq)]
struct Item {
    blob: BlobId,
    version: u64,
}

struct Inner {
    artifacts: Mutex<HashMap<AssetId, Item>>,
    blobs: Blobs,
    revision: AtomicU64,
}

impl AssetRegistry {
    pub fn new(base: PathBuf) -> Result<Self, Error> {
        Ok(AssetRegistry {
            inner: Arc::new(Inner {
                blobs: Blobs::new(base)
                    .with_context("Failed to open blobs path for asset registry")?,
                artifacts: Mutex::new(HashMap::default()),
                revision: AtomicU64::new(0),
            }),
        })
    }

    /// Stages an artifact without making it visible to runtime loaders.
    pub(crate) fn stage_file(&self, file: impl AsRef<Path>) -> Result<BlobId, std::io::Error> {
        self.inner.blobs.insert_tmp_file(file)
    }

    pub(crate) fn publish(&self, id: AssetId, blob: BlobId, version: u64) {
        let mut artifacts = self.inner.artifacts.lock();
        let item = Item { blob, version };
        if artifacts.get(&id) != Some(&item) {
            artifacts.insert(id, item);
            self.inner.revision.fetch_add(1, Ordering::Release);
        }
    }

    /// Removes runtime mappings without deleting shared artifact blobs.
    pub(crate) fn unpublish(&self, ids: &[AssetId]) {
        let mut artifacts = self.inner.artifacts.lock();
        let mut changed = false;
        for id in ids {
            changed |= artifacts.remove(id).is_some();
        }
        if changed {
            self.inner.revision.fetch_add(1, Ordering::Release);
        }
    }

    /// Changes when an artifact is added, removed, or replaced.
    pub(crate) fn revision(&self) -> u64 {
        self.inner.revision.load(Ordering::Acquire)
    }

    pub(crate) fn contains_blob(&self, blob: BlobId) -> bool {
        self.inner.blobs.contains(blob)
    }

    /// Replaces all mappings with readable artifacts from persisted metadata.
    pub(crate) fn restore(&self, entries: impl IntoIterator<Item = (AssetId, BlobId, u64)>) {
        let artifacts: HashMap<AssetId, Item> = entries
            .into_iter()
            .filter(|(_, blob, _)| self.inner.blobs.contains(*blob))
            .map(|(id, blob, version)| (id, Item { blob, version }))
            .collect();
        let mut current = self.inner.artifacts.lock();
        if *current != artifacts {
            *current = artifacts;
            self.inner.revision.fetch_add(1, Ordering::Release);
        }
    }

    pub fn add_asset_from_file(
        &self,
        id: AssetId,
        version: u64,
        file: impl AsRef<Path>,
    ) -> Result<BlobId, std::io::Error> {
        let blob = self.inner.blobs.insert_file(file)?;
        self.publish(id, blob, version);
        Ok(blob)
    }

    pub fn add_asset_from_tmp_file(
        &self,
        id: AssetId,
        version: u64,
        file: impl AsRef<Path>,
    ) -> Result<BlobId, std::io::Error> {
        let blob = self.stage_file(file)?;
        self.publish(id, blob, version);
        Ok(blob)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_tracks_effective_artifacts_and_ignores_metadata_edits() {
        let root = std::env::temp_dir().join(format!(
            "arcana-registry-revision-{}",
            rand::random::<u128>()
        ));
        let registry = AssetRegistry::new(root.clone()).unwrap();
        let id = arcana::id::static_id!(41 as AssetId);
        let first = registry.inner.blobs.insert(b"first").unwrap();
        let second = registry.inner.blobs.insert(b"second").unwrap();
        assert_eq!(registry.revision(), 0);

        registry.publish(id, first, 1);
        assert_eq!(registry.revision(), 1);
        registry.publish(id, first, 1);
        registry.restore([(id, first, 1)]);
        assert_eq!(registry.revision(), 1);

        let mut source = crate::assets::SourceMeta::open(root.join("source.txt")).unwrap();
        source.set_tags(vec!["hero".into()]).unwrap();
        registry.restore([(id, first, 1)]);
        assert_eq!(registry.revision(), 1);

        registry.publish(id, second, 1);
        assert_eq!(registry.revision(), 2);
        registry.publish(id, second, 2);
        assert_eq!(registry.revision(), 3);
        registry.restore([]);
        assert_eq!(registry.revision(), 4);
        assert!(futures::executor::block_on(registry.load(id)).is_err());
        registry.restore([]);
        assert_eq!(registry.revision(), 4);
        drop(registry);
        std::fs::remove_dir_all(root).unwrap();
    }
}
