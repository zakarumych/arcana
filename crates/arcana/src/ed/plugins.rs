use std::{fmt, path::Path};

use edict::World;
use egui::{Color32, Ui, WidgetText};

use arcana_project::{BuildProcess, Dependency, Ident, Project, ProjectManifest};

use crate::{plugin::ArcanaPlugin, try_log_err};

use super::{game::Games, Tab};

struct PluginsLibrary {
    /// Linked library
    #[allow(unused)]
    lib: libloading::Library,
    plugins: Vec<&'static dyn ArcanaPlugin>,
}

impl fmt::Display for PluginsLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "Plugins:\n")?;
            for plugin in &self.plugins {
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

        type ArcanaPluginsFn = fn() -> Vec<&'static dyn ArcanaPlugin>;

        // Safety: None
        let res = unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugins\0") };
        let arcana_plugins = res.map_err(|err| {
            miette::miette!(
                "Failed to load plugins library '{path}'. {err}",
                path = path.display()
            )
        })?;
        let plugins = arcana_plugins();

        for plugin in &plugins {
            plugin.__running_arcana_instance_check(&crate::plugin::GLOBAL_CHECK);
        }

        for plugin in &plugins {
            tracing::debug!("Loaded plugin '{name}'", name = plugin.name());
        }

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
            return project.plugins.iter().all(|p| {
                let is_linked = linked.plugins.iter().any(|plugin| plugin.name() == p.name);
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
        try_log_err!(project.add_plugin_with_path(path, true));
        let build = try_log_err!(project.build_plugins_library());
        self.build = Some(build);
    }

    pub fn remove_plugin(&mut self, name: &str, project: &mut Project) {
        try_log_err!(project.remove_plugin(name, true));
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
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let mut project = world.expect_resource_mut::<Project>();

        if plugins.build.is_some() {
            ui.horizontal(|ui| {
                ui.label("Building...");
                ui.spinner();
            });
        } else if ui.button("Rebuild").clicked() {
            let build = try_log_err!(project.build_plugins_library());
            plugins.build = Some(build);
        }

        ui.horizontal(|ui| {
            ui.menu_button("Add plugin lib", |ui| {
                if ui.button("Path").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("Cargo.toml")
                        .pick_file()
                    {
                        plugins.add_plugin_with_path(&path, &mut project);
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button("New plugin lib", |ui| {
                if let Some(path) = rfd::FileDialog::new().save_file() {
                    plugins.add_plugin_with_path(&path, &mut project);
                    ui.close_menu();
                }
            });
        });

        let mut sync_project = false;
        let mut remove = Vec::new();
        for plugin_idx in 0..project.manifest().plugins.len() {
            let plugin = &mut project.manifest_mut().plugins[plugin_idx];

            let mut heading = WidgetText::from(&plugin.name);

            let tooltip;
            let lib_linked = match &plugins.linked {
                None => None,
                Some(lib) => {
                    let linked = lib
                        .plugins
                        .iter()
                        .copied()
                        .find(|p| p.name() == plugin.name);

                    linked.map(|linked| (lib, linked))
                }
            };

            match lib_linked {
                None => {
                    if plugins.pending.is_some() || plugins.build.is_some() {
                        tooltip = Some("Pending".to_owned());
                        heading = heading.color(Color32::KHAKI);
                    } else {
                        tooltip = Some("Build failed".to_owned());
                        heading = heading.color(Color32::DARK_RED);
                    }
                }
                Some((lib, linked)) => {
                    match check_dependencies(linked, plugin_idx, project.manifest(), &lib.plugins) {
                        Ok(()) => {
                            tooltip = None;
                            heading = heading.color(Color32::GREEN);
                        }
                        Err(error) => {
                            tooltip = Some(error);
                            heading = heading.color(Color32::DARK_RED);
                        }
                    }
                }
            }

            let plugin = &mut project.manifest_mut().plugins[plugin_idx];
            let mut enabled = plugin.enabled;
            let r = ui.checkbox(&mut enabled, heading);
            let r = r.context_menu(|ui| {
                if ui.button("Remove").clicked() {
                    remove.push(plugin.name.clone());
                    sync_project = true;
                    ui.close_menu();
                }
            });
            if let Some(tooltip) = tooltip {
                r.on_hover_text(tooltip);
            }

            if plugin.enabled != enabled {
                plugin.enabled = enabled;
                sync_project = true;
            }
        }

        for name in remove {
            plugins.remove_plugin(&name, &mut project);
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

    /// Finds all linked plugins that were enabled.
    /// If plugin is missing, plugins lib is not linked or dependency is not placed before and enabled
    /// returns None.
    pub fn enabled_plugins(&self, project: &Project) -> Option<Vec<&dyn ArcanaPlugin>> {
        let linked = self.linked.as_ref()?;

        let manifest = project.manifest();

        manifest
            .plugins
            .iter()
            .enumerate()
            .filter(|(_, p)| p.enabled)
            .map(|(idx, plugin)| -> Option<&dyn ArcanaPlugin> {
                let plugin = linked
                    .plugins
                    .iter()
                    .copied()
                    .find(|p| p.name() == plugin.name)?;

                if check_dependencies(plugin, idx, manifest, &linked.plugins).is_err() {
                    return None;
                }

                Some(plugin)
            })
            .collect()
    }
}

fn check_dependencies(
    plugin: &dyn ArcanaPlugin,
    plugin_idx: usize,
    project: &ProjectManifest,
    plugins: &[&dyn ArcanaPlugin],
) -> Result<(), String> {
    for dep in plugin.dependencies() {
        match project.plugins[..plugin_idx]
            .iter()
            .find(|p| p.name == dep.name())
        {
            None => {
                let dep_after = project.plugins[plugin_idx + 1..]
                    .iter()
                    .any(|p| p.name == dep.name());

                if dep_after {
                    return Err(format!("Dependency '{}' is after the plugin", dep.name()));
                } else {
                    return Err(format!("Dependency '{}' is missing", dep.name()));
                }
            }
            Some(dep_plugin) => {
                if !dep_plugin.enabled {
                    return Err(format!("Dependency '{}' is disabled", dep.name()));
                }
                match plugins.iter().find(|p| p.name() == dep.name()) {
                    None => return Err(format!("Dependency '{}' not linked", dep.name())),
                    Some(dep_plugin) => {
                        if !std::ptr::eq(dep_plugin.__cmp_id(), dep.__cmp_id()) {
                            return Err(format!(
                                "Dependency '{}' is not the same instance.",
                                dep.name()
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn new_plugin_cargo_toml(name: &Ident, arcana: &Dependency) -> String {
    format!(
        r#"[package]
    name = "{name}"
    version = "0.1.0"
    edition = "2021"
    publish = false

    [dependencies]
    arcana = {arcana}
    "#
    )
}

fn new_plugin_lib_rs(name: &Ident) -> String {
    format!(
        r#"
        arcana::export_arcana_plugin!(Plugin);

        pub struct Plugin;

        impl arcana::plugin::ArcanaPlugin for Plugin {{
            fn name(&self) -> &'static str {{
                "{name}"
            }}

            fn init(&self, world: &mut arcana::edict::World, scheduler: &mut arcana::edict::Scheduler) {{
                // Add your code here.
            }}
        }}
    "#
    )
}
