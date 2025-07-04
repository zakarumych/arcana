use std::{any::Any, future::Future};

use arcana_metatype::MetaType;

use crate::AssetError;

use super::{assets::Assets, build::AssetBuilder};

/// Asset trait must be implemented for a type to be loaded as an Asset.
pub trait Asset: MetaType + Send + Sync + Clone {
    /// Loaded, optionally not yet built asset.
    /// If building is not required, this can be Self.
    type Loaded: Any + Send + Sync;

    fn load(
        data: &[u8],
        assets: &Assets,
    ) -> impl Future<Output = Result<Self::Loaded, AssetError>> + Send;

    /// Build asset from raw data.
    ///
    /// Loader is provided to load sub-assets.
    fn build(loaded: Self::Loaded, builder: &mut AssetBuilder) -> Result<Self, AssetError>;
}
