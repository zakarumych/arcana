use std::{fmt, path::Path};

use edict::World;
use egui::{Color32, Ui, WidgetText};

use arcana_project::{BuildProcess, Project, ProjectManifest};

use crate::{plugin::ArcanaPlugin, try_log_err};

use super::{game::Games, Tab};

struct PluginsLibrary {
    /// Linked library
    #[allow(unused)]
    lib: libloading::Library,
    plugins: &'static [&'static dyn ArcanaPlugin],
}

impl fmt::Display for PluginsLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "Plugins:\n")?;
            for plugin in self.plugins {
                write!(f, "  {}\n", plugin.name())?;
            }
        } else {
            write!(f, "Plugins: [")?;
            let mut plugins = self.plugins.iter().map(|p| p.name());
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
        #[cfg(windows)]
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

        type ArcanaPluginsFn = fn() -> &'static [&'static dyn ArcanaPlugin];

        // Safety: None
        let res = unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugins\0") };
        let arcana_plugins = res.map_err(|err| {
            miette::miette!(
                "Failed to load plugins library '{path}'. {err}",
                path = path.display()
            )
        })?;
        let plugins = arcana_plugins();

        Ok(PluginsLibrary { lib, plugins })
    }
}

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
            return project
                .plugins
                .iter()
                .all(|p| linked.plugins.iter().any(|plugin| plugin.name() == p.name));
        }
        false
    }

    fn all_plugins_pending(&self, project: &ProjectManifest) -> bool {
        if let Some(linked) = &self.pending {
            return project
                .plugins
                .iter()
                .all(|p| linked.plugins.iter().any(|plugin| plugin.name() == p.name));
        }
        false
    }

    /// Adds new plugins library.
    pub fn add_plugin_with_path(&mut self, path: &Path, project: &mut Project) {
        // Stop current build if there was one.
        self.build = None;
        let name = try_log_err!(project.add_plugin_with_path(path, true));
        tracing::info!("Plugin '{name} added");
        let build = try_log_err!(project.build_plugins_library());
        self.build = Some(build);
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
        for plugin in &mut project.manifest_mut().plugins {
            let mut heading = WidgetText::from(&plugin.name);

            let is_linked = match &plugins.linked {
                None => false,
                Some(lib) => lib.plugins.iter().any(|p| p.name() == plugin.name),
            };

            if is_linked {
                heading = heading.color(Color32::GREEN);
            } else if plugins.pending.is_some() || plugins.build.is_some() {
                heading = heading.color(Color32::KHAKI);
            } else {
                heading = heading.color(Color32::DARK_RED);
            }

            let mut enabled = plugin.enabled;
            ui.checkbox(&mut enabled, heading);
            if plugin.enabled != enabled {
                plugin.enabled = enabled;
                sync_project = true;
            }
        }

        if plugins.build.is_some() {
            ui.horizontal(|ui| {
                ui.label("Building...");
                ui.spinner();
            });
        } else if ui.button("Rebuild").clicked() {
            let build = try_log_err!(project.build_plugins_library());
            plugins.build = Some(build);
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

    // Finds all linked plugins that were enabled.
    // If plugin is missing or plugins lib is not linked returns None.
    pub fn enabled_plugins(&self, project: &Project) -> Option<Vec<&dyn ArcanaPlugin>> {
        let linked = self.linked.as_ref()?;

        let manifest = project.manifest();

        manifest
            .plugins
            .iter()
            .filter(|p| p.enabled)
            .map(|plugin| -> Option<&dyn ArcanaPlugin> {
                linked
                    .plugins
                    .iter()
                    .copied()
                    .find(|p| p.name() == plugin.name)
            })
            .collect()
    }
}
