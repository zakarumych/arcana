use futures::future::BoxFuture;

use super::{error::Error, AssetId};

/// Asset data loaded from [`Store`].
pub struct AssetData {
    /// Serialized asset data.
    pub bytes: Box<[u8]>,

    /// Opaque version for asset.
    /// It can only by interpreted by [`Loader`]
    /// that returned this [`AssetData`] instance.
    pub version: u64,
}

/// Abstract loader for asset raw data.
pub trait Loader: Send + Sync + 'static {
    /// Load asset data from this loader.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] containing [`NotFound`] if asset is not found.
    /// In which case it is recommended to try other loaders.
    ///
    /// [`NotFound`]: crate::assets::NotFound
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, Error>>;

    /// Update asset data if newer is available.
    /// Use version from last returned [`AssetData`] for this asset.
    /// If newer version is available, it will be returned in [`AssetData`].
    /// If no newer version is available, [`None`] will be returned.
    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, Error>>;
}
