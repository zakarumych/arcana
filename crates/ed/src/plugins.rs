use std::path::Path;

use arcana::{
    edict::world::WorldLocal,
    game::Game,
    plugin::ArcanaPlugin,
    project::{
        plugin_with_path, BuildProcess, Dependency, Ident, IdentBuf, Project, ProjectManifest,
    },
    With, World,
};
use egui::{Color32, Ui, WidgetText};
use hashbrown::HashSet;

use super::Tab;

mod private {
    use std::{collections::VecDeque, fmt, path::Path};

    use arcana::{
        plugin::{ArcanaPlugin, GLOBAL_CHECK},
        project::{Dependency, Ident, IdentBuf, Plugin},
    };
    use hashbrown::{HashMap, HashSet};

    pub(super) struct PluginsLibrary {
        /// Linked library
        #[allow(unused)]
        lib: libloading::Library,
        plugins: &'static [(&'static Ident, &'static dyn ArcanaPlugin)],
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

    pub(super) struct SortError {
        pub not_linked: Vec<IdentBuf>,
        pub circular_dependencies: Vec<(IdentBuf, IdentBuf)>,
        pub missing_dependencies: Vec<(IdentBuf, Dependency)>,
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

        /// Sort plugins placing dependencies first.
        /// Errors on circular dependencies, missing dependencies and not linked plugins.
        pub fn sort_plugins(&self, plugins: &[Plugin]) -> Result<Vec<Plugin>, SortError> {
            let mut queue = VecDeque::new();
            let mut items = HashMap::new();

            let mut error = SortError {
                not_linked: Vec::new(),
                circular_dependencies: Vec::new(),
                missing_dependencies: Vec::new(),
            };

            for plugin in plugins.iter() {
                if !self.has(&*plugin.name) {
                    error.not_linked.push(plugin.name.to_buf());
                    continue;
                }

                queue.push_back(&*plugin.name);
                items.insert(&*plugin.name, plugin);
            }

            let mut pending = HashSet::new();
            let mut sorted = HashSet::new();
            let mut result = Vec::new();

            while let Some(name) = queue.pop_front() {
                if sorted.contains(name) {
                    continue;
                }
                pending.insert(name);

                let plugin = items[name];
                let a = self.get(name).unwrap();

                let mut defer = false;
                for &dep in a.dependencies() {
                    if sorted.contains(dep) {
                        continue;
                    }
                    if pending.contains(dep) {
                        error
                            .circular_dependencies
                            .push((name.to_buf(), dep.to_buf()));
                        continue;
                    }

                    if !items.contains_key(dep) {
                        error
                            .missing_dependencies
                            .push((dep.to_buf(), a.get_dependency(dep)));
                        continue;
                    };

                    if !defer {
                        defer = true;
                        queue.push_front(name);
                    }

                    queue.push_front(dep);
                }

                if !defer {
                    sorted.insert(name);
                    result.push(plugin.clone());
                }
            }

            if error.not_linked.is_empty()
                && error.circular_dependencies.is_empty()
                && error.missing_dependencies.is_empty()
            {
                Ok(result)
            } else {
                Err(error)
            }
        }

