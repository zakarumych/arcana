use arcana_intern::Name;
pub use arcana_work_graph::{Job, JobDesc, JobId};

use crate::Location;

/// Job information declared by a plugin.
#[derive(Clone)]
pub struct JobInfo {
    /// Unique identified of the job.
    pub id: JobId,

    /// Name of the job.
    pub name: Name,

    /// Description of the job.
    pub desc: JobDesc,

    /// Location of the filter in the source code.
    pub location: Option<Location>,
}
