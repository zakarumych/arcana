use std::path::Path;

use crate::{
    dependency::Dependency,
    ident::{Ident, IdentBuf},
    plugin::Plugin,
};

// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
// pub struct Item {
//     /// Plugin that registers this system.
//     pub plugin: IdentBuf,

//     /// Name of the system.
//     pub name: IdentBuf,

//     /// Whether this system is enabled.
//     /// Disabled systems are not added to scheduler.
//     pub enabled: bool,
// }

// impl serde::Serialize for Item {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         #[derive(serde::Serialize)]
//         struct SerItem<'a> {
//             plugin: &'a Ident,
//             name: &'a Ident,
//             enabled: bool,
//         }

//         if serializer.is_human_readable() {
//             if self.enabled {
//                 serializer.serialize_str(&format!("{}:{}", self.plugin, self.name))
//             } else {
//                 serializer.serialize_str(&format!("!{}:{}", self.plugin, self.name))
//             }
//         } else {
//             serde::Serialize::serialize(
//                 &SerItem {
//                     plugin: &self.plugin,
//                     name: &self.name,
//                     enabled: self.enabled,
//                 },
//                 serializer,
//             )
//         }
//     }
// }

// impl<'de> serde::Deserialize<'de> for Item {
//     fn deserialize<D>(deserializer: D) -> Result<Item, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         #[derive(serde::Deserialize)]
//         struct DeItem {
//             plugin: IdentBuf,
//             name: IdentBuf,
//             enabled: bool,
//         }

//         if deserializer.is_human_readable() {
//             let expected =
//                 || serde::de::Error::custom("Expected <plugin:system> or <!plugin:system>");

//             let flag_id = String::deserialize(deserializer)?;
//             let mut id = &flag_id[..];
//             let disabled = id.starts_with('!');
//             if disabled {
//                 id = &id[1..];
//             }
//             let (plugin, name) = id.split_once(':').ok_or_else(expected)?;
//             let plugin = Ident::from_str(plugin).map_err(|_| expected())?;
//             let name = Ident::from_str(name).map_err(|_| expected())?;

//             Ok(Item {
//                 plugin: plugin.to_buf(),
//                 name: name.to_buf(),
//                 enabled: !disabled,
//             })
//         } else {
//             let system: DeItem = serde::Deserialize::deserialize(deserializer)?;
//             Ok(Item {
//                 plugin: system.plugin,
//                 name: system.name,
//                 enabled: system.enabled,
//             })
//         }
//     }
// }

/// Project manifest.
/// Contains information about project, dependencies, systems order, etc.
/// Usually put into `Arcana.toml` file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectManifest {
    /// Name of the project.
    pub name: IdentBuf,

    /// How to fetch engine dependency.
    /// Defaults to `Dependency::Crates(version())`.
    pub engine: Dependency,

    /// List of plugin libraries this project depends on.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub plugins: Vec<Plugin>,
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

    pub fn remove_plugin_idx(&mut self, idx: usize) {
        self.plugins.remove(idx);
    }
}

pub(super) fn serialize_manifest(manifest: &ProjectManifest) -> Result<String, toml::ser::Error> {
    use serde::Serialize;
    use std::fmt::Write;

    let mut output = String::new();

    manifest.serialize(toml::Serializer::new(&mut output))?;

    Ok(output)
}
