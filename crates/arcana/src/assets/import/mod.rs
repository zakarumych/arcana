//! Contains everything that is required to create argosy importers library.
//!
//!
//! # Usage
//!
//! ```
//! struct FooImporter;
//!
//! impl argosy_import::Importer for FooImporter {
//!     fn name(&self) -> &str {
//!         "Foo importer"
//!     }
//!
//!     fn formats(&self) -> &[&str] {
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
//!         _sources: &mut dyn argosy_import::Sources,
//!         _dependencies: &mut dyn argosy_import::Dependencies,
//!     ) -> Result<(), argosy_import::ImportError> {
//!         match std::fs::copy(source, output) {
//!           Ok(_) => Ok(()),
//!           Err(err) => Err(argosy_import::ImportError::Other { reason: "SOMETHING WENT WRONG".to_owned() }),
//!         }
//!     }
//! }
//!
//!
//! // Define all required exports.
//! argosy_import::make_argosy_importers_library! {
//!     // Each <expr;> must have type &'static I where I: Importer
//!     &FooImporter;
//! }
//! ```

mod dependencies;
mod importer;
mod sources;

pub use self::{
    dependencies::{AssetDependencies, AssetDependency},
    importer::{ImportError, Importer, ImporterId},
    sources::AssetSources,
};

/// Helper function to emit an error if sources or dependencies are missing.
pub(crate) fn ensure(
    sources: Vec<String>,
    dependencies: Vec<AssetDependency>,
) -> Result<(), ImportError> {
    if sources.is_empty() && dependencies.is_empty() {
        Ok(())
    } else {
        Err(ImportError::Requires {
            sources,
            dependencies,
        })
    }
}
