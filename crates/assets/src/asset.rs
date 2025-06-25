use std::{any::Any, future::Future};

use arcana_id::HasStid;
use vtid::HasVtid;

use super::{assets::Assets, build::AssetBuilder, error::Error};

/// Asset trait must be implemented for a type to be loaded as an Asset.
pub trait Asset: HasStid + HasVtid + Send + Sync + Clone + 'static {
    /// Loaded, optionally not yet built asset.
    /// If building is not required, this can be Self.
    type Loaded: Any + Send + Sync;

    fn load(
        data: &[u8],
        assets: &Assets,
    ) -> impl Future<Output = Result<Self::Loaded, Error>> + Send;

    /// Build asset from raw data.
    ///
    /// Loader is provided to load sub-assets.
    fn build(loaded: Self::Loaded, builder: &mut AssetBuilder) -> Result<Self, Error>;
}
