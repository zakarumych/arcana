use std::{any::Any, sync::atomic::AtomicBool};

use arcana_project::{Dependency, Ident, IdentBuf};
use edict::{IntoSystem, Scheduler, System, World};

#[cfg(feature = "client")]
use crate::funnel::EventFilter;

pub struct PluginInit<'a> {
    pub systems: Vec<(&'a Ident, Box<dyn System + Send>)>,
    #[cfg(feature = "client")]
    pub filters: Vec<(&'a Ident, Box<dyn EventFilter>)>,
}

impl<'a> PluginInit<'a> {
    pub fn new() -> Self {
        PluginInit {
            systems: vec![],
            #[cfg(feature = "client")]
            filters: vec![],
        }
    }

    pub fn with_system<S, M>(mut self, name: &'a Ident, system: S) -> Self
    where
        S: IntoSystem<M>,
    {
        self.add_system(name, system);
        self
    }

    pub fn add_system<S, M>(&mut self, name: &'a Ident, system: S) -> &mut Self
    where
        S: IntoSystem<M>,
    {
        self.systems.push((name, Box::new(system.into_system())));
        self
    }

    #[cfg(feature = "client")]
    pub fn with_filter<F>(mut self, name: &'a Ident, filter: F) -> Self
    where
        F: EventFilter + 'static,
    {
        self.add_filter(name, filter);
        self
    }

    #[cfg(feature = "client")]
    pub fn add_filter<F>(&mut self, name: &'a Ident, filter: F) -> &mut Self
    where
        F: EventFilter + 'static,
    {
        self.filters.push((name, Box::new(filter)));
        self
    }
}

#[macro_export]
macro_rules! plugin_init {
    (
        $(systems: [$($system:ident),* $(,)?])?
        $(filters: [$($filter:ident),* $(,)?])?
    ) => {{
        let init = $crate::plugin::PluginInit::new();
        $(let init = init $(.with_system($crate::project::ident!($system), $system))*;)?

        $($crate::feature_client!{
            let init = init $(.with_filter($crate::project::ident!($filter), $filter))*;
        })?

        init
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
    /// Returns list of plugins this plugin depends on.
    /// Dependencies must be initialized first and deinitialized last.
    fn dependencies(&self) -> &[&Ident] {
        &[]
    }

    /// Returns list of plugins this plugin depends on.
    /// Dependencies must be initialized first and deinitialized last.
    fn get_dependency(&self, dep: &Ident) -> Dependency {
        unknown_dependency();
    }

    /// Returns list of named event filters.
    fn filters(&self) -> &[&Ident] {
        &[]
    }

    /// Returns list of systems.
    fn systems(&self) -> &[&Ident] {
        &[]
    }

    /// Registers components and resources.
    /// Perform any other initialization of the world.
    /// Returns constructed systems and event filters.
    fn init(&self, world: &mut World) -> PluginInit {
        let _ = world;
        PluginInit {
            systems: vec![],
            #[cfg(feature = "client")]
            filters: vec![],
        }
    }

    /// De-initializes world.
    /// Removes resources that belongs to this plugin.
    /// This method is called when game instance is closed,
    /// plugin is disabled or replaced with another version.
    fn deinit(&self, world: &mut World) {
        unimplemented!()
    }

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
    /// New version will load the state from the dump.
    fn dump(&self, world: &World, scratch: &mut [u8]) -> usize {
        unimplemented!()
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
        $(in $world:ident $(: $world_type:ty)? $( => { $($init:tt)* })?)?
    })?) => {
        $(
            #[allow(non_camel_case)]
            struct $plugin;

            impl $crate::plugin::ArcanaPlugin for $plugin {
                $(
                    fn dependencies(&self) -> &[&$crate::project::Ident] {
                        static IDENTS: &[&$crate::project::Ident] = &[$($crate::project::ident!($dependency),)+];
                        IDENTS
                    }

                    fn get_dependency(&self, dep: &Ident) -> $crate::project::Dependency {
                        $(
                            if dep == $crate::project::ident!($dependency) {
                                return $crate::get_dependency!($dependency $dep_kind);
                            }
                        )+
                        $crate::plugin::unknown_dependency()
                    }
                )*

                $(
                    fn systems(&self) -> &[&$crate::project::Ident] {
                        static IDENTS: &[&$crate::project::Ident] = &[$($crate::project::ident!($system_name),)+];
                        IDENTS
                    }
                )?

                $(
                    fn filters(&self) -> &[&$crate::project::Ident] {
                        static IDENTS: &[&$crate::project::Ident] = &[$($crate::project::ident!($filter_name),)+];
                        IDENTS
                    }
                )*

                fn init(&self, world: &mut $crate::edict::World) -> $crate::plugin::PluginInit {
                    $($(world.ensure_component_registered::<$component>();)*)?

                    $(
                        let $world $($world_type)? = &mut *world;
                        $($($init)*)?
                    )?

                    let mut init = $crate::plugin::PluginInit::new();

                    $($(
                        init.add_system($crate::project::ident!($system_name), $crate::name_or_expr!($system_name $(: $system)?));
                    )+)?

                    $($crate::feature_client! {
                        $(
                            init.add_filter($crate::project::ident!($filter_name), $crate::name_or_expr!($filter_name $(: $filter)?));
                        )+
                    })?

                    $crate::init_resources! {
                        world $(as $world)?
                        [$($($resource),+)?]
                    }

                    init
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
