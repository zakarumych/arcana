use arcana::{
    Name,
    error::{Error, fail},
    hash::HashSet,
};
use camino::Utf8Path;

use crate::{
    get_profile,
    project::{BuildProcess, Dependency, Plugin, Profile, Project, ProjectData, new_plugin_crate},
    toaster::Toaster,
};

use super::container::{Loader, Plugins, PluginsError};

/// Tool to manage plugins libraries
/// and enable/disable self.
pub struct PluginManager {
    loader: Loader,

    /// Currently linked plugins container.
    /// It was broadcasted to all other parts of the App.
    linked: Option<Plugins>,

    /// Same as `linked` until taken by the App.
    updated: Option<Plugins>,

    // Pending plugins container.
    // Will become linked on first occasion.
    pending: Option<Plugins>,

    /// Running build process.
    /// Unset when build is finished.
    build: Option<BuildProcess>,

    /// Last plugins build failure report.
    last_build_failure: Option<Error>,

    profile: Profile,

    /// List of all plugins in the project.
    plugins: Vec<Plugin>,

    /// Set of enabled plugins.
    enabled_plugins: HashSet<Name>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            loader: Loader::new(),
            linked: None,
            updated: None,
            pending: None,
            build: None,
            last_build_failure: None,
            profile: get_profile(),
            plugins: Vec::new(),
            enabled_plugins: HashSet::default(),
        }
    }

    pub fn load(&mut self, project: &Project, data: &ProjectData) {
        self.plugins = project.manifest().plugins.clone();
        self.enabled_plugins = data.enabled_plugins.clone();
    }

    pub fn save(&self, project: &mut Project, data: &mut ProjectData) {
        project.manifest_mut().plugins = self.plugins.clone();
        data.enabled_plugins = self.enabled_plugins.clone();
    }

    /// Checks of all plugins are present in linked library.
    fn check_plugins(plugins: &[Plugin], container: &Plugins) -> bool {
        plugins.iter().all(|p| {
            let has = container.has(p.name);
            if !has {
                tracing::debug!("Plugin '{}' is not linked", p.name);
            }
            has
        })
    }

    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn has_plugin(&self, name: Name) -> bool {
        self.plugins.iter().any(|x| x.name == name)
    }

    /// Adds plugin to project.
    pub fn load_plugin(&mut self, name: Name, dependency: Dependency) -> Result<(), Error> {
        let plugin = Plugin::from_dependency(name.to_string(), dependency)?;
        self.add_plugin(plugin)
    }

    /// Adds plugin to project.
    pub fn add_plugin(&mut self, plugin: Plugin) -> Result<(), Error> {
        if self.plugins.iter().any(|x| x.name == plugin.name) {
            fail!("Plugin '{}' already exists", plugin.name);
        }

        self.plugins.push(plugin);

        if self.build.is_some() {
            // Stop current build if there was one.
            tracing::info!(
                "Stopping current build process to re-build plugins library with new plugin"
            );
            self.build = None;
        }

        // Set of active plugins doesn't change yet.
        Ok(())
    }

    /// Creates new local plugin and adds it to the project.
    pub fn new_plugin(
        &mut self,
        name: String,
        path: &Utf8Path,
        project: &Project,
    ) -> Result<(), Error> {
        if self.plugins.iter().any(|x| x.name == name) {
            fail!("Plugin '{}' already exists", name);
        }

        match new_plugin_crate(
            &name,
            path,
            project.engine().clone(),
            Some(project.root_path()),
        ) {
            Ok(plugin) => {
                self.plugins.push(plugin);
            }
            Err(error) => {
                fail!("Failed to create new plugin. {error:?}");
            }
        }

        Ok(())
    }

    pub fn remove_plugin(&mut self, name: Name) {
        self.plugins.retain(|x| x.name != name);
        self.enabled_plugins.remove(&name);
    }

    pub fn is_plugin_enabled(&self, name: Name) -> bool {
        self.enabled_plugins.contains(&name)
    }

    pub fn enable_plugin(&mut self, name: Name) {
        debug_assert!(self.has_plugin(name));
        self.enabled_plugins.insert(name);
    }

    pub fn disable_plugin(&mut self, name: Name) {
        debug_assert!(self.has_plugin(name));
        self.enabled_plugins.remove(&name);
    }

    pub fn take_updated(&mut self) -> Option<Plugins> {
        self.updated.take()
    }

    pub fn tick(&mut self, toaster: &mut Toaster, project: &Project) {
        if let Some(mut build) = self.build.take() {
            match build.finished() {
                None => self.build = Some(build),
                Some(Ok(())) => {
                    tracing::info!(
                        "Finished building plugins library {}",
                        build.artifact().display()
                    );
                    let path = build.artifact();
                    match self.loader.load(&path, &self.enabled_plugins) {
                        Ok(container) => {
                            if !Self::check_plugins(&self.plugins, &container) {
                                tracing::warn!("Not all plugins are linked. Rebuilding");
                                self.build =
                                    ok_log_err!(project.build_plugins_library(self.profile));
                            } else {
                                tracing::info!(
                                    "New plugins container version pending. {container:#?}"
                                );
                                toaster.push_info("Plugins library updated.".to_string());
                                self.pending = Some(container);
                                self.unset_failure();
                            }
                        }
                        Err(error) => {
                            let mut rebuild = false;
                            tracing::error!("Failed to load plugins library. {error:?}");

                            if let Some(plugins_error) = error.downcast_ref::<PluginsError>() {
                                for md in plugins_error.missing_dependencies.iter() {
                                    rebuild = true;
                                    tracing::error!("Missing dependency: {md:?}");

                                    if let Err(error) =
                                        self.load_plugin(md.plugin, md.dependency.clone())
                                    {
                                        tracing::error!(
                                            "Failed to add missing dependency. {error:?}"
                                        );
                                    }
                                }

                                if !plugins_error.circular_dependencies.is_empty() {
                                    toaster.push_error(
                                        "Failed to load plugins library.".to_string(),
                                        &error,
                                    );
                                    self.set_build_failire(error);
                                }
                            } else {
                                toaster.push_error(
                                    "Failed to load plugins library.".to_string(),
                                    &error,
                                );
                                self.set_build_failire(error);
                            }

                            if rebuild {
                                match project.build_plugins_library(self.profile) {
                                    Ok(build) => {
                                        self.build = Some(build);
                                    }
                                    Err(error) => {
                                        toaster.push_error(
                                            "Failed to build plugins library.".to_string(),
                                            &error,
                                        );
                                        self.set_build_failire(error);
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(error)) => {
                    tracing::error!("Failed building plugins library. {error:?}");
                    toaster.push_error("Failed to build plugins library.".to_string(), &error);
                    self.set_build_failire(error);
                }
            }
        }

        match self.pending.take() {
            None => {
                if self.linked.is_none()
                    && self.last_build_failure.is_none()
                    && self.build.is_none()
                {
                    tracing::info!("Make initial plugins library build");

                    match project.build_plugins_library(self.profile) {
                        Ok(build) => {
                            self.build = Some(build);
                        }
                        Err(error) => {
                            toaster
                                .push_error("Failed to build plugins library.".to_string(), &error);
                            self.set_build_failire(error);
                        }
                    }
                }
            }
            Some(c) => {
                tracing::info!("New plugins container version linked. {c:#?}");
                self.linked = Some(c);
                self.updated = self.linked.clone();
            }
        }
    }

    pub fn linked(&self) -> Option<&Plugins> {
        self.linked.as_ref()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn is_building(&self) -> bool {
        self.build.is_some()
    }

    fn unset_failure(&mut self) {
        self.last_build_failure = None;
    }

    fn set_build_failire(&mut self, error: Error) {
        self.last_build_failure = Some(error);
    }

    pub(super) fn last_build_failure(&self) -> Option<&Error> {
        self.last_build_failure.as_ref()
    }

    pub fn start_new_build(&mut self, project: &Project) {
        if let Some(build) = self.build.take() {
            tracing::info!("Cancel build and start new");
            drop(build);
        }

        let build = try_log_err!(project.build_plugins_library(self.profile));
        self.build = Some(build);
    }

    pub fn refresh_enabled_plugins(&mut self) {
        if let Some(c) = &self.pending {
            self.pending = Some(c.with_plugins(&self.enabled_plugins));
        } else if let Some(c) = &self.linked {
            self.pending = Some(c.with_plugins(&self.enabled_plugins));
        }
    }
}
