pub use arcana_assets::import::{Importer, ImporterDesc, ImporterId};
use arcana_intern::Name;

use crate::Location;

/// System information declared by a plugin.
#[derive(Clone, Debug)]
pub struct ImporterInfo {
    /// Unique identified of the importer.
    pub id: ImporterId,

    /// Name of the importer.
    pub name: Name,

    /// Description of the importer.
    pub desc: ImporterDesc,

    /// Location of the importer in the source code.
    pub location: Option<Location>,
}
