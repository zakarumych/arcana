//! Editor hot-loads plugins.
//! This presents a challenge because references to plugin data must not survive after the plugin is unloaded.
//!
//! To solve this the editor places anything that may reference plugin data into a `Container`.
//!
//! When container is created it loads all plugins and imports everything plugins exports.
//!
//! Metadata can be fetched from the container.
//! And it can be used to run game instances.
//! Container will also handle communication with the game instances, ensuring that references
//! from the plugins are not leaked.
//!
//! To load new plugins a new container must be created and old one can be dropped when it's no longer needed.

use std::path::Path;

use arcana::plugin::ArcanaPlugin;
use arcana_project::Ident;
use miette::{Diagnostic, IntoDiagnostic, WrapErr};
use thiserror::Error;

/// Container that encapsulates all plugin data and objects that use it directly.
pub struct Container {
    /// List of plugins loaded from the library.
    /// In dependency-first order.
    plugins: Vec<(&'static Ident, &'static dyn ArcanaPlugin)>,

    /// Linked library.
    /// It is only used to keep the library loaded.
    /// It must be last member of the struct to ensure it is dropped last.
    _lib: libloading::Library,
}

#[derive(Diagnostic, Error, Debug)]
#[error("Plugin not found")]
#[diagnostic(code(ed::container::plugin_not_found), url(docsrs))]
pub struct PluginNotFound(libloading::Error);

impl Container {
    /// Load plugins from the given path and create a new container.
    ///
    /// This function checks that the library exists, can be linked, contains necessary symbols
    /// and verify version compatibility.
    /// This makes it improbable to load bad library by accident, yet easy to do so intentionally.
    /// At the end this function is technically unsound, but it is the best we can do.
    pub fn load(path: &Path) -> miette::Result<Self> {
        // Safety: nope.
        let r = unsafe { libloading::Library::new(path) };
        let lib = r.map_err(PluginNotFound)?;

        todo!()
    }
}
