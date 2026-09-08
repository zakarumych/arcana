//! Data definition for the project.
//!

use std::{fs, io};

pub use arcana::project::{
    BuildProcess, Dependency, Plugin, Profile, Project, is_path_available, new_plugin_crate,
};
use arcana::{
    Name,
    error::{Error, fail},
    hash::{HashMap, HashSet},
    render::RenderGraphId,
};

use crate::{error::ModalErrors, filters::FilterOrder, render::RenderGraph, systems::SystemGraph};

/// In combination with `ProjectManifest` this defines the project completely.
/// This includes enabled plugins, filter chain, system graph, asset collections, etc
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProjectData {
    /// Set of enabled plugins.
    pub enabled_plugins: HashSet<Name>,

    /// System graph.
    pub systems: SystemGraph,

    /// Filter order.
    pub filters: FilterOrder,

    /// Render graphs.
    pub renders: HashMap<RenderGraphId, RenderGraph>,
}

impl ProjectData {
    pub fn load(project: &Project) -> Result<Self, Error> {
        let path = project.root_path().join("Arcana.bin");

        match fs::File::open(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ProjectData::default()),
            Ok(file) => match serde_json::from_reader(file) {
                Ok(data) => Ok(data),
                Err(error) => Err(Error::msg(format!(
                    "Failed to deserialize project data: {}",
                    error
                ))),
            },
            Err(error) => Err(Error::msg(format!(
                "Failed to open Arcana.bin to load project data: {}",
                error
            ))),
        }
    }

    fn save(&self, project: &Project) -> Result<(), Error> {
        use std::io::Write;

        let path = project.root_path().join("Arcana.bin");
        let bak = path.with_extension("bin.bak");

        let _ = std::fs::remove_file(&bak);
        if let Err(error) = std::fs::rename(&path, &bak) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::error!("Failed to backup Arcana.bin: {}", error);
            }
        }

        let mut file = match std::fs::File::create(path) {
            Ok(file) => file,
            Err(error) => {
                fail!(
                    "Failed to create Arcana.bin to store project data: {}",
                    error
                );
            }
        };

        match serde_json::to_string(self) {
            Ok(bytes) => match file.write_all(bytes.as_bytes()) {
                Ok(()) => {}
                Err(error) => {
                    fail!("Failed to write project data: {}", error);
                }
            },
            Err(error) => {
                fail!("Failed to serialize project data: {}", error);
            }
        }

        drop(file);
        project.save()?;
        Ok(())
    }

    pub fn save_in_ui(&self, project: &Project, ui: &egui::Ui, modal_error: &mut ModalErrors) {
        if let Err(error) = self.save(project) {
            modal_error.push_error(ui.viewport_id(), "Project sync error", error);
        }
    }
}
