use std::{any::Any, future::Future};

use super::{assets::Assets, error::Error};

/// Asset trait must be implemented for a type to be loaded as an Asset.
pub trait Asset: Any + Send + Sync + Clone {
    type Build: Future<Output = Result<Self, Error>> + Send;

    /// Build asset from raw data.
    ///
    /// Loader is provided to load sub-assets.
    fn build(data: Box<[u8]>, assets: &Assets) -> Self::Build;
}
