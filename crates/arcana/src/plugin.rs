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
        self.systems.push((name, Box::new(system.into_system())));
        self
    }

    #[cfg(feature = "client")]
    pub fn with_filter<F>(mut self, name: &'a Ident, filter: F) -> Self
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
    fn dependencies(&self) -> Vec<(&Ident, Dependency)> {
        vec![]
    }

    /// Returns list of named event filters.
    fn event_filters(&self) -> Vec<&Ident> {
        vec![]
    }

    /// Returns list of systems.
    fn systems(&self) -> Vec<&Ident> {
        vec![]
    }

    /// Registers components and resources.
    /// Perform any other initialization of the world.
    /// Returns list of systems and event filters.
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

/// Exports plugins from a crate.
/// Use this macro in the crate's root.
/// It may be used only once per crate.
/// All plugins must be listed in the macro call to be exported.
///
/// # Example
///
///
/// ```
/// # use arcana::{export_arcana_plugin, plugin::ArcanaPlugin, edict::{World, Scheduler, Res}};
/// // Define a plugin.
/// struct MyBobPlugin;
///
/// impl ArcanaPlugin for MyBobPlugin {
///   fn name(&self) -> &'static str {
///     "my_plugin"
///   }
///
///   fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
///     world.insert_resource("world".to_string());
///     scheduler.add_system(|r: Res<String>| println!("Hello, {}!", &*r));
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
///   systems: [|r: Res<String>| println!("Hello, {}!", &*r)],
/// });
/// ```
#[macro_export]
macro_rules! export_arcana_plugin {
    ($plugin:ident $({
        $(@components: [$($component:ty)* $(,)?] $(,)?)?
        $(@resources: [$($resource:expr)* $(,)?] $(,)?)?
        $(@systems: [$($sname:ident: $system:expr)* $(,)?] $(,)?)?
        $(@filters: [$($fname:ident: $filter:expr)* $(,)?] $(,)?)?
        $init:expr
    })?) => {
        $(
            #[allow(non_camel_case)]
            struct $plugin;

            impl $crate::plugin::ArcanaPlugin for $plugin {
                $(
                    fn systems(&self) -> Vec<($crate::project::IdentBuf, Box<dyn $crate::edict::System + Send>)> {
                        vec![$(
                            ($crate::project::ident!($sname).to_buf(), Box::new($system) as Box<dyn $crate::edict::System>),
                        )*]
                    }
                )?
                $($crate::feature_client!{
                    fn event_filters(&self) -> Vec<($crate::project::IdentBuf, Box<dyn $crate::funnel::EventFilter>)> {
                        vec![$(
                            ($crate::project::ident!($fname).to_buf(), Box::new($filter) as Box<dyn $crate::funnel::EventFilter>),
                        )*]
                    }
                })?
                fn init_world(&self, world: &mut $crate::edict::World) {
                    $($(world.ensure_component_registered($component);)*)?
                    $($(world.insert_resource($resource);)*)?
                }
            }
        )?

        pub const fn __arcana_plugin() -> &'static $plugin {
            &$plugin
        }

        $crate::feature_ed! {
            pub fn dependency() -> (&'static $crate::project::Ident, $crate::project::Dependency) {
                (
                    $crate::project::Ident::from_ident_str(env!("CARGO_PKG_NAME")),
                    $crate::project::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
                )
            }
        }

        $crate::feature_ed! {
            pub fn git_dependency(git: &str, branch: Option<&str>) -> (&'static $crate::project::Ident, $crate::project::Dependency) {
                (
                    $crate::project::Ident::from_ident_str(env!("CARGO_PKG_NAME")),
                    $crate::project::Dependency::Git {
                        git: git.to_owned(),
                        branch: branch.map(str::to_owned),
                    }
                )
            }
        }

        $crate::feature_ed! {
            pub fn path_dependency() -> (&'static $crate::project::Ident, $crate::project::Dependency) {
                (
                    $crate::project::Ident::from_ident_str(env!("CARGO_PKG_NAME")),
                    $crate::project::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap(),
                )
            }
        }
    };
}

#[doc(hidden)]
pub static GLOBAL_CHECK: AtomicBool = AtomicBool::new(false);
