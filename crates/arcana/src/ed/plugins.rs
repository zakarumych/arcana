use arcana::{
    project::{
        new_plugin_crate, process_path_ident, BuildProcess, Dependency, Plugin, Profile, Project,
        ProjectManifest,
    },
    Ident,
};
use camino::{Utf8Path, Utf8PathBuf};
use egui::{Color32, RichText, Ui};
use egui_file::FileDialog;

use super::{
    container::{Container, Loader, PluginsError},
    data::ProjectData,
    get_profile,
};

/// Tool to manage plugins libraries
/// and enable/disable self.
pub(super) struct Plugins {
    loader: Loader,

    // Pending plugins container.
    // Will become linked on first occasion.
    pending: Option<Container>,

    /// Displaying plugins build failure report.
    /// Unset when build is successful or report widget is closed.
    failure: Option<miette::Report>,

    /// Running build process.
    /// Unset when build is finished.
    build: Option<BuildProcess>,

    /// Open dialog widget.
    dialog: Option<PluginsDialog>,

    profile: Profile,
}

enum PluginsDialog {
    NewPlugin(FileDialog),
    FindPlugin(FileDialog),
}

impl Plugins {
    pub fn new() -> Self {
        Plugins {
            loader: Loader::new(),
            pending: None,
            failure: None,
            build: None,
            dialog: None,
            profile: get_profile(),
        }
    }

    /// Checks of all plugins from manifest are present in linked library.
    fn check_plugins(project: &ProjectManifest, container: &Container) -> bool {
        project.plugins.iter().all(|p| {
            let has = container.has(p.name);
            if !has {
                tracing::debug!("Plugin '{}' is not linked", p.name);
            }
            has
        })
    }

