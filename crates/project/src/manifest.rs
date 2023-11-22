use std::path::Path;

use crate::{
    dependency::Dependency,
    ident::{Ident, IdentBuf},
};

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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Plugin {
    /// Name of the plugin.
    pub name: IdentBuf,

    /// Dependency that can be added to the package to link with the plugin.
    pub dep: Dependency,

    /// Whether this plugin is enabled.
    /// Disabled plugins are not initialized.
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Item {
    /// Plugin that registers this system.
    pub plugin: IdentBuf,

    /// Name of the system.
    pub name: IdentBuf,

    /// Whether this system is enabled.
    /// Disabled systems are not added to scheduler.
    pub enabled: bool,
}

impl serde::Serialize for Item {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct SerItem<'a> {
            plugin: &'a Ident,
            name: &'a Ident,
            enabled: bool,
        }

        if serializer.is_human_readable() {
            if self.enabled {
                serializer.serialize_str(&format!("{}:{}", self.plugin, self.name))
            } else {
                serializer.serialize_str(&format!("!{}:{}", self.plugin, self.name))
            }
        } else {
            serde::Serialize::serialize(
                &SerItem {
                    plugin: &self.plugin,
                    name: &self.name,
                    enabled: self.enabled,
                },
                serializer,
            )
        }
    }
}

impl<'de> serde::Deserialize<'de> for Item {
    fn deserialize<D>(deserializer: D) -> Result<Item, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct DeItem {
            plugin: IdentBuf,
            name: IdentBuf,
            enabled: bool,
        }

        if deserializer.is_human_readable() {
            let expected =
                || serde::de::Error::custom("Expected <plugin:system> or <!plugin:system>");

            let flag_id = String::deserialize(deserializer)?;
            let mut id = &flag_id[..];
            let disabled = id.starts_with('!');
            if disabled {
                id = &id[1..];
            }
            let (plugin, name) = id.split_once(':').ok_or_else(expected)?;
            let plugin = Ident::from_str(plugin).map_err(|_| expected())?;
            let name = Ident::from_str(name).map_err(|_| expected())?;

            Ok(Item {
                plugin: plugin.to_buf(),
                name: name.to_buf(),
                enabled: !disabled,
            })
        } else {
            let system: DeItem = serde::Deserialize::deserialize(deserializer)?;
            Ok(Item {
                plugin: system.plugin,
                name: system.name,
                enabled: system.enabled,
            })
        }
    }
}

/// Project manifest.
/// Contains information about project, dependencies, systems order, etc.
/// Usually put into `Arcana.toml` file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectManifest {
    /// Name of the project.
    pub name: IdentBuf,

    /// How to fetch engine dependency.
    /// Defaults to `Dependency::Crates(version())`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub engine: Option<Dependency>,

    /// List of plugin libraries this project depends on.
    #[serde(skip_serializing_if = "Vec::is_empty", default, with = "plugins_serde")]
    pub plugins: Vec<Plugin>,

    /// List of systems in order they should be added to scheduler.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub var_systems: Vec<Item>,

    /// List of systems in order they should be added to scheduler.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fix_systems: Vec<Item>,

    /// List of systems in order they should be added to scheduler.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub filters: Vec<Item>,
}

impl ProjectManifest {
    pub fn get_plugin(&self, name: &Ident) -> Option<&Plugin> {
        self.plugins.iter().find(|p| &p.name == name)
    }

    pub fn get_plugin_mut(&mut self, name: &Ident) -> Option<&mut Plugin> {
        self.plugins.iter_mut().find(|p| &p.name == name)
    }

    pub fn has_plugin(&self, name: &Ident) -> bool {
        self.plugins.iter().any(|p| &p.name == name)
    }

    pub fn enable_plugin(&mut self, name: &Ident, enabled: bool) {
        if let Some(plugin) = self.get_plugin_mut(name) {
            plugin.enabled = enabled;
        }
    }

    pub fn remove_plugin(&mut self, name: &Ident) -> bool {
        let mut removed = false;
        self.plugins.retain(|p| {
            let retain = p.name != *name;
            removed |= !retain;
            retain
        });
        removed
    }

    pub fn remove_plugin_idx(&mut self, idx: usize) {
        self.plugins.remove(idx);
    }

