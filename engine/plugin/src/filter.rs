pub use arcana_input::{FilterId, InputFilter, IntoInputFilter};
use arcana_intern::Name;

use crate::Location;

/// Filter information declared by a plugin.
#[derive(Clone, Debug)]
pub struct FilterInfo {
    /// Unique identified of the filter.
    pub id: FilterId,

    /// Name of the filter.
    pub name: Name,

    /// Location of the filter in the source code.
    pub location: Option<Location>,
}
