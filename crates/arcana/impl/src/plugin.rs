use std::{any::Any, borrow::Cow, sync::atomic::AtomicBool};

use arcana_project::{Dependency, Ident, IdentBuf};
use edict::{IntoSystem, Scheduler, System, World};
use hashbrown::HashMap;

use crate::events::EventFilter;
use crate::id::Id;
use crate::make_id;
use crate::work::{Job, JobDesc};

make_id!(pub SystemId);
make_id!(pub FilterId);
make_id!(pub JobId);

/// System information declared by a plugin.
#[derive(Clone)]
pub struct SystemInfo {
    /// Unique identified of the system.
    pub id: SystemId,

    /// Name of the system.
    pub name: Cow<'static, Ident>,
}

/// Filter information declared by a plugin.
#[derive(Clone)]
pub struct FilterInfo {
    /// Unique identified of the filter.
    pub id: FilterId,

    /// Name of the filter.
    pub name: Cow<'static, Ident>,
}

/// Job information declared by a plugin.
#[derive(Clone)]
pub struct JobInfo {
    /// Unique identified of the job.
    pub id: JobId,

    /// Name of the job.
    pub name: Cow<'static, Ident>,

    /// Description of the job.
    pub desc: JobDesc,
}

/// Active plugin hub contains
/// systems, filters and jobs
/// populated from plugins.
pub struct PluginsHub {
    pub systems: HashMap<SystemId, Box<dyn System + Send>>,
    pub filters: HashMap<FilterId, Box<dyn EventFilter>>,
    pub jobs: HashMap<JobId, Box<dyn FnMut() -> Box<dyn Job>>>,
}

