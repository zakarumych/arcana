use std::path::Path;

use arcana_project::{PluginBuild, Project};
use edict::World;
use egui::{Ui, WidgetText};
use hashbrown::HashMap;

use crate::plugin::ArcanaPlugin;

use super::{game::Games, Tab};

struct PluginLibrary {
    /// Linked library
    lib: libloading::Library,
    plugin: &'static dyn ArcanaPlugin,
}

impl PluginLibrary {
    pub fn load(path: &Path) -> miette::Result<Self> {
        // Safety: None
        let res = unsafe { libloading::Library::new(path) };
        let lib = res.map_err(|err| {
            miette::miette!(
                "Failed to load plugin library '{path}'. {err}",
                path = path.display()
            )
        })?;

        type ArcanaPluginsFn = fn() -> &'static dyn ArcanaPlugin;

        // Safety: None
        let res = unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugin_dyn\0") };
        let arcana_plugin_dyn = res.map_err(|err| {
            miette::miette!(
                "Failed to load plugin library '{path}'. {err}",
                path = path.display()
            )
        })?;
        let plugin = arcana_plugin_dyn();

        Ok(PluginLibrary { lib, plugin })
    }
}

/// Tool to manage plugins libraries
/// and enable/disable plugins.
pub(super) struct Plugins {
    // List of linked plugins.
    linked: HashMap<String, PluginLibrary>,

    // List of ready plugins, not yet linked
    // to the instances.
    // If not instances are running then linking is no-op.
    pending: HashMap<String, PluginLibrary>,

    /// List of plugin build failures.
    failures: HashMap<String, miette::Report>,

    // List of running builds.
    builds: HashMap<String, PluginBuild>,
}

impl Plugins {
    pub fn new() -> Self {
        Plugins {
            linked: HashMap::new(),
            pending: HashMap::new(),
            failures: HashMap::new(),
            builds: HashMap::new(),
        }
    }

    /// Adds new plugin library.
    pub fn add_plugin_with_path(&mut self, path: &Path, project: &mut Project) {
        self.builds.clear();
        let name = try_log_err!(project.add_plugin_with_path(path, true));
        tracing::info!("Plugin '{name} added");
    }

    pub fn tick(world: &mut World) {
        let world = world.local();
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let project = world.expect_resource_mut::<Project>();
        let games = world.expect_resource_mut::<Games>();

        for name in project.manifest().plugins.iter().map(|p| &*p.name) {
            if plugins.linked.contains_key(name) {
                continue;
            }
            if plugins.pending.contains_key(name) {
                continue;
            }
            if plugins.failures.contains_key(name) {
                continue;
            }
            if plugins.builds.contains_key(name) {
                continue;
            }

            let build = try_log_err!(project.build_plugin_library(name));
            plugins.builds.insert(name.to_owned(), build);
        }

        if games.is_empty() {
            plugins
                .linked
                .retain(|name, _| project.manifest().plugins.iter().any(|p| p.name == *name));
        }

        plugins
            .builds
            .retain(|name, _| project.manifest().plugins.iter().any(|p| p.name == *name));

        plugins
            .pending
            .retain(|name, _| project.manifest().plugins.iter().any(|p| p.name == *name));

        plugins
            .failures
            .retain(|name, _| project.manifest().plugins.iter().any(|p| p.name == *name));

        plugins.builds.retain(|name, build| match build.finished() {
            Ok(false) => true,
            Ok(true) => {
                tracing::info!("Finished building plugin library '{}'", name);
                let path = build.artifact();
                match PluginLibrary::load(path) {
                    Ok(lib) => {
                        plugins.pending.insert(name.clone(), lib);
                    }
                    Err(err) => {
                        tracing::error!("Failed to load plugin library '{name}'. {err}");
                        plugins.failures.insert(name.clone(), err);
                    }
                }
                false
            }
            Err(err) => {
                tracing::error!("Failed building plugin library '{}'", name);
                plugins.failures.insert(name.clone(), err);
                false
            }
        });

        if games.is_empty() {
            for (name, plugin) in plugins.pending.drain() {
                plugins.linked.insert(name, plugin);
            }
        }
    }

    pub fn show(world: &mut World, ui: &mut Ui) {
        let world = world.local();
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let mut project = world.expect_resource_mut::<Project>();

        ui.horizontal(|ui| {
            ui.menu_button("Add plugin lib", |ui| {
                if ui.button("Path").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("Cargo.toml")
                        .pick_file()
                    {
                        plugins.add_plugin_with_path(&path, &mut project);
                    }
                }
            });
        });

        let mut sync_project = false;
        for name in plugins.linked.keys() {
            if project.manifest().plugins.iter().all(|p| p.name != *name) {
                continue;
            };

            let mut heading = WidgetText::from(name);

            if plugins.pending.contains_key(name) || plugins.builds.contains_key(name) {
                heading = heading.color(egui::Color32::KHAKI);
            } else {
                heading = heading.color(egui::Color32::GREEN);
            }

            egui::CollapsingHeader::new(heading)
                .default_open(true)
                .show_unindented(ui, |ui| {
                    if plugins.builds.contains_key(name) {
                        ui.horizontal(|ui| {
                            ui.label("Building...");
                            ui.spinner();
                        });
                    } else if ui.button("Rebuild").clicked() {
                        (|builds: &mut HashMap<_, _>| {
                            let build = try_log_err!(project.build_plugin_library(name));
                            builds.insert(name.clone(), build);
                        })(&mut plugins.builds);
                    }

                    let plugin = project
                        .manifest_mut()
                        .plugins
                        .iter_mut()
                        .find(|p| p.name == *name)
                        .unwrap();

                    let mut enabled = plugin.enabled;
                    ui.checkbox(&mut enabled, name);
                    if plugin.enabled != enabled {
                        plugin.enabled = enabled;
                        sync_project = true;
                    }
                });
        }

        for name in plugins.builds.keys() {
            if plugins.linked.contains_key(name) {
                continue;
            }

            let heading = WidgetText::from(name).color(egui::Color32::YELLOW);
            egui::CollapsingHeader::new(heading)
                .default_open(true)
                .show_unindented(ui, |ui| {
                    ui.label("Building...");
                    ui.spinner();
                });
        }

        if sync_project {
            if let Err(err) = project.sync() {
                tracing::error!("Failed to sync project: {}", err);
            }
        }
    }

    pub fn tab() -> Tab {
        Tab::Plugins
    }

    pub fn enabled_plugins(&self, project: &Project) -> Option<Vec<&dyn ArcanaPlugin>> {
        let manifest = project.manifest();

        manifest
            .plugins
            .iter()
            .filter(|p| p.enabled)
            .map(|plugin| -> Option<&dyn ArcanaPlugin> {
                let lib = &self.linked.get(&plugin.name)?;
                Some(lib.plugin)
            })
            .collect()
    }
}
