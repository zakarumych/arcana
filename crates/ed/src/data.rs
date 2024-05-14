//! Data definition for the project.
//!

use std::io::Write;

use arcana::{project::Project, Ident};
use hashbrown::HashSet;

use crate::{filters::Funnel, render::WorkGraph, systems::SystemGraph};

/// In combination with `ProjectManifest` this defines the project completely.
/// This includes enabled plugins, filter chain, system graph, asset collections, etc
///
/// Stored in the Ed's main `World`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProjectData {
    /// Set of enabled plugins.
    pub enabled_plugins: HashSet<Ident>,

    /// Systems graph.
    pub systems: SystemGraph,

    /// Event funnel.
    pub funnel: Funnel,

    /// Work graph.
    pub workgraph: WorkGraph,
}

impl ProjectData {
    pub fn sync(&mut self, project: &Project) -> miette::Result<()> {
        let path = project.root_path().join("Arcana.bin");
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
                miette::bail!("Failed to create Arcana.bin to store project data: {}", err);
            }
        };

        match serde_json::to_string(self) {
            Ok(bytes) => match file.write_all(bytes.as_bytes()) {
                Ok(()) => Ok(()),
                Err(err) => {
                    miette::bail!("Failed to write project data: {}", err);
                }
            },
            Err(err) => {
                miette::bail!("Failed to serialize project data: {}", err);
            }
        }
    }
}
