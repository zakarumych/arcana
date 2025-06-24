use std::path::Path;

use arcana_intern::{Ident, Name};
use arcana_model::{Model, Value};

use crate::make_uid;

use super::{AssetDependencies, AssetDependency, AssetSources};

make_uid! {
    /// Unique identifier of an importer.
    pub ImporterId;
}

/// Error of `Importer::import` method.
pub enum ImportError {
    /// Importer requires data.
    Requires {
        /// Required sources to build this asset.
        sources: Vec<String>,

        /// Assets this asset depends on.
        dependencies: Vec<AssetDependency>,
    },

    /// Importer failed to import the asset.
    Other {
        /// Failure reason.
        reason: String,
    },
}

/// Trait for an asset importer.
pub trait Importer: Send + Sync + 'static {
    /// Returns name of the importer
    fn name(&self) -> Name
    where
        Self: Sized;

    /// Returns supported formats of the importer.
    fn formats(&self) -> &[Name]
    where
        Self: Sized;

    /// Returns supported file extensions of the importer.
    fn extensions(&self) -> &[&'static str]
    where
        Self: Sized;

    /// Returns target format of the importer.
    fn target(&self) -> Ident
    where
        Self: Sized;

    /// Returns configuration model of the importer.
    fn config(&self) -> Model
    where
        Self: Sized;

    /// Returns importer instance.
    fn new() -> Self
    where
        Self: Sized;

    /// Reads data from `source` path and writes result at `output` path.
    /// Implementation may request additional sources and dependencies.
    /// If some are missing it **should** return `Err(ImportError::Requires { .. })`
    /// with as much information as possible.
    fn import(
        &self,
        config: &Value,
        source: &Path,
        output: &Path,
        sources: &mut dyn AssetSources,
        dependencies: &mut dyn AssetDependencies,
    ) -> Result<(), ImportError>;
}
