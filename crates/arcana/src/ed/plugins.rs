use arcana_project::ProjectManifest;
use egui::Ui;

use crate::plugin::ArcanaPlugin;

struct PluginLibrary {
    name: String,

    /// Linked library
    lib: libloading::Library,
    plugins: &'static [&'static dyn ArcanaPlugin],
}

/// Tool to manage plugins libraries
/// and enable/disable plugins.
pub struct Plugins {
    // List of plugin libraries.
    libs: Vec<PluginLibrary>,
}

impl Plugins {
    pub fn new() -> Self {
        Plugins { libs: Vec::new() }
    }

    pub fn add(&mut self) {}

    pub fn show(&mut self, project: &mut ProjectManifest, ui: &mut Ui) {
        ui.heading("Plugins");
        for lib in &mut self.libs {
            ui.separator();
            ui.heading(&lib.name);
            for plugin in &*lib.plugins {
                let was_enabled = project.enabled.get(&lib.name).map_or(false, |v| {
                    v.iter().find(|p| ***p == *plugin.name()).is_some()
                });
                let mut enabled = was_enabled;

                ui.checkbox(&mut enabled, plugin.name());
                if enabled != was_enabled {
                    if enabled {
                        project
                            .enabled
                            .entry(lib.name.clone())
                            .or_default()
                            .push(plugin.name().to_owned());
                    } else {
                        if let Some(plugins) = project.enabled.get_mut(&lib.name) {
                            plugins.retain(|p| *p != plugin.name());
                        }
                    }
                }
            }
        }
    }
}