    pub fn get_var_system(&self, plugin: &Ident, name: &Ident) -> Option<&Item> {
        self.var_systems
            .iter()
            .find(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn get_var_system_mut(&mut self, plugin: &Ident, name: &Ident) -> Option<&mut Item> {
        self.var_systems
            .iter_mut()
            .find(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn has_var_system(&self, plugin: &Ident, name: &Ident) -> bool {
        self.var_systems
            .iter()
            .any(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn add_var_system(&mut self, plugin: &Ident, name: &Ident, enabled: bool) {
        if !self.has_var_system(plugin, name) {
            self.var_systems.push(Item {
                plugin: plugin.to_buf(),
                name: name.to_buf(),
                enabled,
            });
        }
    }

    pub fn get_fix_system(&self, plugin: &Ident, name: &Ident) -> Option<&Item> {
        self.fix_systems
            .iter()
            .find(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn get_fix_system_mut(&mut self, plugin: &Ident, name: &Ident) -> Option<&mut Item> {
        self.fix_systems
            .iter_mut()
            .find(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn has_fix_system(&self, plugin: &Ident, name: &Ident) -> bool {
        self.fix_systems
            .iter()
            .any(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn add_fix_system(&mut self, plugin: &Ident, name: &Ident, enabled: bool) {
        if !self.has_fix_system(plugin, name) {
            self.fix_systems.push(Item {
                plugin: plugin.to_buf(),
                name: name.to_buf(),
                enabled,
            });
        }
    }

    pub fn get_filter(&self, plugin: &Ident, name: &Ident) -> Option<&Item> {
        self.filters
            .iter()
            .find(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn get_filter_mut(&mut self, plugin: &Ident, name: &Ident) -> Option<&mut Item> {
        self.filters
            .iter_mut()
            .find(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn has_filter(&self, plugin: &Ident, name: &Ident) -> bool {
        self.filters
            .iter()
            .any(|s| *s.plugin == *plugin && *s.name == *name)
    }

    pub fn add_filter(&mut self, plugin: &Ident, name: &Ident, enabled: bool) {
        if !self.has_filter(plugin, name) {
            self.filters.push(Item {
                plugin: plugin.to_buf(),
                name: name.to_buf(),
                enabled,
            });
        }
    }
}

mod plugins_serde {
    use std::fmt;

    use serde::ser::{SerializeMap, Serializer};

    use crate::IdentBuf;

    use super::{Dependency, Plugin};

    pub fn serialize<S>(plugins: &Vec<Plugin>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_map(Some(plugins.len()))?;

        #[derive(serde::Serialize)]
        struct PluginSer<'a> {
            enabled: bool,
            #[serde(flatten)]
            dep: &'a Dependency,
        }

        for plugin in plugins {
            serializer.serialize_entry(
                &plugin.name,
                &PluginSer {
                    enabled: plugin.enabled,
                    dep: &plugin.dep,
                },
            )?;
        }

        serializer.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Plugin>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(VisitPlugins)
    }

    struct VisitPlugins;

    impl<'de> serde::de::Visitor<'de> for VisitPlugins {
        type Value = Vec<Plugin>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("map of plugins")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            #[derive(serde::Deserialize)]
            struct PluginDe {
                enabled: bool,
                #[serde(flatten)]
                dep: Dependency,
            }

            let mut plugins = Vec::new();

            while let Some((name, plugin)) = map.next_entry::<IdentBuf, PluginDe>()? {
                plugins.push(Plugin {
                    name,
                    enabled: plugin.enabled,
                    dep: plugin.dep.clone(),
                });
            }

            Ok(plugins)
        }
    }
}

pub(super) fn serialize_manifest(manifest: &ProjectManifest) -> Result<String, toml::ser::Error> {
    use serde::Serialize;
    use std::fmt::Write;

    let mut output = String::new();
    // output.push_str("name = ");

    // manifest
    //     .name
    //     .serialize(toml::ser::ValueSerializer::new(&mut output))?;

    // if let Some(engine) = &manifest.engine {
    //     #[derive(serde::Serialize)]
    //     struct SerEngine<'a> {
    //         engine: &'a Dependency,
    //     }
    //     SerEngine { engine }.serialize(toml::ser::Serializer::new(&mut output))?;
    // }

    // if !manifest.plugins.is_empty() {
    //     output.push_str("\n[plugins]");

    //     for plugin in &manifest.plugins {
    //         #[derive(serde::Serialize)]
    //         struct PluginSer<'a> {
    //             #[serde(flatten)]
    //             dep: &'a Dependency,
    //             enabled: bool,
    //         }

    //         write!(output, "\n{} = ", plugin.name);
    //         PluginSer {
    //             dep: &plugin.dep,
    //             enabled: plugin.enabled,
    //         }
    //         .serialize(toml::ser::ValueSerializer::new(&mut output))?;
    //     }
    // }

    manifest.serialize(toml::Serializer::new(&mut output))?;

    Ok(output)
}
