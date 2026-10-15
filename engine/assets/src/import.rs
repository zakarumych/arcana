//! Contains everything that is required to create importers.
//!
//!
//! # Usage
//!
//! ```
//! use arcana_assets::import::{ImportContext, ImportError, Importer, ImporterDesc};
//! use arcana_intern::{Ident, Name};
//! use arcana_model::{Model, Value};
//!
//! struct FooImporter;
//!
//! impl Importer for FooImporter {
//!     fn desc() -> ImporterDesc {
//!         ImporterDesc {
//!             name: Name::from_static("Foo importer").unwrap(),
//!             formats: vec![Name::from_static("foo").unwrap()],
//!             extensions: vec!["foo"],
//!             target: Ident::from_static("foo").unwrap(),
//!             config: (Model::Unit, Value::Unit),
//!         }
//!     }
//!
//!     fn import(
//!         &self,
//!         source: &std::path::Path,
//!         output: &std::path::Path,
//!         _config: Value,
//!         _context: &mut dyn ImportContext,
//!     ) -> Result<(), ImportError> {
//!         std::fs::copy(source, output)
//!             .map(|_| ())
//!             .map_err(|error| ImportError::Other { reason: error.to_string() })
//!     }
//! }
//! ```

use std::path::{Path, PathBuf};

use arcana_intern::{Ident, Name};
use arcana_model::{Model, Value};
use smol_str::SmolStr;

use crate::{AssetId, make_uid};

/// Single dependency for a asset.
#[derive(Debug)]
pub struct AssetDependency {
    /// Target format.
    pub target: Ident,

    /// Source path.
    pub source: SmolStr,

    /// Source format if specified.
    pub format: Option<Name>,
}

/// Indicates that requested item is missing.
///
/// Asset pipeline will attempt to make it available and try again.
pub struct Missing;

/// Provides access to asset dependencies.
/// Converts source and target to asset id.
pub trait ImportContext {
    /// Returns dependency id.
    /// If dependency is not available, returns `None`.
    fn get_dependency(&mut self, source: &str, target: Ident) -> Result<AssetId, Missing>;

    /// Returns path to the source.
    /// If source is not available, returns `None`.
    ///
    /// If `source` is a supported non-file URL it will be downloaded and cached locally.
    fn get_source(&mut self, source: &str) -> Result<PathBuf, Missing>;
}

make_uid! {
    /// Unique identifier of an importer.
    pub ImporterId;
}

/// Error of `Importer::import` method.
pub enum ImportError {
    /// Importer requires data.
    Requires {
        /// Required sources to build this asset.
        sources: Vec<SmolStr>,

        /// Assets this asset depends on.
        dependencies: Vec<AssetDependency>,
    },

    /// Importer failed to import the asset.
    Other {
        /// Failure reason.
        reason: String,
    },
}

/// Description of an importer.
#[derive(Clone, Debug)]
pub struct ImporterDesc {
    /// Human-readable name of the importer.
    pub name: Name,

    /// List of source formats this importer supports.
    pub formats: Vec<Name>,

    /// List of file extensions this importer expects.
    pub extensions: Vec<&'static str>,

    /// Target format of the asset this importer produces.
    pub target: Ident,

    /// Configuration for the importer. Model and default value.
    pub config: (Model, Value),
}

/// Trait for an asset importer.
pub trait Importer: Send + Sync + 'static {
    fn desc() -> ImporterDesc
    where
        Self: Sized;

    /// Reads data from `source` path and writes result at `output` path.
    /// Implementation may request additional sources and dependencies.
    /// If some are missing it **should** return `Err(ImportError::Requires { .. })`
    /// with as much information as possible.
    fn import(
        &self,
        source: &Path,
        output: &Path,
        config: Value,
        context: &mut dyn ImportContext,
    ) -> Result<(), ImportError>;
}
