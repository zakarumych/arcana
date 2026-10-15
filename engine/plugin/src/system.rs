pub use arcana_ecs::{
    SystemId,
    system::{IntoSystem, System},
};
use arcana_intern::Name;

use crate::Location;

/// System information declared by a plugin.
#[derive(Clone, Debug)]
pub struct SystemInfo {
    /// Unique identified of the system.
    pub id: SystemId,

    /// Name of the system.
    pub name: Name,

    /// Location of the system in the source code.
    pub location: Option<Location>,
}