impl PluginsHub {
    pub(crate) fn new() -> Self {
        PluginsHub {
            systems: HashMap::new(),
            filters: HashMap::new(),
            jobs: HashMap::new(),
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
    pub fn add_filter(&mut self, id: FilterId, filter: impl EventFilter + 'static) {
        self.filters.insert(id, Box::new(filter));
    }

    /// Adds a job from a plugin to the hub.
    pub fn add_job<F, J>(&mut self, id: JobId, make_job: F)
    where
        F: Fn() -> J + 'static,
        J: Job + 'static,
    {
        self.jobs.insert(id, Box::new(move || Box::new(make_job())));
    }
}

#[macro_export]
macro_rules! plugin_init {
    (
        $(systems: [$($system:ident),* $(,)?])?
        $(filters: [$($filter:ident),* $(,)?])?
        $(jobs: [$($make_job:ident),* $(,)?])?
        => $hub:ident
    ) => {{
        $($hub.add_system($crate::hash_id!(::core::module_path!(), ::core::stringify!($system)), $system))*
        $($hub.add_filter($crate::hash_id!(::core::module_path!(), ::core::stringify!($filter)), $filter))*
        $($hub.add_job($crate::hash_id!(::core::module_path!(), ::core::stringify!($make_job)), $make_job))*
    }};
}

/// Plugin protocol for Bob engine.
/// It allows bundling systems and resources together into a single unit
/// that can be initialized at once.
///
/// User may wish to use plugin protocol and wrap their systems and resources
/// into plugins.
/// Ed uses this protocol to load plugins from libraries.
///
/// A crate that defines a plugin must export a function `arcana_plugin` that
/// returns plugin static reference to plugin instance.
///
/// The easiest way to do this is to use [`export_arcana_plugin!`](`export_arcana_plugin`) macro.
pub trait ArcanaPlugin: Any + Sync {
    /// Returns list of plugin names this plugin depends on.
    /// Dependencies must be initialized first and deinitialized last.
    fn dependencies(&self) -> Vec<(&'static Ident, Dependency)> {
        Vec::new()
    }

    /// Returns list of named event filters.
    fn filters(&self) -> Vec<FilterInfo> {
        Vec::new()
    }

    /// Returns list of systems.
    fn systems(&self) -> Vec<SystemInfo> {
        Vec::new()
    }

    /// Returns list of systems.
    fn jobs(&self) -> Vec<JobInfo> {
        Vec::new()
    }

    /// Registers components and resources.
    /// Perform any other initialization of the world.
    /// Returns constructed systems and event filters.
    fn init(&self, world: &mut World, hub: &mut PluginsHub) {
        let _ = (world, hub);
    }

    /// De-initializes world.
    /// Removes resources that belongs to this plugin.
    /// This method is called when game instance is closed,
    /// plugin is disabled or replaced with another version.
    fn deinit(&self, world: &mut World) {}

    /// Returns true if this plugin can be replaced with the `updated` plugin.
    /// The updated plugin should typically be a newer version of the same plugin.
    ///
    /// Plugins may conservatively return `false` here.
    /// And then they may not implement `dump` and `load` methods.
    fn compatible(&self, updated: &dyn ArcanaPlugin) -> bool {
        false
    }

    /// Dump state of the world known to this plugin.
    /// This method is called when the plugin is reloaded with updated code
    /// before `deinit` method.
    /// New version, if compatible, will load the state from the dump.
    fn dump(&self, world: &World, scratch: &mut [u8]) -> usize {
        0
    }

    /// Load state of the world known to this plugin dumped by previous version.
    fn load(&self, world: &mut World, scratch: &[u8]) {
        unimplemented!()
    }

    #[doc(hidden)]
    fn __running_arcana_instance_check(&self, check: &AtomicBool) -> bool {
        (check as *const _ == &GLOBAL_CHECK as *const _)
            && GLOBAL_CHECK.load(::core::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(feature = "ed")]
    #[doc(hidden)]
    fn __eq(&self, other: &dyn ArcanaPlugin) -> bool {
        self.type_id() == other.type_id()
    }
}

/// List of loaded plugins.
pub struct LinkedPlugins {
    plugins: Vec<&'static dyn ArcanaPlugin>,
}

#[doc(hidden)]
#[macro_export]
macro_rules! name_or_expr {
    ($_:ident: $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        $name
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! get_dependency {
    ($name:ident crate) => {{
        $name::dependency()
    }};
    ($name:ident ...) => {{
        $name::path_dependency()
    }};
    ($name:ident path) => {{
        $name::path_dependency()
    }};
    ($name:ident { git = $git:literal $( branch = $branch:literal )? }) => {{
        let mut branch = None;
        $(branch = Some($branch);)?
        $name::git_dependency($git, branch)
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! init_resources {
    ($_:ident as $world:ident [$($resource:expr),* $(,)?]) => {
        $(
            let resource = $resource;
            $world.insert_resource($resource);
        )*
    };
    ($world:ident [$($resource:expr),* $(,)?]) => {
        $(
            $world.insert_resource($resource);
        )*
    };
}

/// Exports plugins from a crate.
/// Use this macro in the crate's root.
/// It may be used only once per crate.
/// All plugins must be listed in the macro call to be exported.
///
/// # Example
///
/// ```
/// # use arcana::{export_arcana_plugin, plugin::{ArcanaPlugin, PluginInit}, project:ident, edict::{World, Scheduler, Res}};
/// // Define a plugin.
/// struct MyBobPlugin;
///
/// impl ArcanaPlugin for MyBobPlugin {
///   fn init(&self, world: &mut World) -> PluginInit {
///     world.insert_resource("world".to_string());
///     PluginInit::new().with_system(ident!(hello), |r: Res<String>| println!("Hello, {}!", &*r));
///   }
/// }
///
/// // Export it.
/// export_arcana_plugin!(MyBobPlugin);
/// ```
///
/// ```
/// # use arcana::{export_arcana_plugin, plugin::ArcanaPlugin, edict::{World, Scheduler, Res}};
/// // Export implicitly define plugin.
/// export_arcana_plugin!(MyBobPlugin {
///   resources: ["world".to_string()],
///   systems: [hello: |r: Res<String>| println!("Hello, {}!", &*r)],
/// });
/// ```
#[macro_export]
macro_rules! export_arcana_plugin {
    ($plugin:ident $({
        $(dependencies: [$($dependency:ident $dep_kind:tt),+ $(,)?] $(,)?)?
        $(resources: [$($resource:expr),+ $(,)?] $(,)?)?
        $(components: [$($component:ty),+ $(,)?] $(,)?)?
        $(systems: [$($system_name:ident $(: $system:expr)?),+ $(,)?] $(,)?)?
        $(filters: [$($filter_name:ident $(: $filter:expr)?),+ $(,)?] $(,)?)?
        $(jobs: [$($job_name:ident, $job_desc:expr => $make_job:block),+ $(,)?] $(,)?)?
        $(in $world:ident $(: $world_type:ty)? $( => { $($init:tt)* })?)?
    })?) => {
        $(
            #[allow(non_camel_case)]
            struct $plugin;

            impl $crate::plugin::ArcanaPlugin for $plugin {
                $(
                    fn dependencies(&self) -> Vec<(&'static $crate::project::Ident, $crate::project::Dependency)> {
                        vec![$(
                            ($crate::project::ident!($dependency), $crate::get_dependency!($dependency $dep_kind)),
                        )+]
                    }
                )*

                $(
                    fn systems(&self) -> Vec<$crate::plugin::SystemInfo> {
                        vec![$(
                            $crate::plugin::SystemInfo {
                                id: $crate::hash_id!(::core::module_path!(), ::core::stringify!($system_name)),
                                name: ::std::borrow::Cow::Borrowed($crate::project::ident!($system_name)),
                            },
                        )+]
                    }
                )?

                $(
                    fn filters(&self) -> Vec<$crate::plugin::FilterInfo> {
                        vec![$(
                            $crate::plugin::FilterInfo {
                                id: $crate::hash_id!(::core::module_path!(), ::core::stringify!($filter_name)),
                                name: ::std::borrow::Cow::Borrowed($crate::project::ident!($filter_name)),
                            },
                        )+]
                    }
                )*

                $(
                    fn jobs(&self) -> Vec<$crate::plugin::JobInfo> {
                        vec![$(
                            $crate::plugin::JobInfo {
                                id: $crate::hash_id!(::core::module_path!(), ::core::stringify!($job_name)),
                                name: ::std::borrow::Cow::Borrowed($crate::project::ident!($job_name)),
                                desc: $job_desc,
                            },
                        )+]
                    }
                )?

                fn init(&self, world: &mut $crate::edict::World, hub: &mut $crate::plugin::PluginsHub) {
                    $($(world.ensure_component_registered::<$component>();)*)?

                    $(
                        let $world $($world_type)? = &mut *world;
                        $($($init)*)?
                    )?

                    $($(
                        hub.add_system($crate::hash_id!(::core::module_path!(), ::core::stringify!($system_name)), $crate::name_or_expr!($system_name $(: $system)?));
                    )+)?

                    $($(
                        hub.add_filter($crate::hash_id!(::core::module_path!(), ::core::stringify!($filter_name)), $crate::name_or_expr!($filter_name $(: $filter)?));
                    )+)?

                    $($(
                        hub.add_job($crate::hash_id!(::core::module_path!(), ::core::stringify!($job_name)), $make_job);
                    )+)?

                    $crate::init_resources! {
                        world $(as $world)?
                        [$($($resource),+)?]
                    }
                }
            }
        )?

        pub const fn __arcana_plugin() -> &'static dyn $crate::plugin::ArcanaPlugin {
            &$plugin
        }

        pub fn dependency() -> $crate::project::Dependency {
            $crate::project::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
        }

        pub fn git_dependency(git: &str, branch: Option<&str>) -> $crate::project::Dependency {
            $crate::project::Dependency::Git {
                git: git.to_owned(),
                branch: branch.map(str::to_owned),
            }
        }

        pub fn path_dependency() -> $crate::project::Dependency {
            $crate::project::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap()
        }
    };
}

#[doc(hidden)]
pub static GLOBAL_CHECK: AtomicBool = AtomicBool::new(false);

pub fn unknown_dependency() -> ! {
    panic!("Unknown dependency")
}
