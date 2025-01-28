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

mod dependencies;
mod importer;
mod sources;

pub use self::{
    dependencies::{AssetDependencies, AssetDependency},
    importer::{EmptyConfig, ImportError, Importer, ImporterDesc, ImporterId},
    sources::AssetSources,
};

/// Helper function for [`Importer`] implementations to emit an error if sources or dependencies are missing.
pub fn ensure(sources: Vec<String>, dependencies: Vec<AssetDependency>) -> Result<(), ImportError> {
    if sources.is_empty() && dependencies.is_empty() {
        Ok(())
    } else {
        Err(ImportError::Requires {
            sources,
            dependencies,
        })
    }
}
