use std::path::PathBuf;

use futures::future::BoxFuture;
use hashbrown::HashMap;

use crate::{
    assets::{AssetData, AssetId, Error, Loader},
    io::blobs::{BlobId, Blobs},
};

/// Asset manager for the Arcana engine.
pub struct Repository {
    blobs: Blobs,
}

impl Repository {
    fn new(path: PathBuf) -> Self {
        Repository {
            blobs: Blobs::new(path).unwrap(),
        }
    }
}

/// Repository of assets with specific type.
struct TypedRepository {
    assets: HashMap<AssetId, BlobId>,
}

/// [`Loader`] for [`Repository`].
struct RepositoryLoader {}

impl Loader for RepositoryLoader {
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, Error>> {}

    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, Error>> {
    }
}
