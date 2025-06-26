use std::path::Path;

use arcana_intern::{Ident, Name};
use arcana_metatype::{primitive_meta, Meta};

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

pub struct ImporterDesc {
    pub name: Name,
    pub formats: Vec<Name>,
    pub extensions: Vec<&'static str>,
    pub target: Ident,
}

/// Special config type that should be used when importer does not require any configuration.
pub struct EmptyConfig;

primitive_meta!(EmptyConfig);

/// Trait for an asset importer.
pub trait Importer: Send + Sync + 'static {
    fn desc() -> ImporterDesc
    where
        Self: Sized;

    /// Returns default configuration for this importer.
    ///
    /// Caller may update this value before passing it to `import` method.
    ///
    /// Caller should not change the inner type of the returned value.
    ///
    /// This Meta should support be `EmptyConfig` type or a type that supports inspection.
    fn config(&self) -> Meta {
        Meta::new(EmptyConfig)
    }

    /// Reads data from `source` path and writes result at `output` path.
    /// Implementation may request additional sources and dependencies.
    /// If some are missing it **should** return `Err(ImportError::Requires { .. })`
    /// with as much information as possible.
    fn import(
        &self,
        source: &Path,
        output: &Path,
        config: Option<Meta>,
        sources: &mut dyn AssetSources,
        dependencies: &mut dyn AssetDependencies,
    ) -> Result<(), ImportError>;
}
