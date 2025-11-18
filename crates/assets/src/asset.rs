use std::{any::Any, future::Future};

use arcana_id::HasStid;

use crate::AssetError;

use super::{assets::Assets, build::AssetBuilder};

/// This trait must be derived for all asset types.
pub trait AssetVersion: HasStid {
    /// Opaque version number of the asset type.
    /// It usually increases when asset type is recompiled,
    /// but may stay the same if dynamic library defining the asset type is rebuilt
    /// without recompilation of the type.
    /// Thus indicating that instances of the asset type can be reused.
    fn version() -> u64
    where
        Self: Sized;
}

/// Asset trait must be implemented for a type to be loaded as an Asset.
pub trait Asset: AssetVersion + Send + Sync + Clone {
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
