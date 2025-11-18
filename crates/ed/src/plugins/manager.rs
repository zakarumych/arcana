use arcana::{
    Ident,
    error::{Error, fail},
};
use camino::Utf8Path;

use crate::{
    get_profile,
    project::{
        BuildProcess, Dependency, Plugin, Profile, Project, ProjectManifest, new_plugin_crate,
    },
};

use super::container::{Loader, Plugins, PluginsError};

/// Tool to manage plugins libraries
/// and enable/disable self.
pub struct PluginsManager {
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
    last_failure: Option<Error>,

    profile: Profile,
}

impl PluginsManager {
    pub fn new() -> Self {
        PluginsManager {
            loader: Loader::new(),
            linked: None,
            updated: None,
            pending: None,
            build: None,
            last_failure: None,
            profile: get_profile(),
        }
    }

    /// Checks of all plugins from manifest are present in linked library.
    fn check_plugins(project: &ProjectManifest, container: &Plugins) -> bool {
        project.plugins.iter().all(|p| {
            let has = container.has(p.name);
            if !has {
                tracing::debug!("Plugin '{}' is not linked", p.name);
            }
            has
        })
    }

    /// Adds plugin to project.
    pub fn add_plugin(
        &mut self,
        name: Ident,
        dep: Dependency,
        project: &mut Project,
    ) -> Result<(), Error> {
        if project.has_plugin(name) {
            fail!("Plugin '{}' already exists", name);
        }

        let plugin = Plugin::from_dependency(name, dep)?;
        project.add_plugin(plugin)?;

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

    /// Adds new local plugin
    pub fn new_plugin(
        &mut self,
        name: Ident,
        path: &Utf8Path,
        project: &mut Project,
    ) -> Result<(), Error> {
        if project.has_plugin(name) {
            fail!("Plugin '{}' already exists", name);
        }

        match new_plugin_crate(
            &name,
            path,
            project.engine().clone(),
            Some(project.root_path()),
        ) {
            Ok(plugin) => match project.add_plugin(plugin) {
                Ok(true) => {
                    project.sync();
                    self.build = Some(project.build_plugins_library(self.profile)?);
                }
                Ok(false) => {
                    fail!("Plugin '{}' already exists", name);
                }
                Err(error) => {
                    fail!("Failed to add plugin. {error:?}");
                }
            },
            Err(error) => {
                fail!("Failed to create new plugin. {error:?}");
            }
        }

        Ok(())
    }

    pub fn take_updated(&mut self) -> Option<Plugins> {
        self.updated.take()
    }

    pub fn tick(&mut self, project: &mut Project) {
        if let Some(mut build) = self.build.take() {
            match build.finished() {
                Ok(false) => self.build = Some(build),
                Ok(true) => {
                    tracing::info!(
                        "Finished building plugins library {}",
                        build.artifact().display()
                    );
                    let path = build.artifact();
                    match self.loader.load(&path, &project.data.enabled_plugins) {
                        Ok(container) => {
                            if !Self::check_plugins(project.manifest(), &container) {
                                tracing::warn!("Not all plugins are linked. Rebuilding");
                                self.build =
                                    ok_log_err!(project.build_plugins_library(self.profile));
                            } else {
                                tracing::info!(
                                    "New plugins container version pending. {container:#?}"
                                );
                                self.pending = Some(container);
                                self.last_failure = None;
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
                                        self.add_plugin(md.plugin, md.dependency.clone(), project)
                                    {
                                        tracing::error!(
                                            "Failed to add missing dependency. {error:?}"
                                        );
                                    }
                                }

                                if !plugins_error.circular_dependencies.is_empty() {
                                    self.last_failure = Some(error);
                                }
                            } else {
                                self.last_failure = Some(error);
                            }

                            if rebuild {
                                try_log_err!(project.sync());

                                match project.build_plugins_library(self.profile) {
                                    Ok(build) => {
                                        self.build = Some(build);
                                    }
                                    Err(error) => {
                                        self.last_failure = Some(error);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    tracing::error!("Failed building plugins library. {error:?}");
                    self.last_failure = Some(error);
                }
            }
        }

        match self.pending.take() {
            None => {
                if self.linked.is_none() && self.last_failure.is_none() && self.build.is_none() {
                    tracing::info!("Make initial plugins library build");

                    match project.build_plugins_library(self.profile) {
                        Ok(build) => {
                            self.build = Some(build);
                        }
                        Err(error) => {
                            self.last_failure = Some(error);
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

    pub fn last_failure(&self) -> Option<&Error> {
        self.last_failure.as_ref()
    }

    pub fn start_new_build(&mut self, project: &Project) {
        if let Some(build) = self.build.take() {
            tracing::info!("Cancel build and start new");
            drop(build);
        }

        let build = try_log_err!(project.build_plugins_library(self.profile));
        self.build = Some(build);
    }

    pub fn refresh_enabled_plugins(&mut self, project: &Project) {
        if let Some(c) = &self.pending {
            self.pending = Some(c.with_plugins(&project.data.enabled_plugins));
        } else if let Some(c) = &self.linked {
            self.pending = Some(c.with_plugins(&project.data.enabled_plugins));
        }
    }
}
