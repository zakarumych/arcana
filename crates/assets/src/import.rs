//! Contains everything that is required to create importers.
//!
//!
//! # Usage
//!
//! ```
//! struct FooImporter;
//!
//! impl Importer for FooImporter {
//!     fn name(&self) -> &str {
//!         "Foo importer"
//!     }
//!
//!     fn formats(&self) -> &[&str] {1
//!         &["foo"]
//!     }
//!
//!     fn target(&self) -> &str {
//!         "foo"
//!     }
//!
//!     fn extensions(&self) -> &[&str] {
//!         &["json"]
//!     }
//!
//!     fn import(
//!         &self,
//!         source: &std::path::Path,
//!         output: &std::path::Path,
//!         _sources: &mut dyn Sources,
//!         _dependencies: &mut dyn Dependencies,
//!     ) -> Result<(), ImportError> {
//!         match std::fs::copy(source, output) {
//!           Ok(_) => Ok(()),
//!           Err(err) => Err(ImportError::Other { reason: "SOMETHING WENT WRONG".to_owned() }),
//!         }
//!     }
//! }
//! ```

use std::path::{Path, PathBuf};

use arcana_intern::{Ident, Name};
use arcana_model::{Model, Value};
use smol_str::SmolStr;

use crate::{make_uid, AssetId};

/// Single dependency for a asset.
#[derive(Debug)]
pub struct AssetDependency {
    /// Source path.
    pub source: SmolStr,

    /// Target format.
    pub target: Ident,
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
