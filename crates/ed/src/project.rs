//! Data definition for the project.
//!

use std::{
    fs, io,
    ops::{Deref, DerefMut},
    path::Path,
};

use arcana::{
    error::{fail, Error},
    render::RenderGraphId,
    Ident,
};
use hashbrown::{HashMap, HashSet};

use super::{filters::Funnel, render::RenderGraph, systems::SystemGraph};

pub use arcana_project::{
    is_available, new_plugin_crate, BuildProcess, Dependency, Plugin, ProjectManifest,
};

pub struct Project {
    pub inner: arcana_project::Project,
    pub data: internal::ProjectData,
}

mod internal {
    use super::*;

    /// In combination with `ProjectManifest` this defines the project completely.
    /// This includes enabled plugins, filter chain, system graph, asset collections, etc
    #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
    pub struct ProjectData {
        /// Set of enabled plugins.
        pub enabled_plugins: HashSet<Ident>,

        /// Systems graph.
        pub systems: SystemGraph,

        /// Event funnel.
        pub funnel: Funnel,

        /// Render graphs.
        pub render_graphs: HashMap<RenderGraphId, RenderGraph>,
    }
}

impl Deref for Project {
    type Target = arcana_project::Project;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Project {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Project {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let project = arcana_project::Project::open(path)?;

        let path = project.root_path().join("Arcana.bin");

        let data = match fs::File::open(path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => internal::ProjectData::default(),
            Ok(file) => match serde_json::from_reader(file) {
                Ok(data) => data,
                Err(err) => {
                    return Err(Error::msg(format!(
                        "Failed to deserialize project data: {}",
                        err
                    )));
                }
            },
            Err(err) => {
                return Err(Error::msg(format!(
                    "Failed to open Arcana.bin to load project data: {}",
                    err
                )));
            }
        };

        Ok(Project {
            inner: project,
            data,
        })
    }

    fn sync(&mut self) -> Result<(), Error> {
        use std::io::Write;

        let path = self.inner.root_path().join("Arcana.bin");
        let bak = path.with_extension("bin.bak");

        let _ = std::fs::remove_file(&bak);
        if let Err(err) = std::fs::rename(&path, &bak) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::error!("Failed to backup Arcana.bin: {}", err);
            }
        }

        let mut file = match std::fs::File::create(path) {
            Ok(file) => file,
            Err(err) => {
                fail!("Failed to create Arcana.bin to store project data: {}", err);
            }
        };

        match serde_json::to_string(&self.data) {
            Ok(bytes) => match file.write_all(bytes.as_bytes()) {
                Ok(()) => {}
                Err(err) => {
                    fail!("Failed to write project data: {}", err);
                }
            },
            Err(err) => {
                fail!("Failed to serialize project data: {}", err);
            }
        }

        self.inner.sync()?;
        Ok(())
    }
}
