use std::{fmt, path::Path};

use edict::World;
use egui::{Color32, Ui, WidgetText};

use arcana_project::{
    plugin_with_path, BuildProcess, Dependency, Ident, IdentBuf, Plugin, Project, ProjectManifest,
};

use crate::{ok_log_err, plugin::ArcanaPlugin, try_log_err};

use super::{game::Games, Tab};

mod private {
    use std::{fmt, path::Path};

    use arcana_project::Ident;

    use crate::plugin::ArcanaPlugin;

    pub(super) struct PluginsLibrary {
        /// Linked library
        #[allow(unused)]
        lib: libloading::Library,
        plugins: &'static [(&'static Ident, &'static dyn ArcanaPlugin)],
    }

    impl PluginsLibrary {
        pub fn get(&self, name: &Ident) -> Option<&dyn ArcanaPlugin> {
            self.plugins
                .iter()
                .find_map(|(n, p)| if **n == *name { Some(*p) } else { None })
        }

        pub fn has(&self, name: &Ident) -> bool {
            self.plugins.iter().any(|(n, _)| **n == *name)
        }

        pub fn list<'a>(&'a self) -> impl Iterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)> {
            self.plugins.iter().copied()
        }
    }

    impl fmt::Display for PluginsLibrary {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if f.alternate() {
                write!(f, "Plugins:\n")?;
                for (name, _) in self.plugins {
                    write!(f, "  {}\n", *name)?;
                }
            } else {
                write!(f, "Plugins: [")?;
                let mut plugins = self.plugins.iter().map(|(name, _)| *name);
                if let Some(name) = plugins.next() {
                    write!(f, "{}", name)?;
                    for name in plugins {
                        write!(f, ", {}", name)?;
                    }
                }
                write!(f, "]")?;
            }
            Ok(())
        }
    }

    impl PluginsLibrary {
        pub fn load(path: &Path) -> miette::Result<Self> {
            // #[cfg(windows)]
            let path = {
                let filename = match path.file_name() {
                    None => miette::bail!("Invalid plugins library path '{}'", path.display()),
                    Some(name) => name,
                };

                loop {
                    let r = rand::random::<u32>();
                    let mut new_filename = filename.to_owned();
                    new_filename.push(format!(".{r:0X}"));
                    let new_path = path.with_file_name(new_filename);
                    if !new_path.exists() {
                        std::fs::copy(path, &new_path).map_err(|err| {
                            miette::miette!(
                                "Failed to copy plugins library '{path}' to '{new_path}'. {err}",
                                path = path.display(),
                                new_path = new_path.display()
                            )
                        })?;
                        tracing::debug!(
                            "Copied plugins library '{path}' to '{new_path}'",
                            path = path.display(),
                            new_path = new_path.display()
                        );
                        break new_path;
                    }
                }
            };

            // Safety: None
            let res = unsafe { libloading::Library::new(&*path) };
            let lib = res.map_err(|err| {
                miette::miette!(
                    "Failed to load plugins library '{path}'. {err}",
                    path = path.display()
                )
            })?;

            tracing::debug!("Loaded plugins library '{path}'", path = path.display());

            type ArcanaPluginsFn = fn() -> &'static [(&'static Ident, &'static dyn ArcanaPlugin)];

            // Safety: None
            let res = unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugins\0") };
            let arcana_plugins = res.map_err(|err| {
                miette::miette!(
                    "Failed to load plugins library '{path}'. {err}",
                    path = path.display()
                )
            })?;

            let plugins = arcana_plugins();

            match plugins.len() {
                1 => {
                    tracing::debug!("Loaded plugins library has one plugin");
                }
                len => {
                    tracing::debug!("Loaded plugins library has {len} plugins");
                }
            }

            for (name, plugin) in plugins {
                tracing::debug!("Verifying plugin '{}'", name);
                if !plugin.__running_arcana_instance_check(&crate::plugin::GLOBAL_CHECK) {
                    miette::bail!(
                        "Plugin '{name}' is linked to wrong Arcana instance",
                        name = name
                    );
                }
            }

            Ok(PluginsLibrary { lib, plugins })
        }
    }
}
use private::PluginsLibrary;

/// Tool to manage plugins libraries
/// and enable/disable plugins.
pub(super) struct Plugins {
    // Linked plugins library.
    linked: Option<PluginsLibrary>,

    // Pending plugins library.
    // Will become linked when all instances are reloaded.
    pending: Option<PluginsLibrary>,

    /// Plugins build failure.
    failure: Option<miette::Report>,

    // Running build process.
    build: Option<BuildProcess>,
}

