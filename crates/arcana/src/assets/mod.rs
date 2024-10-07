//! Assets are crucial part of any game.
//!
//! In Arcana assets are managed by a pipeline.
//!
//! - Import step happens when developer adds new asset source.
//!   Once imported, an asset ID is allocated for it and associated with the source.
//!   Association is stored in the file system along with the source file.
//!   For non-FS sources, association is stored at specific configurable location.
//!
//! - Process step happens after import. It is responsible for converting source into a game-native format.
//!   User-defined processors are responsible for this step. One must be chosen at import step.
//!
//! - Load step happens at runtime when asset is fetched and not found in cache.
//!   In development, if source change is detected, asset is re-processed before loading.
//!
//! - Build step happens after right after load step.
//!   It is responsible for converting raw asset data into an asset object.
//!   This includes creating GPU resources and filling them with data.
//!
//!
//! Assets are stored in a cache and may be evicted if not used to free up CPU and GPU memory.
//! For this purpose assets declare how much they consume.
//!
//! Assets may contain other assets. This is useful to allow object of any complexity to be an asset.
//! For example renderable asset may contain mesh, material and shader assets.
//! Character asset may contain multiple renderable assets, as well as animations, sounds and behavior assets.
//! Map asset may contain terrain, buildings, characters and other complex assets.
//!
//! When complex asset is processed, references to sub-assets contained in source must be resolved to asset IDs and saved in game-native format.
//!
//! Complex assets may also implement `Unfold` trait for unfolding single object into multiple components and other entities.

mod asset;
mod assets;
mod build;
mod error;
mod id;
pub mod import;
mod loader;

pub use self::{
    asset::Asset,
    assets::Assets,
    build::{AssetBuildContext, AssetBuilder},
    error::{Error, NotFound},
    id::AssetId,
    loader::{AssetData, Loader},
};
