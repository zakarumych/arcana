use std::path::{Path, absolute};

use arcana_error::{Error, WithContext};
use arcana_intern::{IdentError, Name, validate_ident};
use camino::{Utf8PathBuf, absolute_utf8};
use serde::{Deserialize, de, ser::SerializeStruct};

use crate::{CARGO_TOML_NAME, Ident, dependency::Dependency};

/// Contains information about plugin.
///
/// Plugins are libraries that export `ArcanaPlugin` via top-level `arcana_plugin` function.
/// `ArcanaPlugin` provides runtime available information about the plugin.
///
/// Plugins initialization consists of registering
/// `Component`s,
/// `Resource`s,
/// `System`s,
/// `InputFilter`s.
///
/// Initialization is done in a way that ensures that dependencies are initialized before dependents.
///
/// Components and resources are registered in the world.
/// Systems are registered in system hub
/// from which they are later fetched to be added to scheduler
/// according to the order specified in the manifest.
///
/// If manifest has enabled system not registered by plugin, game instance cannot be started.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Plugin {
    pub name: Name,
    pub dependency: Dependency,
    pub description: String,
    pub ident: Ident, // Same as name, but '-' are replaced with '_'
}

impl serde::Serialize for Plugin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializer = serializer.serialize_struct("Plugin", 3)?;
        serializer.serialize_field("name", &self.name)?;
        serializer.serialize_field("dependency", &self.dependency)?;
        serializer.serialize_field("description", &self.description)?;
        serializer.end()
    }
}

impl<'de> serde::Deserialize<'de> for Plugin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct PluginDe {
            name: Name,
            dependency: Dependency,
            description: String,
        }

        let PluginDe {
            name,
            dependency,
            description,
        } = Deserialize::deserialize(deserializer)?;

        let ident = make_ident(name).map_err(|err| err.as_serde_error())?;

        Ok(Plugin {
            name,
            ident,
            description,
            dependency,
        })
    }
}

impl Plugin {
    /// Create plugin from dependency.
    pub fn from_dependency(name: String, dependency: Dependency) -> Result<Self, Error> {
        match dependency {
            Dependency::Crates(version) => Plugin::released(name, version),
            Dependency::Git { git, branch } => Plugin::from_git(name, git, branch),
            Dependency::Path { path } => {
                let plugin = Plugin::open_local(path)?;
                if plugin.name != name {
                    return Err(Error::msg(format!(
                        "Plugin name mismatch: expected '{name}', found '{}'",
                        plugin.name
                    )));
                }
                Ok(plugin)
            }
        }
    }

    /// Create plugin from crates.io.
    pub fn released(name: String, version: String) -> Result<Self, Error> {
        let (name, ident) = name_ident(name)?;
        Ok(Plugin {
            name,
            ident,
            description: String::new(),
            dependency: Dependency::Crates(version),
        })
    }

    /// Create plugin from git repository.
    pub fn from_git(name: String, git: String, branch: Option<String>) -> Result<Self, Error> {
        let (name, ident) = name_ident(name)?;
        Ok(Plugin {
            name,
            ident,
            description: String::new(),
            dependency: Dependency::Git { git, branch },
        })
    }

    /// Open local plugin from path.
    pub fn open_local(path: Utf8PathBuf) -> Result<Self, Error> {
        let Ok(real_path) = absolute_utf8(&path) else {
            return Err(Error::msg(format!(
                "Failed to resolve plugin path: {}",
                path
            )));
        };

        let cargo_toml_path = real_path.join(CARGO_TOML_NAME);

        let manifest = match cargo_toml::Manifest::from_path(cargo_toml_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                return Err(Error::msg(format!(
                    "Failed to read plugin manifest '{path}/{CARGO_TOML_NAME}': {error:?}"
                )));
            }
        };

        let package = match manifest.package {
            Some(package) => package,
            None => {
                return Err(Error::msg(format!(
                    "Plugin manifest '{path}/{CARGO_TOML_NAME}' does not contain package section",
                )));
            }
        };

        let (name, ident) = name_ident(package.name)
            .with_context(format!("Plugin manifest '{path}/{CARGO_TOML_NAME}'"))?;

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
            ident,
            description,
            dependency,
        })
    }
}

fn name_ident(s: String) -> Result<(Name, Ident), Error> {
    if s.contains('-') {
        let Ok(ident) = Ident::from_string(s.replace('-', "_")) else {
            return Err(Error::msg(format!("Name is not valid crate identifier",)));
        };

        let name = Name::from_string(s).unwrap();

        Ok((name, ident))
    } else {
        let Ok(ident) = Ident::from_string(s) else {
            return Err(Error::msg(format!("Name is not valid crate identifier",)));
        };

        let name = ident.into();

        Ok((name, ident))
    }
}

fn make_ident(name: Name) -> Result<Ident, IdentError> {
    if name.contains('-') {
        Ident::from_string(name.replace('-', "_"))
    } else {
        Ident::from_static(name.as_str())
    }
}
