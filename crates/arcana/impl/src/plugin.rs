use std::{any::Any, sync::atomic::AtomicBool};

use arcana_names::{Ident, Name};
use arcana_project::Dependency;
use edict::{IntoSystem, System, World};
use hashbrown::HashMap;

use crate::code::{CodeDesc, CodeId, FlowCode, PureCode};
use crate::input::{FilterId, InputFilter, IntoInputFilter};
use crate::make_id;
use crate::work::{Job, JobDesc, JobId};

make_id!(pub SystemId);

/// System information declared by a plugin.
#[derive(Clone)]
pub struct SystemInfo {
    /// Unique identified of the system.
    pub id: SystemId,

    /// Name of the system.
    pub name: Name,
}

/// Filter information declared by a plugin.
#[derive(Clone)]
pub struct FilterInfo {
    /// Unique identified of the filter.
    pub id: FilterId,

    /// Name of the filter.
    pub name: Name,
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
}

/// Job information declared by a plugin.
#[derive(Clone)]
pub struct CodeInfo {
    /// Unique identified of the code..
    pub id: CodeId,

    /// Name of the code.
    pub name: Name,

    /// Description of the code.
    pub desc: CodeDesc,
}

/// Active plugin hub contains
/// systems, filters and jobs
/// populated from plugins.
pub struct PluginsHub {
    pub systems: HashMap<SystemId, Box<dyn System + Send>>,
    pub filters: HashMap<FilterId, Box<dyn InputFilter>>,
    pub jobs: HashMap<JobId, Box<dyn Job>>,
    pub pure_fns: HashMap<CodeId, PureCode>,
    pub flow_fns: HashMap<CodeId, FlowCode>,
}

