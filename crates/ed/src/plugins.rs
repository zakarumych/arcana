use std::{collections::VecDeque, fmt, path::Path};

use arcana::{
    edict::world::WorldLocal,
    game::Game,
    plugin::{ArcanaPlugin, GLOBAL_CHECK},
    project::{BuildProcess, Dependency, Ident, IdentBuf, Profile, Project, ProjectManifest},
    With, World,
};
use arcana_project::{new_plugin_crate, process_path_name, Plugin};
use camino::{Utf8Path, Utf8PathBuf};
use egui::{Color32, RichText, Ui};
use egui_file::FileDialog;
use hashbrown::HashSet;

use crate::{data::ProjectData, get_profile, sync_project, systems::Systems};

use super::Tab;

pub struct PluginsLibrary {
    /// Linked library
    #[allow(unused)]
    lib: libloading::Library,
    plugins: Vec<(&'static Ident, &'static dyn ArcanaPlugin)>,
}

impl fmt::Display for PluginsLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "Plugins:\n")?;
            for (name, _) in &self.plugins {
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

struct SortError {
    pub circular_dependencies: Vec<(IdentBuf, IdentBuf)>,
    pub missing_dependencies: Vec<(IdentBuf, Dependency)>,
}

impl PluginsLibrary {
    pub fn get(&self, name: &Ident) -> Option<&dyn ArcanaPlugin> {
        let (_, p) = self.get_static(name)?;
        Some(p)
    }

    fn get_static(&self, name: &Ident) -> Option<(&'static Ident, &'static dyn ArcanaPlugin)> {
        self.plugins
            .iter()
            .find_map(|(n, p)| if **n == *name { Some((*n, *p)) } else { None })
    }

    pub fn has(&self, name: &Ident) -> bool {
        self.plugins.iter().any(|(n, _)| **n == *name)
    }

    pub fn list<'a>(&'a self) -> impl Iterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)> {
        self.plugins.iter().copied()
    }

    /// Sort plugins placing dependencies first.
    /// Errors on circular dependencies, missing dependencies and not linked plugins.
    fn sort_plugins(&mut self, plugins: &mut [Plugin]) -> Result<(), SortError> {
        let mut queue = VecDeque::new();

        let mut error = SortError {
            circular_dependencies: Vec::new(),
            missing_dependencies: Vec::new(),
        };

        for (name, _) in &self.plugins {
            queue.push_back(*name);
        }

        let mut pending = HashSet::new();
        let mut sorted = HashSet::new();
        let mut result = Vec::new();

        while let Some(name) = queue.pop_front() {
            if sorted.contains(name) {
                continue;
            }
            pending.insert(name);

            let plugin = self.get(name).unwrap();

            let mut defer = false;
            for (dep_name, dependency) in plugin.dependencies() {
                if sorted.contains(dep_name) {
                    continue;
                }

                if pending.contains(dep_name) {
                    error
                        .circular_dependencies
                        .push((name.to_buf(), dep_name.to_buf()));
                    continue;
                }

                if !self.has(dep_name) {
                    error
                        .missing_dependencies
                        .push((dep_name.to_buf(), dependency));
                    continue;
                };

                if !defer {
                    defer = true;
                    queue.push_front(name);
                }

                queue.push_front(dep_name);
            }

            if !defer {
                sorted.insert(name);
                result.push(name);
            }
        }

        if !error.circular_dependencies.is_empty() || !error.missing_dependencies.is_empty() {
            return Err(error);
        }

        let mut sorted_plugins = Vec::new();

        for (idx, name) in result.into_iter().enumerate() {
            let (name, plugin) = self.get_static(name).unwrap();
            sorted_plugins.push((name, plugin));

            let plugin = plugins
                .iter_mut()
                .position(|p| p.name == *name)
                .expect("Plugin not found");

            plugins.swap(idx, plugin);
        }

        assert_eq!(self.plugins.len(), sorted_plugins.len());

        self.plugins = sorted_plugins;
        Ok(())
    }

    /// Enumerate active plugins
    /// e.g. enabled plugins for which all dependencies are active
    pub fn active_plugins<'a>(
        &'a self,
        enabled_plugins: &'a HashSet<IdentBuf>,
    ) -> impl Iterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)> + 'a {
        let mut inactive = HashSet::new();

        self.plugins.iter().filter_map(move |(name, plugin)| {
            if !enabled_plugins.contains(*name) {
                inactive.insert(*name);
                return None;
            }

            for (dep, _) in plugin.dependencies() {
                if inactive.contains(dep) {
                    return None;
                }
            }

            Some((*name, *plugin))
        })
    }

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
            if !plugin.__running_arcana_instance_check(&GLOBAL_CHECK) {
                miette::bail!(
                    "Plugin '{name}' is linked to wrong Arcana instance",
                    name = name
                );
            }
        }

        Ok(PluginsLibrary {
            lib,
            plugins: plugins.to_vec(),
        })
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

    dialog: Option<PluginsDialog>,

    /// Set of active plugins.
    active_plugins: HashSet<IdentBuf>,

    profile: Profile,
}

