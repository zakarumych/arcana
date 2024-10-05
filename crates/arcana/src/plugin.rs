use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use arcana_names::{Ident, Name};
use arcana_project::Dependency;
use edict::{
    system::{IntoSystem, System},
    world::World,
};
use hashbrown::HashMap;

use crate::assets::import::{Importer, ImporterId};
use crate::code::{CodeDesc, CodeNodeId, FlowCode, PureCode};
use crate::events::EventId;
use crate::input::{FilterId, InputFilter, IntoInputFilter};
use crate::work::{Job, JobDesc, JobId};
use crate::{make_id, Stid};

make_id! {
    /// ID of the ECS system.
    pub SystemId;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

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

/// Code information declared by a plugin.
#[derive(Clone)]
pub struct CodeInfo {
    /// Unique identified of the code.
    pub id: CodeNodeId,

    /// Name of the code.
    pub name: Name,

    /// Description of the code.
    pub desc: CodeDesc,

    /// Location of the filter in the source code.
    pub location: Option<Location>,
}

/// Job information declared by a plugin.
#[derive(Clone)]
pub struct EventInfo {
    /// Unique identified of the event.
    pub id: EventId,

    /// Name of the event.
    pub name: Name,

    /// List of event values.
    pub values: Vec<Stid>,

    /// Location of the filter in the source code.
    pub location: Option<Location>,
}

/// System information declared by a plugin.
#[derive(Clone, Debug)]
pub struct ImporterInfo {
    /// Unique identified of the importer.
    pub id: ImporterId,

    /// Name of the importer.
    pub name: Name,

    /// Location of the importer in the source code.
    pub location: Option<Location>,
}

/// Active plugin hub contains
/// systems, filters and jobs
/// populated from plugins.
pub struct PluginsHub {
    pub systems: HashMap<SystemId, Box<dyn System + Send>>,
    pub filters: HashMap<FilterId, Box<dyn InputFilter>>,
    pub jobs: HashMap<JobId, Box<dyn Job>>,
    pub pure_fns: HashMap<CodeNodeId, PureCode>,
    pub flow_fns: HashMap<CodeNodeId, FlowCode>,
    pub importers: HashMap<ImporterId, Box<dyn Importer>>,
}

impl PluginsHub {
    pub fn new() -> Self {
        PluginsHub {
            systems: HashMap::new(),
            filters: HashMap::new(),
            jobs: HashMap::new(),
            pure_fns: HashMap::new(),
            flow_fns: HashMap::new(),
            importers: HashMap::new(),
        }
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

    /// Adds a pure fn from a plugin to the hub.
    pub fn add_pure_fn(&mut self, id: CodeNodeId, code: PureCode) {
        self.pure_fns.insert(id, code);
    }

    /// Adds a flow fn from a plugin to the hub.
    pub fn add_flow_fn(&mut self, id: CodeNodeId, code: FlowCode) {
        self.flow_fns.insert(id, code);
    }
}

#[doc(hidden)]
static GLOBAL_LINK_CHECK: AtomicBool = AtomicBool::new(false);

// Re-exported as `arcana_linked` from plugins lib.
#[doc(hidden)]
pub fn running_arcana_instance_check(check: &AtomicBool) -> bool {
    (check as *const _ == &GLOBAL_LINK_CHECK as *const _)
        && GLOBAL_LINK_CHECK.load(::core::sync::atomic::Ordering::SeqCst)
}

#[doc(hidden)]
pub fn check_arcana_instance(arcana_linked: fn(check: &AtomicBool) -> bool) -> bool {
    arcana_linked(&GLOBAL_LINK_CHECK)
}

#[doc(hidden)]
pub fn set_running_arcana_instance() {
    tracing::info!("Arcana instance is running");
    let old = GLOBAL_LINK_CHECK.swap(true, ::core::sync::atomic::Ordering::SeqCst);
    assert!(!old, "Arcana instance is already running");
}

#[inline(never)]
#[cold]
pub fn unknown_dependency() -> ! {
    panic!("Unknown dependency")
}

/// Plugin for Arcana engine.
/// It allows bundling systems and resources together into a single unit
/// that can be initialized at once.
///
/// User may wish to use plugin and wrap their systems and resources
/// into plugins.
/// Ed uses this to load plugins from libraries.
///
/// A crate must use `declare_plugin!` macro to declare it is a plugin.
#[derive(Default)]
pub struct ArcanaPlugin {
    location: Option<PathBuf>,
    dependencies: Vec<(Ident, Dependency)>,
    filters: Vec<FilterInfo>,
    systems: Vec<SystemInfo>,
    jobs: Vec<JobInfo>,
    events: Vec<EventInfo>,
    codes: Vec<CodeInfo>,
    importers: Vec<ImporterInfo>,
    fill_hub: Vec<fn(&mut PluginsHub)>,
    init: Vec<fn(&mut World)>,
}

impl ArcanaPlugin {
    pub fn new() -> Self {
        ArcanaPlugin::default()
    }

    pub fn add_dependency(&mut self, name: Ident, dep: Dependency) {
        self.dependencies.push((name, dep));
    }

    pub fn add_filter(&mut self, info: FilterInfo, add: fn(&mut PluginsHub)) {
        self.filters.push(info);
        self.fill_hub.push(add);
    }

    pub fn add_system(&mut self, info: SystemInfo, add: fn(&mut PluginsHub)) {
        self.systems.push(info);
        self.fill_hub.push(add);
    }

    pub fn add_job(&mut self, info: JobInfo, add: fn(&mut PluginsHub)) {
        self.jobs.push(info);
        self.fill_hub.push(add);
    }

    pub fn add_event(&mut self, info: EventInfo) {
        self.events.push(info);
    }

    pub fn add_code(&mut self, info: CodeInfo, add: fn(&mut PluginsHub)) {
        self.codes.push(info);
        self.fill_hub.push(add);
    }

    pub fn add_importer(&mut self, info: ImporterInfo, add: fn(&mut PluginsHub)) {
        self.importers.push(info);
        self.fill_hub.push(add);
    }

    pub fn add_init(&mut self, add: fn(&mut World)) {
        self.init.push(add);
    }
}

impl ArcanaPlugin {
    pub fn location(&self) -> Option<PathBuf> {
        self.location.clone()
    }

    pub fn dependencies(&self) -> Vec<(Ident, Dependency)> {
        self.dependencies.clone()
    }

    pub fn filters(&self) -> Vec<FilterInfo> {
        self.filters.clone()
    }

    pub fn systems(&self) -> Vec<SystemInfo> {
        self.systems.clone()
    }

    pub fn jobs(&self) -> Vec<JobInfo> {
        self.jobs.clone()
    }

    pub fn events(&self) -> Vec<EventInfo> {
        self.events.clone()
    }

    pub fn codes(&self) -> Vec<CodeInfo> {
        self.codes.clone()
    }

    pub fn init(&self, world: &mut World, hub: &mut PluginsHub) {
        for fill in &self.fill_hub {
            fill(hub);
        }

        for init in &self.init {
            init(world);
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! plugin_dependency_kind {
    ($name:ident) => {
        $crate::ident!($name), $name::arcana_plugin::dependency()
    };
    ($name:ident path) => {
        $name::arcana_plugin::path_dependency()
    };
    ($name:ident ...) => {
        $name::arcana_plugin::path_dependency()
    };
    ($name:ident git = $git:literal) => {
        $crate::project::Dependency::Git {
            git: String::from($git),
            branch: None,
        }
    };
    ($name:ident { git = $git:literal, branch = $branch:literal }) => {
        $crate::project::Dependency::Git {
            git: String::from($git),
            branch: Some(String::from($branch)),
        }
    };
}

/// Plugin crate must use this macro at the root module to declare it is a plugin.
#[doc(hidden)]
#[macro_export]
macro_rules! declare_plugin {
    ($([$($dependency:ident $($kind:tt)*)+])?) => {
        #[doc(hidden)]
        pub mod arcana_plugin {
            pub fn dependency() -> $crate::project::Dependency {
                $crate::project::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
            }

            pub fn path_dependency() -> $crate::project::Dependency {
                $crate::project::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap()
            }

            pub fn get() -> $crate::plugin::ArcanaPlugin {
                // Safety: This value is accessed mutably at cdylib load time.
                // Afterwards it can only be accessed immutably here.
                unsafe { ARCANA_PLUGIN_REGISTRY.plugin() }
            }

            pub static mut ARCANA_PLUGIN_REGISTRY: $crate::plugin::init::Registry =
                ::arcana::plugin::init::Registry::new();
        }

        $(
            $crate::plugin_ctor_add!(plugin => {
                $(
                    plugin.add_dependency($crate::ident!($dependency), $crate::plugin_dependency_kind!($dependency $($kind)*));
                )+
            });
        )*
    };
}

#[doc(hidden)]
pub mod init {
    use std::collections::BTreeMap;

    use super::*;

    pub use ::ctor::ctor;

    #[derive(Clone, Copy)]
    pub struct CtorNode {
        ctor: fn(&mut ArcanaPlugin),
        next: Option<&'static CtorNode>,
    }

    impl CtorNode {
        pub const fn new(ctor: fn(&mut ArcanaPlugin)) -> Self {
            CtorNode { ctor, next: None }
        }
    }

    pub struct Registry {
        manifest_dir: &'static str,
        list: Option<&'static CtorNode>,
        dependencies: BTreeMap<Ident, Dependency>,
    }

    impl Registry {
        pub const fn new() -> Self {
            Registry {
                manifest_dir: env!("CARGO_MANIFEST_DIR"),
                list: None,
                dependencies: BTreeMap::new(),
            }
        }

        pub fn register(&mut self, node: &'static mut CtorNode) {
            node.next = self.list;
            self.list = Some(node);
        }

        pub fn plugin(&mut self) -> ArcanaPlugin {
            let mut plugin = ArcanaPlugin::new();
            plugin.dependencies = self
                .dependencies
                .iter()
                .map(|(n, d)| (*n, d.clone()))
                .collect();

            plugin.location = Some(PathBuf::from(self.manifest_dir));

            let mut node = self.list;
            while let Some(n) = node {
                (n.ctor)(&mut plugin);
                node = n.next;
            }

            plugin
        }
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! pkg_name {
        () => {
            $crate::Ident::from_str(env!("CARGO_PKG_NAME")).unwrap()
        };
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! plugin_ctor_add {
        ($plugin:ident => $($code:tt)*) => {
            const _: () = {
                #[$crate::plugin::init::ctor]
                fn add() {
                    static mut CTOR_NODE: $crate::plugin::init::CtorNode =
                        $crate::plugin::init::CtorNode::new(
                            |$plugin: &mut $crate::plugin::ArcanaPlugin| {
                                // At this point cdylib is initialized and any code can be executed.
                                $($code)*
                            },
                        );

                    // Safety: This code is executed at cdylib load time
                    // sequentially with other ctors.
                    unsafe {
                        crate::arcana_plugin::ARCANA_PLUGIN_REGISTRY.register(&mut CTOR_NODE);
                    }
                }
            };
        };
    }
}
