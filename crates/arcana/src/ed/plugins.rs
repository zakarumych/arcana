use std::path::Path;

use arcana_project::{PluginBuild, Project};
use edict::World;
use egui::{Ui, WidgetText};
use hashbrown::{hash_map::RawEntryMut, HashMap, HashSet};

use crate::plugin::ArcanaPlugin;

use super::{game::Games, ResultExt, Tab};

struct PluginLibrary {
    /// Linked library
    lib: libloading::Library,
    plugins: &'static [&'static dyn ArcanaPlugin],
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

        type ArcanaPluginsFn = fn() -> &'static [&'static dyn ArcanaPlugin];

        // Safety: None
        let res = unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugins\0") };
        let arcana_plugins = res.map_err(|err| {
            miette::miette!(
                "Failed to load plugin library '{path}'. {err}",
                path = path.display()
            )
        })?;
        let plugins = arcana_plugins();

        Ok(PluginLibrary { lib, plugins })
    }
}

/// Tool to manage plugins libraries
/// and enable/disable plugins.
pub(super) struct Plugins {
    // List of linked plugin libraries.
    libs: HashMap<String, PluginLibrary>,

    // List of ready plugin libraries, not yet linked
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
            libs: HashMap::new(),
            pending: HashMap::new(),
            failures: HashMap::new(),
            builds: HashMap::new(),
        }
    }

    /// Adds new plugin library.
    pub fn add_library_path(&mut self, path: &Path, project: &mut Project) {
        let name = try_log_err!(project.add_library_path(path, true));
        let build = try_log_err!(project.build_plugin_library(&name));
        self.builds.insert(name.clone(), build);
    }

    pub fn tick(world: &mut World) {
        let world = world.local();
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let project = world.expect_resource_mut::<Project>();

        for name in project.manifest().plugin_libs.keys() {
            if plugins.libs.contains_key(name) {
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
            plugins.builds.insert(name.clone(), build);
        }

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
                plugins.failures.insert(name.clone(), err);
                false
            }
        });

        if plugins.pending.is_empty() {
            return;
        }

        let games = world.expect_resource_mut::<Games>();

        assert!(
            games.is_empty(),
            "There are no game instances to link plugins to"
        );

        for (name, plugin) in plugins.pending.drain() {
            plugins.libs.insert(name, plugin);
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
                        plugins.add_library_path(&path, &mut project);
                    }
                }
            });
        });

        let mut sync_project = false;
        for (name, lib) in &mut plugins.libs {
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

                    let manifest = project.manifest_mut();
                    for plugin in &*lib.plugins {
                        let position = manifest
                            .enabled
                            .iter()
                            .position(|(l, p)| l == name && p == plugin.name());

                        let mut enabled = position.is_some();
                        ui.checkbox(&mut enabled, plugin.name());

                        match (position, enabled) {
                            (Some(pos), false) => {
                                manifest.enabled.remove(pos);
                                sync_project = true;
                            }
                            (None, true) => {
                                manifest
                                    .enabled
                                    .push((name.clone(), plugin.name().to_owned()));
                                sync_project = true;
                            }
                            _ => {}
                        }
                    }
                });
        }

        for name in plugins.builds.keys() {
            if plugins.libs.contains_key(name) {
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
}