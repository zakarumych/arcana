use arcana_assets::import::{Importer, ImporterId};
use arcana_ecs::{SystemId, system::System};
use arcana_id::Stid;
use arcana_input::{FilterId, InputFilter, IntoInputFilter};
use arcana_work_graph::{Job, JobId};
use edict::system::IntoSystem;
use hashbrown::HashMap;

use crate::ComponentInfo;

/// Active systems, filters, jobs, importers and component descriptors from plugins.
///
/// Retain each contributing plugin library while the hub or any cloned
/// component descriptor is in use.
pub struct PluginsHub {
    pub systems: HashMap<SystemId, Box<dyn System + Send>>,
    pub filters: HashMap<FilterId, Box<dyn InputFilter>>,
    pub jobs: HashMap<JobId, Box<dyn Job>>,
    pub importers: HashMap<ImporterId, Box<dyn Importer>>,
    pub components: HashMap<Stid, ComponentInfo>,
}

impl PluginsHub {
    pub fn new() -> Self {
        PluginsHub {
            systems: HashMap::new(),
            filters: HashMap::new(),
            jobs: HashMap::new(),
            importers: HashMap::new(),
            components: HashMap::new(),
        }
    }

    /// Adds a component descriptor. Panics if its stable ID is already registered.
    pub fn add_component(&mut self, component: ComponentInfo) {
        let id = component.id();
        assert!(
            !self.components.contains_key(&id),
            "Component stable ID {id} is already registered"
        );
        self.components.insert(id, component);
    }

    /// Adds a system from a plugin to the hub.
    pub fn add_system<S, M>(&mut self, id: SystemId, system: S)
    where
        S: IntoSystem<M>,
    {
        self.systems.insert(id, Box::new(system.into_system()));
    }

    /// Adds a filter from a plugin to the hub.
    pub fn add_filter<F, M>(&mut self, id: FilterId, filter: F)
    where
        F: IntoInputFilter<M>,
    {
        self.filters
            .insert(id, Box::new(filter.into_input_filter()));
    }

    /// Adds a job from a plugin to the hub.
    pub fn add_job(&mut self, id: JobId, job: impl Job) {
        self.jobs.insert(id, Box::new(job));
    }

    /// Adds a importer from a plugin to the hub.
    pub fn add_importer(&mut self, id: ImporterId, importer: impl Importer) {
        self.importers.insert(id, Box::new(importer));
    }
}
