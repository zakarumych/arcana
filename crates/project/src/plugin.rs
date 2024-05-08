use std::path::Path;

use camino::Utf8PathBuf;

use crate::{dependency::Dependency, real_path, IdentBuf, CARGO_TOML_NAME};

/// Contains information about plugin.
///
/// Plugins are libraries that export `ArcanaPlugin` via top-level `arcana_plugin` function.
/// `ArcanaPlugin` provides runtime avaialable information about the plugin.
///
/// Plugins initializaton consists of registering
/// `Component`s,
/// `Resource`s,
/// `System`s,
/// `EventFilter`s.
///
/// Initialization is done in a way that ensures that dependencies are initialized before dependents.
///
/// Components and resources are registered in the world.
/// Systems are registered in system hub
/// from which they are later fetched to be added to scheduler
/// according to the order specified in the manifest.
///
/// If manifest has enabled system not registed by plugin, game instance cannot be started.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Plugin {
    pub name: IdentBuf,
    pub description: String,
    pub dependency: Dependency,
}

impl Plugin {
    /// Create plugin from dependency.
    pub fn from_dependency(name: IdentBuf, dependency: Dependency) -> miette::Result<Self> {
        match dependency {
            Dependency::Crates(version) => Ok(Plugin::released(name, version)),
            Dependency::Git { git, branch } => Ok(Plugin::from_git(name, git, branch)),
            Dependency::Path { path } => {
                let plugin = Plugin::open_local(path)?;
                if plugin.name != name {
                    miette::bail!(
                        "Plugin name mismatch: expected '{name}', found '{}'",
                        plugin.name
                    );
                }
                Ok(plugin)
            }
        }
    }

    /// Create plugin from crates.io.
    pub fn released(name: IdentBuf, version: String) -> Self {
        Plugin {
            name,
            description: String::new(),
            dependency: Dependency::Crates(version),
        }
    }

    /// Create plugin from git repository.
    pub fn from_git(name: IdentBuf, git: String, branch: Option<String>) -> Self {
        Plugin {
            name,
            description: String::new(),
            dependency: Dependency::Git { git, branch },
        }
    }

    /// Open local plugin from path.
    pub fn open_local(path: Utf8PathBuf) -> miette::Result<Self> {
        let Some(real_path) = real_path(path.as_std_path()) else {
            miette::bail!("Failed to resolve plugin path: {}", path);
        };

        let cargo_toml_path = real_path.join(CARGO_TOML_NAME);

        let manifest = match cargo_toml::Manifest::from_path(cargo_toml_path) {
            Ok(manifest) => manifest,
            Err(err) => {
                miette::bail!("Failed to read plugin manifest '{path}/{CARGO_TOML_NAME}': {err:?}",);
            }
        };

        let package = match manifest.package {
            Some(package) => package,
            None => {
                miette::bail!(
                    "Plugin manifest '{path}/{CARGO_TOML_NAME}' does not contain package section",
                );
            }
        };

        let Ok(name) = IdentBuf::from_string(package.name) else {
            miette::bail!(
                "Plugin manifest '{path}/{CARGO_TOML_NAME}' package name is not valid identifier",
            );
        };

        let description = match package.description {
            Some(cargo_toml::Inheritable::Set(description)) => description,
            Some(cargo_toml::Inheritable::Inherited { .. }) => {
                tracing::warn!(
                    "Plugin manifest '{path}/{CARGO_TOML_NAME}' package description is inherited, fetching from workspace is not yet supported",
                );
                String::new()
            }
            None => String::new(),
        };

        let dependency = Dependency::Path { path };

        Ok(Plugin {
            name,
            description,
            dependency,
        })
    }
}