        /// List active plugins
        /// e.g. enabled plugins for which all dependencies are active
        pub fn active_plugins<'a>(&self, plugins: &'a [Plugin]) -> HashSet<IdentBuf> {
            let mut active = HashSet::new();
            let mut inactive = HashSet::new();

            'p: for plugin in plugins.iter() {
                if !plugin.enabled {
                    inactive.insert(&*plugin.name);
                    continue;
                }
                let Some(a) = self.get(&*plugin.name) else {
                    inactive.insert(&*plugin.name);
                    continue;
                };

                let mut queue = VecDeque::new();
                let mut pending = HashSet::new();

                for &dep in a.dependencies() {
                    if active.contains(dep) {
                        continue;
                    }
                    if inactive.contains(dep) {
                        inactive.insert(&*plugin.name);
                        continue 'p;
                    }
                    queue.push_back(dep);
                }

                while let Some(name) = queue.pop_front() {
                    match plugins.iter().find(|p| *p.name == *name) {
                        None => {
                            inactive.insert(&*plugin.name);
                            continue 'p;
                        }
                        Some(plugin) => {
                            if !plugin.enabled {
                                inactive.insert(&*plugin.name);
                                continue 'p;
                            }
                        }
                    }

                    let Some(a) = self.get(name) else {
                        inactive.insert(&*plugin.name);
                        continue 'p;
                    };
                    pending.insert(name);

                    let mut defer = false;
                    for &dep in a.dependencies() {
                        if active.contains(dep) {
                            continue;
                        }
                        if inactive.contains(dep) {
                            inactive.insert(&*plugin.name);
                            continue 'p;
                        }
                        if pending.contains(dep) {
                            inactive.insert(&*plugin.name);
                            continue 'p;
                        }

                        if !defer {
                            defer = true;
                            queue.push_front(name);
                        }
                        queue.push_front(dep);
                    }

                    if !defer {
                        active.insert(name);
                    }
                }

                active.insert(&*plugin.name);
            }

            active.into_iter().map(|name| name.to_buf()).collect()
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

