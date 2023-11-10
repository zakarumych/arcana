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
#[derive(Debug, Clone, Hash)]
pub struct Plugin {
    /// Name of the plugin.
    pub name: IdentBuf,

    /// Dependency that can be added to the package to link with the plugin.
    pub dep: Dependency,

    /// Whether this plugin is enabled.
    /// Disabled plugins are not initialized.
    pub enabled: bool,
}

pub struct System {
    /// Plugin that registers this system.
    pub plugin: IdentBuf,

    /// Name of the system.
    pub name: IdentBuf,

    /// Whether this system is enabled.
    /// Disabled systems are not added to scheduler.
    pub enabled: bool,
}

impl serde::Serialize for System {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct SerSystem<'a> {
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
                &SerSystem {
                    plugin: &self.plugin,
                    name: &self.name,
                    enabled: self.enabled,
                },
                serializer,
            )
        }
    }
}

impl<'de> serde::Deserialize<'de> for System {
    fn deserialize<D>(deserializer: D) -> Result<System, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct DeSystem {
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

            Ok(System {
                plugin: plugin.to_buf(),
                name: name.to_buf(),
                enabled: !disabled,
            })
        } else {
            let system: DeSystem = serde::Deserialize::deserialize(deserializer)?;
            Ok(System {
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
#[derive(serde::Serialize, serde::Deserialize)]
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
    #[serde(skip_serializing_if = "Vec::is_empty", default, with = "systems_serde")]
    pub systems: Vec<System>,
}

impl ProjectManifest {
    pub fn get_plugin(&self, name: &Ident) -> Option<&Plugin> {
        self.plugins.iter().find(|p| &p.name == name)
    }

    pub fn get_plugin_mut(&mut self, name: &Ident) -> Option<&mut Plugin> {
        self.plugins.iter_mut().find(|p| &p.name == name)
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
            dep: &'a Dependency,
            enabled: bool,
        }

        for plugin in plugins {
            serializer.serialize_entry(
                &plugin.name,
                &PluginSer {
                    dep: &plugin.dep,
                    enabled: plugin.enabled,
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
                dep: Dependency,
                enabled: bool,
            }

            let mut plugins = Vec::new();

            while let Some((name, plugin)) = map.next_entry::<IdentBuf, PluginDe>()? {
                plugins.push(Plugin {
                    name,
                    dep: plugin.dep.clone(),
                    enabled: plugin.enabled,
                });
            }

            Ok(plugins)
        }
    }
}

mod systems_serde {
    use std::fmt;

    use serde::ser::{SerializeMap, SerializeSeq, Serializer};

    use crate::IdentBuf;

    use super::System;

    fn is_true(b: &bool) -> bool {
        *b
    }

    fn default_true() -> bool {
        true
    }

    pub fn serialize<S>(systems: &Vec<System>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_seq(Some(systems.len()))?;

        for system in systems {
            if system.enabled {
                serializer.serialize_element(&format!("{}:{}", system.plugin, system.name));
            } else {
                serializer.serialize_element(&format!("!{}:{}", system.plugin, system.name));
            }
        }

        serializer.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<System>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(VisitSystems)
    }

    struct VisitSystems;

    impl<'de> serde::de::Visitor<'de> for VisitSystems {
        type Value = Vec<System>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("list of system ids")
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut plugins = Vec::new();

            while let Some(id) = seq.next_element::<String>()? {
                let mut id_ref = &id[..];
                if id.starts_with('!') {
                    id_ref = &id[1..];
                }
            }

            Ok(plugins)
        }
    }
}
