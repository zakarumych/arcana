use std::path::Path;

use arcana_intern::{Ident, Name, ident};

use crate::{dependency::Dependency, plugin::Plugin};

/// Project manifest.
/// Contains information about project, dependencies, systems order, etc.
/// Put into `<project-name.arcana>` file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectManifest {
    /// Name of the project.
    pub name: Ident,

    /// How to fetch engine dependency.
    /// Defaults to `Dependency::Crates(version())`.
    pub engine: Dependency,

    /// List of plugin libraries this project depends on.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub plugins: Vec<Plugin>,
}

impl ProjectManifest {
    pub fn get_plugin(&self, name: Ident) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.name == name)
    }

    pub fn get_plugin_mut(&mut self, name: Ident) -> Option<&mut Plugin> {
        self.plugins.iter_mut().find(|p| p.name == name)
    }

    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.iter().any(|p| p.name == name)
    }

    pub fn remove_plugin_idx(&mut self, idx: usize) {
        self.plugins.remove(idx);
    }
}
