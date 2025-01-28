use std::{any::Any, future::Future};

use vtid::HasVtid;

use super::{assets::Assets, build::AssetBuilder, error::Error};

/// Asset trait must be implemented for a type to be loaded as an Asset.
pub trait Asset: HasVtid + Send + Sync + Clone + 'static {
    /// Loaded, optionally not yet built asset.
    /// If building is not required, this can be Self.
    type Loaded: Any + Send + Sync;

    fn load(
        data: Box<[u8]>,
        assets: &Assets,
    ) -> impl Future<Output = Result<Self::Loaded, Error>> + Send;

    /// Build asset from raw data.
    ///
    /// Loader is provided to load sub-assets.
    fn build(loaded: Self::Loaded, builder: &mut AssetBuilder) -> Result<Self, Error>;
}
