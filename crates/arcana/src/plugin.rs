use std::{any::Any, sync::atomic::AtomicBool};

#[cfg(feature = "ed")]
use arcana_project::Dependency;

use edict::{Scheduler, World};

#[cfg(feature = "client")]
use crate::funnel::EventFunnel;

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
    /// Name of the plugin.
    fn name(&self) -> &'static str;

    /// Returns slice with plugins this plugins depends on.
    /// Dependencies must be initialized first and
    /// deinitialized last.
    #[cfg(feature = "ed")]
    fn dependencies(&self) -> Vec<(&'static dyn ArcanaPlugin, Dependency)> {
        vec![]
    }

    /// Initializes world and scheduler.
    /// This method should install all necessary systems and resources.
    /// Avoid adding entities here, because this method can be called again.
    fn init(&self, world: &mut World, scheduler: &mut Scheduler);

    #[cfg(feature = "client")]
    fn init_funnel(&self, funnel: &mut EventFunnel) {
        let _ = funnel;
    }

    /// De-initializes world and scheduler.
    /// Removes systems and resources that belongs to this plugin.
    /// This method is called when game instance is closed,
    /// plugin is disabled or replaced with another version.
    fn deinit(&self, world: &mut World, scheduler: &mut Scheduler) {
        unimplemented!()
    }

    /// Returns true if this plugin can be replaced with the `updated` plugin.
    /// The updated plugin should typically be a newer version of the same plugin.
    ///
    /// They may be incompatible if binary dump schema changes.
    /// If new version is not compatible with the old one,
    /// the editor will not reload the plugin until all game instances
    /// that use this plugin are closed.
    ///
    /// Plugins may conservatively return `false` here.
    /// And then they may not implement `dump` and `load` methods.
    #[cfg(feature = "ed")]
    fn compatible(&self, updated: &dyn ArcanaPlugin) -> bool {
        false
    }

    /// Dump state of the world known to this plugin.
    /// This method is called when the plugin is reloaded with updated code
    /// before `deinit` method.
    /// New version will load the state from the dump.
    #[cfg(feature = "ed")]
    fn dump(&self, world: &World, scratch: &mut [u8]) -> usize {
        unimplemented!()
    }

    /// Load state of the world known to this plugin dumped by previous version.
    #[cfg(feature = "ed")]
    fn load(&self, world: &mut World, scratch: &[u8]) {
        unimplemented!()
    }

    #[cfg(feature = "ed")]
    #[doc(hidden)]
    fn __running_arcana_instance_check(&self, check: &AtomicBool) {
        assert!(
            check as *const _ == &GLOBAL_CHECK as *const _
                && GLOBAL_CHECK.load(::core::sync::atomic::Ordering::Relaxed),
            "Wrong instance of Arcana library linked"
        );
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
        $(resources: [$($rinit:expr)* $(,)?] $(,)?)?
        $(systems: [$($sinit:expr)* $(,)?] $(,)?)?
    })?) => {
        $(
            #[allow(non_camel_case)]
            struct $plugin;

            impl $crate::plugin::ArcanaPlugin for $plugin {
                fn name(&self) -> &'static str {
                    stringify!($plugin)
                }

                fn init(&self, world: &mut $crate::edict::World, scheduler: &mut $crate::edict::Scheduler) {
                    $($(world.insert_resource($rinit);)*)?
                    $($(scheduler.add_system($sinit);)*)?
                }
            }
        )?

        pub const fn __arcana_plugin() -> &'static $plugin {
            &$plugin
        }

        $crate::feature_ed! {
            pub fn dependency() -> (&'static dyn $crate::plugin::ArcanaPlugin, $crate::project::Dependency) {
                (
                    __arcana_plugin(),
                    $crate::project::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
                )
            }
        }

        $crate::feature_ed! {
            pub fn git_dependency(git: &str, branch: Option<&str>) -> (&'static dyn $crate::plugin::ArcanaPlugin, $crate::project::Dependency) {
                (
                    __arcana_plugin(),
                    $crate::project::Dependency::Git {
                        git: git.to_owned(),
                        branch: branch.map(str::to_owned),
                    }
                )
            }
        }

        $crate::feature_ed! {
            pub fn path_dependency() -> (&'static dyn $crate::plugin::ArcanaPlugin, $crate::project::Dependency) {
                (
                    __arcana_plugin(),
                    $crate::project::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap(),
                )
            }
        }
    };
}

#[doc(hidden)]
pub static GLOBAL_CHECK: AtomicBool = AtomicBool::new(false);