enum PluginsDialog {
    NewPlugin(FileDialog),
    FindPlugin(FileDialog),
}

impl Plugins {
    pub fn new() -> Self {
        Plugins {
            linked: None,
            pending: None,
            failure: None,
            build: None,
            dialog: None,
            active_plugins: HashSet::new(),
            profile: get_profile(),
        }
    }

    /// Checks of all plugins from manifest are present in linked library.
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

    // fn all_plugins_pending(&self, project: &ProjectManifest) -> bool {
    //     if let Some(linked) = &self.pending {
    //         return project.plugins.iter().all(|p| linked.has(&p.name));
    //     }
    //     false
    // }

    /// Adds new plugin.
    pub fn add_plugin(
        &mut self,
        name: IdentBuf,
        dep: Dependency,
        project: &mut Project,
    ) -> miette::Result<()> {
        if project.has_plugin(&name) {
            miette::bail!("Plugin '{}' already exists", name);
        }

        let plugin = Plugin::from_dependency(name, dep)?;
        project.add_plugin(plugin)?;

        // Stop current build if there was one.
        tracing::info!(
            "Stopping current build process to re-build plugins library with new plugin"
        );
        self.build = None;

        // Set of active plugins doesn't change yet.
        Ok(())
    }

    pub fn get_plugin(&self, name: &Ident) -> Option<&dyn ArcanaPlugin> {
        self.linked.as_ref()?.get(name)
    }

