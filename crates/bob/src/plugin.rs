use edict::{Scheduler, World};
use hashbrown::HashMap;

/// Plugin protocol for Bob engine.
/// It allows bundling systems and resources together into a single unit
/// that can be initialized at once.
///
/// User may wish to use plugin protocol and wrap their systems and resources
/// into plugins.
/// Ed uses this protocol to load plugins from libraries.
///
/// A crate that defines plugins must export a function `bob_plugins` that
/// returns a slice of static references to plugin objects.
///
/// The easiest way to do this is to use [`export_bob_plugins!`](`export_bob_plugins`) macro.
pub trait BobPlugin {
    /// Name of the plugin.
    fn name(&self) -> &'static str;
    fn init(&self, world: &mut World, scheduler: &mut Scheduler);
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
/// # use bob::{export_bob_plugins, plugin::BobPlugin, edict::{World, Scheduler, Res}};
/// // Define a plugin.
/// struct MyBobPlugin;
///
/// impl BobPlugin for MyBobPlugin {
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
/// export_bob_plugins!(MyBobPlugin);
/// ```
///
/// ```
/// # use bob::{export_bob_plugins, plugin::BobPlugin, edict::{World, Scheduler, Res}};
/// // Export implicitly define plugin.
/// export_bob_plugins!(MyBobPlugin {
///   resources: ["world".to_string()],
///   systems: [|r: Res<String>| println!("Hello, {}!", &*r)],
/// });
/// ```
#[macro_export]
macro_rules! export_bob_plugins {
    ($($plugin:ident $({
        $(resources: [$($rinit:expr)* $(,)?] $(,)?)?
        $(systems: [$($sinit:expr)* $(,)?] $(,)?)?
    })?),* $(,)?) => {
        $($(
            #[allow(non_camel_case)]
            struct $plugin;

            impl $crate::plugin::BobPlugin for $plugin {
                fn name(&self) -> &'static str {
                    stringify!($plugin)
                }

                fn init(&self, world: &mut $crate::edict::World, scheduler: &mut $crate::edict::Scheduler) {
                    $($(world.insert_resource($rinit);)*)?
                    $($(scheduler.add_system($sinit);)*)?
                }
            }
        )?)*

        pub fn bob_plugins() -> &'static [&'static dyn $crate::plugin::BobPlugin] {
            &[$(&$plugin,)*]
        }
    };
}

struct PluginLib {
    plugins: &'static [&'static dyn BobPlugin],
}

impl PluginLib {
    pub fn init(&self, name: &str, world: &mut World, scheduler: &mut Scheduler) {
        for plugin in self.plugins {
            if plugin.name() == name {
                plugin.init(world, scheduler);
                return;
            }
        }
    }
}

/// Collection of plugin libraries.
pub struct PluginHub {
    libs: HashMap<&'static str, PluginLib>,
}

impl PluginHub {
    pub fn new() -> Self {
        PluginHub {
            libs: HashMap::new(),
        }
    }

    pub fn add_plugins(&mut self, lib: &'static str, plugins: &'static [&'static dyn BobPlugin]) {
        self.libs.insert(lib, PluginLib { plugins });
    }

    pub fn init(&self, lib: &str, name: &str, world: &mut World, scheduler: &mut Scheduler) {
        if let Some(lib) = self.libs.get(lib) {
            lib.init(name, world, scheduler);
        }
    }

    pub fn list(&self) -> Vec<(String, Vec<String>)> {
        let mut list = Vec::new();
        for (&lib, lib_plugins) in &self.libs {
            let mut names = Vec::new();
            for plugin in lib_plugins.plugins {
                names.push(plugin.name().to_owned());
            }
            list.push((lib.to_owned(), names));
        }
        list
    }
}