impl Plugins {
    pub fn new() -> Self {
        Plugins {
            linked: None,
            pending: None,
            failure: None,
            build: None,
        }
    }

    fn all_plugins_linked(&self, project: &ProjectManifest) -> bool {
        if let Some(linked) = &self.linked {
            return project.plugins.iter().all(|p| {
                let is_linked = linked.has(&p.name);
                if !is_linked {
                    tracing::debug!("Plugin '{}' is not linked", p.name);
                }
                is_linked
            });
        }
        false
    }

    fn all_plugins_pending(&self, project: &ProjectManifest) -> bool {
        if let Some(linked) = &self.pending {
            return project.plugins.iter().all(|p| linked.has(&p.name));
        }
        false
    }

    /// Adds new plugin.
    pub fn add_plugin(&mut self, name: IdentBuf, dep: Dependency, project: &mut Project) -> bool {
        if project.add_plugin(name, dep) {
            // Stop current build if there was one.
            tracing::info!(
                "Stopping current build process to re-build plugins library with new plugin"
            );
            self.build = None;
            true
        } else {
            false
        }
    }

    pub fn tick(world: &mut World) {
        let world = world.local();
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let games = world.expect_resource_mut::<Games>();
        let project = world.expect_resource::<Project>();

        if let Some(mut build) = plugins.build.take() {
            match build.finished() {
                Ok(false) => plugins.build = Some(build),
                Ok(true) => {
                    tracing::info!(
                        "Finished building plugins library {}",
                        build.artifact().display()
                    );
                    let path = build.artifact();
                    match PluginsLibrary::load(path) {
                        Ok(lib) => {
                            tracing::info!("New plugins lib version pending. {lib:#}");
                            plugins.pending = Some(lib);
                            plugins.failure = None;
                        }
                        Err(err) => {
                            tracing::error!("Failed to load plugins library. {err}");
                            plugins.failure = Some(err);
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("Failed building plugins library. {err}");
                    plugins.failure = Some(err);
                }
            }
        }

        if games.is_empty() {
            if let Some(lib) = plugins.pending.take() {
                tracing::info!("New plugins lib version linked");
                plugins.linked = Some(lib);
            }

            if plugins.failure.is_none()
                && plugins.build.is_none()
                && !plugins.all_plugins_linked(project.manifest())
            {
                tracing::info!("Plugins lib is not linked. Building...");
                let build = try_log_err!(project.build_plugins_library());
                plugins.build = Some(build);
            }
        }
    }

    pub fn show(world: &mut World, ui: &mut Ui) {
        let world = world.local();
        let me = &mut *world.expect_resource_mut::<Plugins>();
        let mut project = world.expect_resource_mut::<Project>();
        let mut sync = false;
        let mut rebuild = false;

        if me.build.is_some() {
            ui.horizontal(|ui| {
                ui.label("Building...");
                ui.spinner();
            });
        } else if ui.button("Rebuild").clicked() {
            let build = try_log_err!(project.build_plugins_library());
            me.build = Some(build);
        }

        ui.horizontal(|ui| {
            ui.menu_button("Add plugin lib", |ui| {
                if ui.button("Path").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("Cargo.toml")
                        .pick_file()
                    {
                        if add_plugin_with_path(&path, &mut project) {
                            sync = true;
                            rebuild = true;
                        }
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button("New plugin lib", |ui| {
                if let Some(path) = rfd::FileDialog::new().save_file() {
                    if add_plugin_with_path(&path, &mut project) {
                        sync = true;
                        rebuild = true;
                        ui.close_menu();
                    }
                }
            });
        });

        let mut plugins_copy = project.manifest().plugins.clone();
        let mut remove_plugins = Vec::new();

        egui_dnd::dnd(ui, "plugins-list").show_vec(
            &mut plugins_copy,
            |ui, plugin, handle, state| {
                let mut heading = WidgetText::from(plugin.name.as_str());

                let tooltip;
                let lib_linked = match &me.linked {
                    None => None,
                    Some(lib) => lib.get(&plugin.name).map(|linked| (lib, linked)),
                };

                match lib_linked {
                    None => {
                        if me.pending.is_some() || me.build.is_some() {
                            tooltip = Some("Pending".to_owned());
                            heading = heading.color(Color32::KHAKI);
                        } else {
                            tooltip = Some("Build failed".to_owned());
                            heading = heading.color(Color32::DARK_RED);
                        }
                    }
                    Some((lib, linked)) => {
                        tooltip = None;
                        heading = heading.color(Color32::GREEN);
                    }
                }

                ui.horizontal(|ui| {
                    handle.ui(ui, |ui| {
                        ui.label(egui_phosphor::regular::DOTS_SIX_VERTICAL);
                    });

                    let mut enabled = plugin.enabled;
                    let r = ui.checkbox(&mut enabled, heading);
                    if plugin.enabled != enabled {
                        plugin.enabled = enabled;
                        sync = true;
                    }

                    let r = r.context_menu(|ui| {
                        if ui.button("Remove").clicked() {
                            remove_plugins.push(plugin.name.clone());
                            ui.close_menu();
                        }
                    });
                    if let Some(tooltip) = tooltip {
                        r.on_hover_text(tooltip);
                    }
                });
            },
        );

        project.manifest_mut().plugins = plugins_copy;

        for name in remove_plugins {
            if project.remove_plugin(&name) {
                sync = true;
                rebuild = true;
            }

            if let Some(linked) = &me.linked {
                if let Some(removed_plugin) = linked.get(&name) {
                    // Disable dependencies of the removed plugin.
                    removed_plugin.dependencies().iter().for_each(|(dep, _)| {
                        if let Some(dep) = project.manifest_mut().get_plugin_mut(*dep) {
                            dep.enabled = false
                        }
                    });
                }
            }
        }

        if sync {
            try_log_err!(project.sync());
        }

        if rebuild {
            me.build = None;
            try_log_err!(project.init_workspace());
            me.build = ok_log_err!(project.build_plugins_library());
        }
    }

    pub fn tab() -> Tab {
        Tab::Plugins
    }

    /// List all linked plugins that were enabled.
    pub fn enabled_plugins<'a, 'b>(
        &'a self,
        project: &'b Project,
    ) -> Option<Vec<(&'b Ident, &'a dyn ArcanaPlugin)>> {
        let linked = self.linked.as_ref()?;
        let mut plugins = Vec::new();

        for plugin in project.manifest().plugins.iter() {
            if plugin.enabled {
                let p = linked.get(&plugin.name)?;
                plugins.push((&*plugin.name, p));
            }
        }

        Some(plugins)
    }
}

/// Adds new plugins library.
fn add_plugin_with_path(path: &Path, project: &mut Project) -> bool {
    let (name, dep) = try_log_err!(plugin_with_path(path); false);
    project.add_plugin(name, dep)
}

// fn has_missing_dependencies(plugin: &dyn ArcanaPlugin, project: &Project) -> bool {
//     for (dep, _) in plugin.dependencies() {
//         if !project
//             .manifest()
//             .plugins
//             .iter()
//             .any(|p| p.name == dep.name())
//         {
//             return true;
//         }
//     }

//     false
// }

// fn missing_dependencies<'a>(
//     plugin: &'a dyn ArcanaPlugin,
//     project: &'a mut Project,
// ) -> impl Iterator<Item = (String, Dependency)> + 'a {
//     plugin
//         .dependencies()
//         .into_iter()
//         .filter_map(|(dep, lookup)| {
//             let exists = project
//                 .manifest()
//                 .plugins
//                 .iter()
//                 .any(|p| p.name == dep.name());
//             if !exists {
//                 Some((dep.name().to_owned(), lookup.clone()))
//             } else {
//                 None
//             }
//         })
// }

// fn check_dependencies(
//     plugin: &dyn ArcanaPlugin,
//     plugin_idx: usize,
//     project: &ProjectManifest,
//     plugins: &[(&Ident, &dyn ArcanaPlugin)],
// ) -> Result<(), String> {
//     for (p, name, dep) in plugin.dependencies() {
//         assert_eq!(project.plugins[plugin_idx].name, *name);
//         assert_eq!(project.plugins[plugin_idx].dep, dep);
//         assert_eq!(*plugins[plugin_idx].0, *name);
//         assert_eq!(plugins[plugin_idx].1 as *const _, plugin as *const _);

//         match project.plugins[..plugin_idx]
//             .iter()
//             .find(|p| *p.name == *name)
//         {
//             None => {
//                 let dep_after = project.plugins[plugin_idx + 1..]
//                     .iter()
//                     .any(|p| *p.name == *name);

//                 if dep_after {
//                     return Err(format!("Dependency '{}' is after the plugin", name));
//                 } else {
//                     return Err(format!("Dependency '{}' is missing", name));
//                 }
//             }
//             Some(plugin) => {
//                 if !plugin.enabled {
//                     return Err(format!("Dependency '{}' is disabled", name));
//                 }
//                 match plugins.iter().find(|(name, _)| name == name) {
//                     None => return Err(format!("Dependency '{}' not linked", name)),
//                     Some(plugin) => {
//                         if !plugin.__eq(dep) {
//                             return Err(format!(
//                                 "Dependency '{}' is not the same plugin.",
//                                 dep.name()
//                             ));
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     Ok(())
// }
