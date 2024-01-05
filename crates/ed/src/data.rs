//! Data definition for the project.
//!

use std::{cell::RefCell, rc::Rc};

use arcana_project::IdentBuf;
use hashbrown::HashSet;

use crate::{systems::SystemGraph, workgraph::WorkGraph};

/// In combination with `ProjectManifest` this defines the project completely.
/// This includes enabled plugins, filter chain, system graph etc.
///
/// Stored in the Ed's main `World`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProjectData {
    /// Set of enabled plugins.
    pub enabled_plugins: HashSet<IdentBuf>,

    /// Systems graph.
    pub systems: Rc<RefCell<SystemGraph>>,

    pub workgraph: Rc<RefCell<WorkGraph>>,
}
