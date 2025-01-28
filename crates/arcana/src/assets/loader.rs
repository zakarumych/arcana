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
    /// Returns `Ok(Some(asset_data))` if asset is loaded successfully.
    /// Returns `Ok(None)` if asset is not found, allowing checking other sources.
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, Error>>;

    /// Update asset data if newer is available.
    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, Error>>;
}
