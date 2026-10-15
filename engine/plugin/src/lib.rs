//! Plugin API
//!
//! Plugin system is the heart of Arcana.
//! It allows adding systems, input filters, GPU jobs, asset importers and more.
//!
//! Plugins are implemented as Rust crates that depend on `arcana` engine crate, and use proc-attributes for exports.
//! At development time, Arcana Ed will collect added plugins into a plugins library that can be loaded at runtime.
//! For release plugins will be statically linked to the game binary, removing performance overhead.

mod component;
mod filter;
mod hub;
mod import;
mod job;
mod system;

pub use self::{
    component::{Component, ComponentError, ComponentInfo},
    filter::{FilterId, FilterInfo, InputFilter, IntoInputFilter},
    hub::PluginsHub,
    import::{Importer, ImporterDesc, ImporterId, ImporterInfo},
    job::{Job, JobDesc, JobId, JobInfo},
    system::{IntoSystem, System, SystemId, SystemInfo},
};

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use arcana_id::HasStid;
use arcana_intern::Name;
use arcana_model::{Reflect, Value};
use arcana_project::Dependency;
use edict::world::World;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
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
/// A crate must use [`declare_plugin!`] macro to declare it is a plugin.
pub struct ArcanaPlugin {
    /// Path to the plugin source code if it is a local plugin.
    location: Option<PathBuf>,

    /// World initialization functions.
    /// Those will be run each time a world is initialized.
    init: Vec<fn(&mut World)>,

    /// Authored defaults, invoked explicitly when creating a scene.
    scene_init: Vec<fn(&mut World)>,

    /// List of dependencies.
    dependencies: Vec<(Name, Dependency)>,

    /// List of event filters.
    filters: Vec<FilterInfo>,

    /// List of systems.
    systems: Vec<SystemInfo>,

    /// List of jobs.
    jobs: Vec<JobInfo>,

    /// List of asset importers.
    importers: Vec<ImporterInfo>,

    /// Editable component registrations.
    components: Vec<ComponentInfo>,

    fill_hub: Vec<fn(&mut PluginsHub)>,
}

impl ArcanaPlugin {
    pub fn new(location: Option<PathBuf>) -> Self {
        ArcanaPlugin {
            location,
            init: Vec::new(),
            scene_init: Vec::new(),
            dependencies: Vec::new(),
            filters: Vec::new(),
            systems: Vec::new(),
            jobs: Vec::new(),
            importers: Vec::new(),
            components: Vec::new(),
            fill_hub: Vec::new(),
        }
    }

    pub fn add_dependency(&mut self, name: Name, dep: Dependency) {
        self.dependencies.push((name, dep));
    }

    pub fn add_init(&mut self, add: fn(&mut World)) {
        self.init.push(add);
    }

    /// Registers authored defaults. Ordinary world initialization does not seed scenes.
    pub fn add_scene_init(&mut self, init: fn(&mut World)) {
        self.scene_init.push(init);
    }

    /// Seeds a newly authored scene. Call once after runtime initialization.
    pub fn seed_scene(&self, world: &mut World) {
        for init in &self.scene_init {
            init(world);
        }
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

    pub fn add_importer(&mut self, info: ImporterInfo, add: fn(&mut PluginsHub)) {
        self.importers.push(info);
        self.fill_hub.push(add);
    }

    /// Registers editable component operations using `T`'s stable ID.
    /// Panics if that ID is already registered by this plugin.
    pub fn add_component<T>(
        &mut self,
        name: Name,
        initial_value: fn() -> Value,
    ) -> &mut ComponentInfo
    where
        T: Component + Reflect + HasStid,
    {
        let component = ComponentInfo::new::<T>(name, initial_value);
        let id = component.id();
        assert!(
            self.components.iter().all(|component| component.id() != id),
            "Component stable ID {id} is already registered"
        );
        let index = self.components.len();
        self.components.push(component);
        &mut self.components[index]
    }

    pub fn location(&self) -> Option<PathBuf> {
        self.location.clone()
    }

    pub fn dependencies(&self) -> Vec<(Name, Dependency)> {
        self.dependencies.clone()
    }

    pub fn init(&self, world: &mut World, hub: &mut PluginsHub) {
        for component in &self.components {
            hub.add_component(component.clone());
        }

        for fill in &self.fill_hub {
            fill(hub);
        }

        for init in &self.init {
            init(world);
        }
    }

    pub fn filters(&self) -> &[FilterInfo] {
        &self.filters
    }

    pub fn systems(&self) -> &[SystemInfo] {
        &self.systems
    }

    pub fn jobs(&self) -> &[JobInfo] {
        &self.jobs
    }

    pub fn importers(&self) -> &[ImporterInfo] {
        &self.importers
    }

    pub fn components(&self) -> &[ComponentInfo] {
        &self.components
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! plugin_dependency_kind {
    ($name:ident) => {
        $name::arcana_plugin::dependency()
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
            pub fn dependency() -> $crate::for_macro::Dependency {
                $crate::for_macro::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
            }

            pub fn path_dependency() -> $crate::for_macro::Dependency {
                $crate::for_macro::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap()
            }

            pub fn get() -> $crate::ArcanaPlugin {
                // // Safety: This value is accessed mutably at cdylib load time.
                // // Afterwards it can only be accessed immutably here.
                // unsafe { ARCANA_PLUGIN_REGISTRY.plugin() }

                let mut plugin = $crate::ArcanaPlugin::new(Some($crate::for_macro::PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
                for add in ARCANA_PLUGIN_REGISTRY {
                    add(&mut plugin);
                }

                $($(
                        plugin.add_dependency($crate::for_macro::ident!($dependency), $crate::plugin_dependency_kind!($dependency $($kind)*));
                )+)*

                plugin
            }

            // pub static mut ARCANA_PLUGIN_REGISTRY: $crate::for_macro::Registry =
            //     $crate::for_macro::Registry::new();

            #[$crate::for_macro::distributed_slice]
            #[linkme(crate = $crate::for_macro::linkme)]
            pub(crate) static ARCANA_PLUGIN_REGISTRY: [fn(&mut $crate::ArcanaPlugin)];
        }
    };
}

#[doc(hidden)]
pub mod for_macro {
    pub use std::path::PathBuf;

    pub use arcana_intern::{Ident, ident};
    pub use arcana_project::Dependency;
    pub use ctor::ctor;
    pub use linkme::{self as linkme, distributed_slice};

    #[doc(hidden)]
    #[macro_export]
    macro_rules! pkg_name {
        () => {
            $crate::for_macro::Ident::from_str(env!("CARGO_PKG_NAME")).unwrap()
        };
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! plugin_ctor_add {
        ($registry:ident @ $plugin:ident => $($code:tt)*) => {
            const _: () = {
                fn add($plugin: &mut $crate::ArcanaPlugin) {
                    // At this point cdylib is initialized and any code can be executed.
                    $($code)*
                }

                #[$crate::for_macro::distributed_slice(crate::arcana_plugin::$registry)]
                #[linkme(crate = $crate::for_macro::linkme)]
                static ADD: fn(&mut $crate::ArcanaPlugin) = add;
            };
        };
    }
}
