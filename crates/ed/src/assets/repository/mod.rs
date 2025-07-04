use std::path::PathBuf;

use arcana::{metatype::Stid, smol_str::SmolStr};
use futures::future::BoxFuture;
use hashbrown::HashMap;

use crate::{
    assets::{AssetData, AssetId, Error, Loader},
    blobs::{BlobId, Blobs},
};

/// Asset repository for the Arcana engine.
pub struct Repository {
    /// Container for asset blobs.
    blobs: Blobs,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct StoredAsset {
    /// Asset ID of the asset.
    /// This is a unique identifier for the asset within the repository.
    id: AssetId,

    /// Asset name.
    /// This is a human-readable identifier for the asset.
    name: SmolStr,

    /// Blob ID of the asset data.
    blob: BlobId,

    /// Asset type ID.
    stid: Stid,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RepositoryData {
    /// Metadata for all stored assets.
    assets: Vec<StoredAsset>,

    /// Map from asset ID to index in `assets`.
    id_map: HashMap<AssetId, usize>,

    /// Map from asset name to index in `assets`.
    name_map: HashMap<SmolStr, usize>,
}

impl Repository {
    fn new(path: PathBuf) -> Self {
        Repository {
            blobs: Blobs::new(path).unwrap(),
        }
    }
}

/// [`Loader`] for [`Repository`].
struct RepositoryLoader {}

impl Loader for RepositoryLoader {
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, Error>> {
        todo!()
    }

    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, Error>> {
        todo!()
    }
}