impl PluginsHub {
    pub fn new() -> Self {
        PluginsHub {
            systems: HashMap::new(),
            filters: HashMap::new(),
            jobs: HashMap::new(),
            pure_fns: HashMap::new(),
            flow_fns: HashMap::new(),
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
    pub fn add_pure_fn(&mut self, id: CodeId, code: PureCode) {
        self.pure_fns.insert(id, code);
    }

    /// Adds a flow fn from a plugin to the hub.
    pub fn add_flow_fn(&mut self, id: CodeId, code: FlowCode) {
        self.flow_fns.insert(id, code);
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
    fn dependencies(&self) -> Vec<(Ident, Dependency)> {
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

    /// Returns list of render jobs.
    fn jobs(&self) -> Vec<JobInfo> {
        Vec::new()
    }

    /// Returns list of pure codes.
    fn codes(&self) -> Vec<CodeInfo> {
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
    fn deinit(&self, world: &mut World) {
        let _ = world;
    }

    /// Returns true if this plugin can be replaced with the `updated` plugin.
    /// The updated plugin should typically be a newer version of the same plugin.
    ///
    /// Plugins may conservatively return `false` here.
    /// And then they may not implement `dump` and `load` methods.
    fn compatible(&self, updated: &dyn ArcanaPlugin) -> bool {
        let _ = updated;
        false
    }

    /// Dump state of the world known to this plugin.
    /// This method is called when the plugin is reloaded with updated code
    /// before `deinit` method.
    /// New version, if compatible, will load the state from the dump.
    fn dump(&self, world: &World, scratch: &mut [u8]) -> usize {
        let _ = (world, scratch);
        0
    }

    /// Load state of the world known to this plugin dumped by previous version.
    fn load(&self, world: &mut World, scratch: &[u8]) {
        let _ = (world, scratch);
        unimplemented!()
    }
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
macro_rules! job_desc_or_expr {
    ($_:ident: $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        <$name>::desc()
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! job_new_or_expr {
    ($_:ident => $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        <$name>::new()
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! pure_desc_or_expr {
    ($_:ident: $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        IntoPureCode::into_pure_code($name).0
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! flow_desc_or_expr {
    ($_:ident: $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        $name.into_flow_code().0
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! pure_code_or_expr {
    ($_:ident: $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        $name.into_pure_code().1
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! flow_code_or_expr {
    ($_:ident: $e:expr) => {{
        $e
    }};
    ($name:ident) => {{
        $name.into_flow_code().1
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
    ($name:ident { git = $git:literal $(, branch = $branch:literal )? }) => {{
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
/// struct MyPlugin;
///
/// impl ArcanaPlugin for MyPlugin {
///   fn init(&self, world: &mut World) -> PluginInit {
///     world.insert_resource("world".to_string());
///     PluginInit::new().with_system(ident!(hello), |r: Res<String>| println!("Hello, {}!", &*r));
///   }
/// }
///
/// // Export it.
/// export_arcana_plugin!(MyPlugin);
/// ```
///
/// Alternatively following syntax can be used to define plugin type and implement `ArcanaPlugin` for it in one place.
///
///
/// ```
/// # use arcana::{export_arcana_plugin, plugin::ArcanaPlugin, edict::{World, Scheduler, Res}};
/// // Export implicitly define plugin.
/// export_arcana_plugin!(MyPlugin {
///   // First list dependencies.
///   // Skip if there are no dependencies.
///   // Comma separated list of dependency identifiers and kind tokens.
///   // See below for syntax.
///   dependencies: [
///     published_dependency_name crate, // Takes dependency from crates.io
///     local_dependency_name ...,       // Takes dependency from local path, path is fetched from dependency with a function generated by this macro.
///     git_dependency_name { git = "git-url" }, // Takes dependency from git repository with a given url.
///     git_branch_dependency_name { git = "git-url", branch = "branch" }], // Takes dependency from git repository with a given url and use specified branch.
///
///   // Next declare resources that needs to be initialized.
///   // Skip if there are no resources to insert.
///   // Comma separated list of expressions.
///   // Each expression will be evaluated and result inserted into the `World` as a resource.
///   // If same type is encountered multiple times, it will replace previous value.
///   resources: ["world".to_string()],
///
///   // Next declare components that needs to be registered.
///   // Skip if there are no components to register.
///   // Commas separated list of component types.
///   // Types must implement `Component` trait.
///   components: [Foo, Bar],
///
///   // Next declare systems that will be available for scheduler.
///   // Skip if there are no systems to add.
///   // Comma separated list of system names with optional system expressions after ':' token.
///   // If expression is provided, expression will be avaluated and its result transformed with `IntoSystem`.
///   // Otherwise system name ident will be used as system expression.
///   // If system expression is a closure, make sure to specify all argument types explicitly.
///   systems: [
///     hello: |r: Res<String>| println!("Hello, {}!", &*r),
///     bye, // assume there's `fn bye(<valid system args>) {}`
///   ],
///
///   // Next declare filters that will be available for event filtering.
///   // Skip if there are no filters to add.
///   // Comma separated list of filter names with optional filter expressions after ':' token.
///   // If expression is provided, expression will be avaluated and its result transformed with `IntoFilter`.
///   filters: [],
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
        $(jobs: [$($job_name:ident $(: $job_desc:expr)? $(=> $job:expr)?),+ $(,)?] $(,)?)?
        $(pure_fns: [$($pure_fn:ident $(: $pure_desc:expr)? $(=> $pure_code:expr)?),+ $(,)?] $(,)?)?
        $(flow_fns: [$($flow_fn:ident $(: $flow_desc:expr)? $(=> $flow_code:expr)?),+ $(,)?] $(,)?)?
        $(in $world:ident $(: $world_type:ty)? $( => { $($init:tt)* })?)?
    })?) => {
        $(
            #[allow(non_camel_case)]
            struct $plugin;

            impl $crate::plugin::ArcanaPlugin for $plugin {
                $(
                    fn dependencies(&self) -> Vec<($crate::Ident, $crate::project::Dependency)> {
                        vec![$(
                            ($crate::ident!($dependency), $crate::get_dependency!($dependency $dep_kind)),
                        )+]
                    }
                )*

                $(
                    fn systems(&self) -> Vec<$crate::plugin::SystemInfo> {
                        vec![$(
                            $crate::plugin::SystemInfo {
                                id: $crate::local_name_hash_id!($system_name),
                                name: $crate::ident!($system_name).into(),
                            },
                        )+]
                    }
                )?

                $(
                    fn filters(&self) -> Vec<$crate::plugin::FilterInfo> {
                        vec![$(
                            $crate::plugin::FilterInfo {
                                id: $crate::local_name_hash_id!($filter_name),
                                name: $crate::ident!($filter_name).into(),
                            },
                        )+]
                    }
                )*

                $(
                    fn jobs(&self) -> Vec<$crate::plugin::JobInfo> {
                        vec![$(
                            $crate::plugin::JobInfo {
                                id: $crate::local_name_hash_id!($job_name),
                                name: $crate::ident!($job_name).into(),
                                desc: $crate::job_desc_or_expr!($job_name $(: $job_desc)?),
                            },
                        )+]
                    }
                )?

                fn codes(&self) -> Vec<$crate::plugin::CodeInfo> {
                    use $crate::code::{IntoPureCode, IntoFlowCode, IntoAsyncFlowCode};
                    let mut codes = Vec::new();

                    $($(
                        codes.push($crate::plugin::CodeInfo {
                            id: $crate::local_name_hash_id!($flow_fn),
                            name: $crate::ident!($flow_fn).into(),
                            desc: $crate::flow_desc_or_expr!($flow_fn $(: $flow_desc)?),
                        });
                    )+)?

                    $($(
                        codes.push($crate::plugin::CodeInfo {
                            id: $crate::local_name_hash_id!($pure_fn),
                            name: $crate::ident!($pure_fn).into(),
                            desc: $crate::pure_desc_or_expr!($pure_fn $(: $pure_desc)?),
                        });
                    )+)?

                    codes
                }

                fn init(&self, world: &mut $crate::edict::World, hub: &mut $crate::plugin::PluginsHub) {
                    use $crate::code::{IntoPureCode, IntoFlowCode, IntoAsyncFlowCode};

                    $($(world.ensure_component_registered::<$component>();)*)?

                    $(
                        let $world $($world_type)? = &mut *world;
                        $($($init)*)?
                    )?

                    $($(
                        hub.add_system($crate::local_name_hash_id!($system_name), $crate::name_or_expr!($system_name $(: $system)?));
                    )+)?

                    $($(
                        hub.add_filter($crate::local_name_hash_id!($filter_name), $crate::name_or_expr!($filter_name $(: $filter)?));
                    )+)?

                    $($(
                        hub.add_job($crate::local_name_hash_id!($job_name), $crate::job_new_or_expr!($job_name $(=> $job)?));
                    )+)?

                    $($(
                        hub.add_pure_fn($crate::local_name_hash_id!($pure_fn), $crate::pure_code_or_expr!($pure_fn $(=> $pure_code)?));
                    )+)?

                    $($(
                        hub.add_flow_fn($crate::local_name_hash_id!($flow_fn), $crate::flow_code_or_expr!($flow_fn $(=> $flow_code)?));
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
pub static GLOBAL_LINK_CHECK: AtomicBool = AtomicBool::new(false);

#[doc(hidden)]
pub fn running_arcana_instance_check(check: &AtomicBool) -> bool {
    (check as *const _ == &GLOBAL_LINK_CHECK as *const _)
        && GLOBAL_LINK_CHECK.load(::core::sync::atomic::Ordering::SeqCst)
}

#[doc(hidden)]
pub fn set_running_arcana_instance() {
    let old = GLOBAL_LINK_CHECK.swap(true, ::core::sync::atomic::Ordering::SeqCst);
    assert!(!old, "Arcana instance is already running");
}

pub fn unknown_dependency() -> ! {
    panic!("Unknown dependency")
}