    pub fn tick(world: &mut World) {
        let world = world.local();
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let mut project = world.expect_resource_mut::<Project>();

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

        if world.view::<With<Game>>().iter().count() == 0 {
            if let Some(mut lib) = plugins.pending.take() {
                tracing::info!("New plugins lib version linked");

                if let Err(err) = lib.sort_plugins(&mut project.plugins_mut()) {
                    for (name, dep) in err.missing_dependencies {
                        tracing::info!("Missing dependency '{name}'");
                        if let Err(err) = plugins.add_plugin(name.to_buf(), dep, &mut project) {
                            tracing::error!("Failed to add missing dependency '{name}'. {err}");
                        }
                    }
                    for (name, dep) in err.circular_dependencies {
                        plugins.failure = Some(miette::miette!(
                            "Circular dependency between '{name}' and '{dep}'",
                            name = name,
                            dep = dep
                        ));
                    }
                }

                let mut data = world.expect_resource_mut::<ProjectData>();
                plugins.active_plugins = lib
                    .active_plugins(&data.enabled_plugins)
                    .map(|(name, _)| name.to_buf())
                    .collect();

                // Update systems and filters.
                let ProjectData {
                    enabled_plugins,
                    systems,
                    ..
                } = &mut *data;

                // Filters::update_plugins(&mut *data, active_plugins);
                world.expect_resource_mut::<Systems>().update_plugins(
                    &mut *systems.borrow_mut(),
                    lib.active_plugins(&*enabled_plugins),
                );

                // Lib is self-consistent.
                // Replace old lib with new one.
                plugins.linked = Some(lib);
            }

            if plugins.failure.is_none()
                && plugins.build.is_none()
                && !plugins.all_plugins_linked(project.manifest())
            {
                // If not building and has no last-build error and not all plugins are linked
                // - rebuild plugins library.

                tracing::info!("Plugins lib is not linked. Building...");
                let build = try_log_err!(project.build_plugins_library(plugins.profile));
                plugins.build = Some(build);
            }
        }
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let mut project = world.expect_resource_mut::<Project>();
        let mut data = world.expect_resource_mut::<ProjectData>();

        let mut sync = false;
        let mut rebuild = false;

        // Building status

        ui.add_enabled_ui(plugins.dialog.is_none(), |ui| {
            ui.allocate_ui_with_layout(
                ui.style().spacing.interact_size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if plugins.build.is_some() {
                        ui.spinner();
                        ui.label("Building");
                    } else if let Some(failure) = &plugins.failure {
                        let r = ui.label(
                            egui::RichText::from("Plugins build: failed")
                                .color(ui.visuals().error_fg_color),
                        );
                        r.on_hover_ui(|ui| {
                            ui.label(failure.to_string());
                        });
                    } else {
                        ui.label("Plugins build: Ok");
                    }
                },
            );

            // Top menu
            ui.horizontal(|ui| {
                let r = match plugins.build.is_none() {
                    false => {
                        ui.add_enabled(false, egui::Button::new(egui_phosphor::regular::HAMMER))
                    }
                    true => ui.button(egui_phosphor::regular::HAMMER),
                };
                if r.clicked() {
                    let build = try_log_err!(project.build_plugins_library(plugins.profile));
                    plugins.build = Some(build);
                }
                let r = ui.button(egui_phosphor::regular::PLUS);

                if r.clicked() {
                    let mut dialog = FileDialog::select_folder(None);
                    dialog.open();
                    plugins.dialog = Some(PluginsDialog::NewPlugin(dialog));
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("New plugin");
                    });
                }

                let r = ui.button(egui_phosphor::regular::FOLDER_OPEN);
                if r.clicked() {
                    let mut dialog = FileDialog::select_folder(None);
                    dialog.open();
                    plugins.dialog = Some(PluginsDialog::FindPlugin(dialog));
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("Add plugin");
                    });
                }
            });

            ui.separator();

            // Plugins list
            let mut remove_plugin = None;

            egui::Grid::new("plugins-list")
                .striped(true)
                .show(ui, |ui| {
                    for (idx, plugin) in project.plugins().iter().enumerate() {
                        let mut heading = RichText::from(plugin.name.as_str());

                        let mut tooltip = "";
                        if !plugins.is_linked(&plugin.name) {
                            // Not linked plugin may not be active.
                            if plugins.pending.is_some() || plugins.build.is_some() {
                                tooltip = "Pending";
                                heading = heading.color(ui.visuals().warn_fg_color);
                            } else {
                                tooltip = "Build failed";
                                heading = heading.color(ui.visuals().error_fg_color);
                            }
                        } else if !data.enabled_plugins.contains(&plugin.name) {
                            heading = heading.color(ui.visuals().warn_fg_color);
                        } else if !plugins.is_active(&plugin.name) {
                            tooltip = "Dependencies are not enabled";
                            heading = heading.color(ui.visuals().warn_fg_color);
                        } else {
                            heading = heading.color(Color32::LIGHT_GREEN);
                        }

                        let was_enabled = data.enabled_plugins.contains(&plugin.name);
                        let mut enabled = was_enabled;
                        let r = ui.checkbox(&mut enabled, heading);

                        if !tooltip.is_empty() {
                            r.on_hover_text(tooltip);
                        }

                        if !was_enabled && enabled {
                            data.enabled_plugins.insert(plugin.name.clone());
                            sync = true;
                        } else if was_enabled && !enabled {
                            data.enabled_plugins.remove(&plugin.name);
                            sync = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let r = ui.button(egui_phosphor::regular::TRASH);
                            if r.clicked() {
                                data.enabled_plugins.remove(&plugin.name);
                                remove_plugin = Some(idx);
                                sync = true;
                                rebuild = true;
                            }
                        });

                        ui.end_row();
                    }
                });

            if let Some(idx) = remove_plugin {
                project.manifest_mut().remove_plugin_idx(idx);
            }
        });

        match &mut plugins.dialog {
            None => {}
            Some(PluginsDialog::FindPlugin(dialog)) => match dialog.show(ui.ctx()).state() {
                egui_file::State::Open => {}
                egui_file::State::Closed | egui_file::State::Cancelled => {
                    plugins.dialog = None;
                }
                egui_file::State::Selected => match dialog.path() {
                    None => {
                        plugins.dialog = None;
                    }
                    Some(path) => {
                        match Utf8Path::from_path(path) {
                            Some(path) => {
                                match add_plugin_with_path(path.to_path_buf(), &mut project) {
                                    Ok(true) => {
                                        sync = true;
                                        rebuild = true;
                                    }
                                    Ok(false) => {
                                        tracing::warn!("Plugin already exists");
                                    }
                                    Err(err) => {
                                        tracing::error!("Failed to add plugin. {err}");
                                    }
                                }
                            }
                            None => {
                                tracing::error!("Invalid plugin path '{}'", path.display());
                            }
                        }
                        plugins.dialog = None;
                    }
                },
            },
            Some(PluginsDialog::NewPlugin(dialog)) => match dialog.show(ui.ctx()).state() {
                egui_file::State::Open => {}
                egui_file::State::Closed | egui_file::State::Cancelled => {
                    plugins.dialog = None;
                }
                egui_file::State::Selected => match dialog.path() {
                    None => {
                        plugins.dialog = None;
                    }
                    Some(path) => {
                        match Utf8Path::from_path(path) {
                            Some(path) => match process_path_name(path.as_std_path(), None) {
                                Ok((path, name)) => match Utf8PathBuf::from_path_buf(path) {
                                    Ok(path) => {
                                        match new_plugin_crate(
                                            &name,
                                            &path,
                                            project.engine().clone(),
                                        ) {
                                            Ok(plugin) => match project.add_plugin(plugin) {
                                                Ok(true) => {
                                                    sync = true;
                                                    rebuild = true;
                                                }
                                                Ok(false) => {
                                                    tracing::warn!("Plugin already exists");
                                                }
                                                Err(err) => {
                                                    tracing::error!("Failed to add plugin. {err}");
                                                }
                                            },
                                            Err(err) => {
                                                tracing::error!(
                                                    "Failed to create new plugin. {err}"
                                                );
                                            }
                                        }
                                    }
                                    Err(path) => {
                                        tracing::error!(
                                            "Plugin path is not UTF-8: {}",
                                            path.display()
                                        );
                                    }
                                },
                                Err(err) => {
                                    tracing::error!("Failed to process plugin path. {err}");
                                }
                            },
                            None => {
                                tracing::error!("Invalid plugin path '{}'", path.display());
                            }
                        }
                        plugins.dialog = None;
                    }
                },
            },
        }

        if sync {
            if let Some(lib) = &plugins.linked {
                plugins.active_plugins = lib
                    .active_plugins(&data.enabled_plugins)
                    .map(|(name, _)| name.to_buf())
                    .collect();

                let ProjectData {
                    enabled_plugins,
                    systems,
                    ..
                } = &mut *data;

                // Filters::update_plugins(&mut *data, active_plugins);
                world.expect_resource_mut::<Systems>().update_plugins(
                    &mut *systems.borrow_mut(),
                    lib.active_plugins(&*enabled_plugins),
                );
            }

            try_log_err!(sync_project(&project, &data));
        }

        if rebuild {
            plugins.build = None;
            try_log_err!(project.init_workspace());
            plugins.build = ok_log_err!(project.build_plugins_library(plugins.profile));
        }
    }

    pub fn tab() -> Tab {
        Tab::Plugins
    }

    /// Checks if plugins with given name is active.
    pub fn is_active(&self, name: &Ident) -> bool {
        self.active_plugins.contains(name)
    }

    /// Checks if plugins with given name is active.
    pub fn is_linked(&self, name: &Ident) -> bool {
        self.linked.as_ref().map_or(false, |lib| lib.has(name))
    }

    pub fn active_plugins<'a>(
        &'a self,
    ) -> Option<impl Iterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)> + 'a> {
        let lib = self.linked.as_ref()?;
        Some(lib.active_plugins(&self.active_plugins))
    }
}

/// Adds new plugins library
fn add_plugin_with_path(path: Utf8PathBuf, project: &mut Project) -> miette::Result<bool> {
    let plugin = Plugin::open_local(path)?;

    project.add_plugin(plugin)
}