    /// Adds new plugin.
    pub fn add_plugin(
        &mut self,
        name: Ident,
        dep: Dependency,
        project: &mut Project,
    ) -> miette::Result<()> {
        if project.has_plugin(name) {
            miette::bail!("Plugin '{}' already exists", name);
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

    pub fn tick(
        &mut self,
        project: &mut Project,
        data: &ProjectData,
        need_build: bool,
    ) -> Option<Container> {
        if let Some(mut build) = self.build.take() {
            match build.finished() {
                Ok(false) => self.build = Some(build),
                Ok(true) => {
                    tracing::info!(
                        "Finished building plugins library {}",
                        build.artifact().display()
                    );
                    let path = build.artifact();
                    match self.loader.load(&path, &data.enabled_plugins) {
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
                                self.failure = None;
                            }
                        }
                        Err(mut err) => {
                            let mut rebuild = false;
                            tracing::error!("Failed to load plugins library. {err:?}");

                            if let Some(err) = err.downcast_mut::<PluginsError>() {
                                for md in err.missing_dependencies.drain(..) {
                                    rebuild = true;
                                    tracing::error!("Missing dependency: {md:?}");

                                    if let Err(err) =
                                        self.add_plugin(md.plugin, md.dependency, project)
                                    {
                                        tracing::error!(
                                            "Failed to add missing dependency. {err:?}"
                                        );
                                    }
                                }
                            }

                            if let Some(mut related) = err.related() {
                                for err in &mut related {
                                    tracing::error!("Related error: {err:?}");
                                }
                            }

                            self.failure = Some(err);

                            if rebuild {
                                try_log_err!(project.sync(); None);

                                match project.build_plugins_library(self.profile) {
                                    Ok(build) => {
                                        self.build = Some(build);
                                    }
                                    Err(err) => {
                                        self.failure = Some(err);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("Failed building plugins library. {err:?}");
                    self.failure = Some(err);
                }
            }
        }

        match self.pending.take() {
            None => {
                if need_build && self.failure.is_none() && self.build.is_none() {
                    tracing::info!("Make initial plugins library build");

                    match project.build_plugins_library(self.profile) {
                        Ok(build) => {
                            self.build = Some(build);
                        }
                        Err(err) => {
                            self.failure = Some(err);
                        }
                    }
                }
                None
            }
            Some(c) => {
                tracing::info!("New plugins container version linked. {c:#?}");
                Some(c)
            }
        }
    }

    pub fn show(
        &mut self,
        linked: Option<&Container>,
        project: &mut Project,
        data: &mut ProjectData,
        ui: &mut Ui,
    ) {
        let mut sync = false;
        let mut rebuild = false;

        // Building status

        ui.add_enabled_ui(self.dialog.is_none(), |ui| {
            ui.allocate_ui_with_layout(
                ui.style().spacing.interact_size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if self.build.is_some() {
                        ui.spinner();
                        ui.label("Building");
                    } else if let Some(failure) = &self.failure {
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
                let r = match self.build.is_none() {
                    false => {
                        ui.add_enabled(false, egui::Button::new(egui_phosphor::regular::HAMMER))
                    }
                    true => ui.button(egui_phosphor::regular::HAMMER),
                };
                if r.clicked() {
                    let build = try_log_err!(project.build_plugins_library(self.profile));
                    self.build = Some(build);
                }
                let r = ui.button(egui_phosphor::regular::PLUS);

                if r.clicked() {
                    let mut dialog = FileDialog::select_folder(None);
                    dialog.open();
                    self.dialog = Some(PluginsDialog::NewPlugin(dialog));
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("New plugin");
                    });
                }

                let r = ui.button(egui_phosphor::regular::FOLDER_OPEN);
                if r.clicked() {
                    let mut dialog = FileDialog::select_folder(None);
                    dialog.open();
                    self.dialog = Some(PluginsDialog::FindPlugin(dialog));
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
                        if !linked.map_or(false, |c| c.has(plugin.name)) {
                            // Not linked plugin may not be active.
                            if self.pending.is_some() || self.build.is_some() {
                                tooltip = "Pending";
                                heading = heading.color(ui.visuals().warn_fg_color);
                            } else {
                                tooltip = "Plugin is missing in library";
                                heading = heading.color(ui.visuals().error_fg_color);
                            }
                        } else if !data.enabled_plugins.contains(&plugin.name) {
                            heading = heading.color(ui.visuals().warn_fg_color);
                        } else if !linked.map_or(false, |c| c.is_active(plugin.name)) {
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

        match &mut self.dialog {
            None => {}
            Some(PluginsDialog::FindPlugin(dialog)) => match dialog.show(ui.ctx()).state() {
                egui_file::State::Open => {}
                egui_file::State::Closed | egui_file::State::Cancelled => {
                    self.dialog = None;
                }
                egui_file::State::Selected => match dialog.path() {
                    None => {
                        self.dialog = None;
                    }
                    Some(path) => {
                        match Utf8Path::from_path(path) {
                            Some(path) => match add_plugin_with_path(path.to_path_buf(), project) {
                                Ok(true) => {
                                    sync = true;
                                    rebuild = true;
                                }
                                Ok(false) => {
                                    tracing::warn!("Plugin already exists");
                                }
                                Err(err) => {
                                    tracing::error!("Failed to add plugin. {err:?}");
                                }
                            },
                            None => {
                                tracing::error!("Invalid plugin path '{}'", path.display());
                            }
                        }
                        self.dialog = None;
                    }
                },
            },
            Some(PluginsDialog::NewPlugin(dialog)) => match dialog.show(ui.ctx()).state() {
                egui_file::State::Open => {}
                egui_file::State::Closed | egui_file::State::Cancelled => {
                    self.dialog = None;
                }
                egui_file::State::Selected => match dialog.path() {
                    None => {
                        self.dialog = None;
                    }
                    Some(path) => {
                        match Utf8Path::from_path(path) {
                            Some(path) => match process_path_ident(path.as_std_path(), None) {
                                Ok((path, name)) => match Utf8PathBuf::from_path_buf(path) {
                                    Ok(path) => {
                                        match new_plugin_crate(
                                            &name,
                                            &path,
                                            project.engine().clone(),
                                            Some(project.root_path()),
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
                                                    tracing::error!(
                                                        "Failed to add plugin. {err:?}"
                                                    );
                                                }
                                            },
                                            Err(err) => {
                                                tracing::error!(
                                                    "Failed to create new plugin. {err:?}"
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
                                    tracing::error!("Failed to process plugin path. {err:?}");
                                }
                            },
                            None => {
                                tracing::error!("Invalid plugin path '{}'", path.display());
                            }
                        }
                        self.dialog = None;
                    }
                },
            },
        }

        assert!(sync || !rebuild, "Rebuild without sync");

        if sync {
            try_log_err!(data.sync(&project));

            if rebuild {
                try_log_err!(project.sync());

                self.build = None;
                self.pending = None;
                try_log_err!(project.init_workspace());
                self.build = ok_log_err!(project.build_plugins_library(self.profile));
            }

            if let Some(c) = &self.pending {
                self.pending = Some(c.with_plugins(&data.enabled_plugins));
            } else if let Some(c) = &linked {
                self.pending = Some(c.with_plugins(&data.enabled_plugins));
            }
        }
    }
}

/// Adds new plugins library
fn add_plugin_with_path(path: Utf8PathBuf, project: &mut Project) -> miette::Result<bool> {
    let plugin = Plugin::open_local(path)?;

    project.add_plugin(plugin)
}