    /// Set of active plugins.
    active_plugins: HashSet<IdentBuf>,
}

impl Plugins {
    pub fn new() -> Self {
        Plugins {
            linked: None,
            pending: None,
            failure: None,
            build: None,
            active_plugins: HashSet::new(),
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

    // fn all_plugins_pending(&self, project: &ProjectManifest) -> bool {
    //     if let Some(linked) = &self.pending {
    //         return project.plugins.iter().all(|p| linked.has(&p.name));
    //     }
    //     false
    // }

    /// Adds new plugin.
    pub fn add_plugin(&mut self, name: IdentBuf, dep: Dependency, project: &mut Project) -> bool {
        if project.add_plugin(name, dep) {
            // Stop current build if there was one.
            tracing::info!(
                "Stopping current build process to re-build plugins library with new plugin"
            );
            self.build = None;

            // Set of active plugins doesn't change yet.
            true
        } else {
            false
        }
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
            if let Some(lib) = plugins.pending.take() {
                tracing::info!("New plugins lib version linked");

                match lib.sort_plugins(&project.manifest().plugins) {
                    Ok(sorted) => {
                        project.manifest_mut().plugins = sorted;

                        for (name, plugin) in lib.list() {
                            for system in plugin.systems() {
                                project.manifest_mut().add_system(name, system, true);
                            }
                            for filter in plugin.filters() {
                                project.manifest_mut().add_filter(name, filter, true);
                            }
                        }

                        plugins.active_plugins = lib.active_plugins(&project.manifest().plugins);
                        plugins.linked = Some(lib);
                    }
                    Err(err) => {
                        for name in err.not_linked {
                            tracing::info!("Plugin '{name}' is not linked", name = name);
                        }
                        for (name, dep) in err.missing_dependencies {
                            tracing::info!("Missing dependency '{name}'");
                            plugins.add_plugin(name.to_buf(), dep, &mut project);
                        }
                        for (name, dep) in err.circular_dependencies {
                            plugins.failure = Some(miette::miette!(
                                "Circular dependency between '{name}' and '{dep}'",
                                name = name,
                                dep = dep
                            ));
                        }
                    }
                }
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

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let plugins = &mut *world.expect_resource_mut::<Plugins>();
        let mut project = world.expect_resource_mut::<Project>();
        let mut sync = false;
        let mut rebuild = false;

        // Building status

        ui.allocate_ui_with_layout(
            ui.style().spacing.interact_size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if plugins.build.is_some() {
                    ui.spinner();
                    ui.label("Building");
                } else if let Some(failure) = &plugins.failure {
                    let r = ui.label("Build failed");
                    r.on_hover_ui(|ui| {
                        ui.label(failure.to_string());
                    });
                } else {
                    ui.label("Build succeeded");
                }
            },
        );

        // Top menu
        ui.horizontal(|ui| {
            let r = match plugins.build.is_none() {
                false => ui.add_enabled(false, egui::Button::new(egui_phosphor::regular::HAMMER)),
                true => ui.button(egui_phosphor::regular::HAMMER),
            };
            if r.clicked() {
                let build = try_log_err!(project.build_plugins_library());
                plugins.build = Some(build);
            }
            let r = ui.button(egui_phosphor::regular::PLUS);

            if r.clicked() {
                if let Some(path) = rfd::FileDialog::new().save_file() {
                    if add_plugin_with_path(&path, &mut project) {
                        sync = true;
                        rebuild = true;
                    }
                }
            } else {
                r.on_hover_ui(|ui| {
                    ui.label("New plugin");
                });
            }

            let r = ui.button(egui_phosphor::regular::FOLDER_OPEN);
            if r.clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("Cargo.toml")
                    .pick_file()
                {
                    if add_plugin_with_path(&path, &mut project) {
                        sync = true;
                        rebuild = true;
                    }
                }
            } else {
                r.on_hover_ui(|ui| {
                    ui.label("Add plugin");
                });
            }
        });

        // Plugins list
        let mut remove_plugin = None;
        let mut toggle_plugin = None;

        for (idx, plugin) in project.manifest().plugins.iter().enumerate() {
            let mut heading = WidgetText::from(plugin.name.as_str());

            let mut tooltip = "";
            if !plugins.is_linked(&plugin.name) {
                // Not linked plugin may not be active.
                if plugins.pending.is_some() || plugins.build.is_some() {
                    tooltip = "Pending";
                    heading = heading.color(Color32::KHAKI);
                } else {
                    tooltip = "Build failed";
                    heading = heading.color(Color32::DARK_RED);
                }
            } else if !plugins.is_active(&plugin.name) {
                tooltip = "Dependencies are not enabled";
                heading = heading.color(Color32::KHAKI);
            } else {
                heading = heading.color(Color32::GREEN);
            }

            ui.horizontal(|ui| {
                let mut enabled = plugin.enabled;
                let r = ui.checkbox(&mut enabled, heading);

                if plugin.enabled != enabled {
                    toggle_plugin = Some(idx);
                }
                r.on_disabled_hover_text(tooltip);

                let r = ui.button(egui_phosphor::regular::TRASH);
                if r.clicked() {
                    remove_plugin = Some(idx);
                }
            });
        }

        if let Some(idx) = toggle_plugin {
            let plugin = &mut project.manifest_mut().plugins[idx];
            plugin.enabled = !plugin.enabled;
            sync = true;
        }

        if let Some(idx) = remove_plugin {
            project.manifest_mut().remove_plugin_idx(idx);
            sync = true;
            rebuild = true;
        }

        if sync {
            if let Some(linked) = &plugins.linked {
                plugins.active_plugins = linked.active_plugins(&project.manifest().plugins);
            }
            try_log_err!(project.sync());
        }

        if rebuild {
            plugins.build = None;
            try_log_err!(project.init_workspace());
            plugins.build = ok_log_err!(project.build_plugins_library());
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

    /// List all active plugins
    pub fn active_plugins<'a>(&'a self) -> Option<Vec<(&'a Ident, &'a dyn ArcanaPlugin)>> {
        let linked = self.linked.as_ref()?;

        let mut list = Vec::new();
        for (name, plugin) in linked.list() {
            if self.active_plugins.contains(name) {
                list.push((name, plugin));
            }
        }
        Some(list)
    }
}

/// Adds new plugins library.
fn add_plugin_with_path(path: &Path, project: &mut Project) -> bool {
    let (name, dep) = try_log_err!(plugin_with_path(path); false);
    project.add_plugin(name, dep)
}
