use std::path::{PathBuf, absolute};

use arcana::{Ident, error::Error, validate_ident};
use arcana_project::Plugin;
use camino::{Utf8Path, Utf8PathBuf};
use egui::{Color32, RichText, Ui};
use egui_file::FileDialog;

use crate::{error::Errors, project::Project};

use super::PluginsManager;

/// Widget to control [`PluginsManager`]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PluginsWidget {
    /// Open dialog widget.
    #[serde(skip)]
    dialog: Option<PluginsDialog>,
}

enum PluginsDialog {
    NewPlugin(NewPlugin),
    FindPlugin(FileDialog),
}

struct NewPlugin {
    name: String,
    path: String,
    real_path: Utf8PathBuf,
    path_dialog: Option<FileDialog>,
    ready: bool,
}

impl PluginsWidget {
    pub fn new() -> Self {
        PluginsWidget { dialog: None }
    }

    pub fn show(
        &mut self,
        manager: &mut PluginsManager,
        project: &mut Project,
        errors: &mut Errors,
        ui: &mut Ui,
    ) {
        // Building status
        let mut sync_project = false;
        let mut rebuild_plugins = false;

        ui.add_enabled_ui(self.dialog.is_none(), |ui| {
            ui.allocate_ui_with_layout(
                ui.style().spacing.interact_size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if manager.is_building() {
                        ui.spinner();
                        ui.label("Building");
                    } else if let Some(failure) = manager.last_failure() {
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
                let r = match !manager.is_building() {
                    false => {
                        ui.add_enabled(false, egui::Button::new(egui_phosphor::regular::HAMMER))
                    }
                    true => ui.button(egui_phosphor::regular::HAMMER),
                };
                if r.clicked() {
                    manager.start_new_build(project);
                }
                let r = ui.button(egui_phosphor::regular::PLUS);

                if r.clicked() {
                    self.dialog = Some(PluginsDialog::NewPlugin(NewPlugin {
                        name: String::new(),
                        path: String::new(),
                        real_path: Utf8PathBuf::new(),
                        path_dialog: None,
                        ready: false,
                    }));
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("New plugin");
                    });
                }

                let r = ui.button(egui_phosphor::regular::FOLDER_OPEN);
                if r.clicked() {
                    let mut dialog = FileDialog::select_folder(std::env::current_dir().ok());
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
                    for (idx, plugin) in project.inner.plugins().iter().enumerate() {
                        let mut heading = RichText::from(plugin.name.as_str());

                        let mut tooltip = "";
                        if !manager.linked().map_or(false, |c| c.has(plugin.name)) {
                            // Not linked plugin may not be active.
                            if manager.has_pending() || manager.is_building() {
                                tooltip = "Pending";
                                heading = heading.color(ui.visuals().warn_fg_color);
                            } else {
                                tooltip = "Plugin is missing in library";
                                heading = heading.color(ui.visuals().error_fg_color);
                            }
                        } else if !project.data.enabled_plugins.contains(&plugin.name) {
                            heading = heading.color(ui.visuals().warn_fg_color);
                        } else if !manager.linked().map_or(false, |c| c.is_active(plugin.name)) {
                            tooltip = "Dependencies are not enabled";
                            heading = heading.color(ui.visuals().warn_fg_color);
                        } else {
                            heading = heading.color(Color32::LIGHT_GREEN);
                        }

                        let was_enabled = project.data.enabled_plugins.contains(&plugin.name);
                        let mut enabled = was_enabled;
                        let r = ui.checkbox(&mut enabled, heading);

                        if !tooltip.is_empty() {
                            r.on_hover_text(tooltip);
                        }

                        if !was_enabled && enabled {
                            project.data.enabled_plugins.insert(plugin.name.clone());
                            sync_project = true;
                        } else if was_enabled && !enabled {
                            project.data.enabled_plugins.remove(&plugin.name);
                            sync_project = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let r = ui.button(egui_phosphor::regular::TRASH);
                            if r.clicked() {
                                project.data.enabled_plugins.remove(&plugin.name);
                                remove_plugin = Some(idx);
                                sync_project = true;
                                rebuild_plugins = true;
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
                                    project.sync();
                                }
                                Ok(false) => {
                                    tracing::warn!("Plugin already exists");
                                }
                                Err(error) => {
                                    tracing::error!("Failed to add plugin. {error:?}");
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
            Some(PluginsDialog::NewPlugin(new_plugin)) => {
                let mut close = false;

                if let Some(path_dialog) = &mut new_plugin.path_dialog {
                    match path_dialog.show(ui.ctx()).state() {
                        egui_file::State::Open => {}
                        egui_file::State::Closed | egui_file::State::Cancelled => {
                            new_plugin.path_dialog = None;
                        }
                        egui_file::State::Selected => match path_dialog.path() {
                            None => {
                                new_plugin.path_dialog = None;
                            }
                            Some(path) => {
                                if let Some(path) = Utf8Path::from_path(path) {
                                    new_plugin.path = path.to_string();
                                } else {
                                    tracing::error!("Invalid plugin path '{}'", path.display());
                                }
                                new_plugin.path_dialog = None;
                            }
                        },
                    }
                }

                egui::Window::new("New Plugin")
                    .auto_sized()
                    .show(ui.ctx(), |ui| {
                        let mut dirty = false;

                        let o = egui::TextEdit::singleline(&mut new_plugin.name)
                            .hint_text("Plugin name")
                            .clip_text(false)
                            .show(ui);

                        if o.response.changed() {
                            dirty = true;
                        }

                        ui.horizontal(|ui| {
                            let o = egui::TextEdit::singleline(&mut new_plugin.path)
                                .hint_text("Path to plugin")
                                .clip_text(false)
                                .show(ui);

                            if o.response.changed() {
                                dirty = true;
                            }

                            let r = ui.add_enabled(
                                new_plugin.path_dialog.is_none(),
                                egui::Button::new(egui_phosphor::regular::DOTS_THREE).small(),
                            );

                            if r.clicked() {
                                let initial_path = if new_plugin.path.is_empty() {
                                    None
                                } else {
                                    Some(PathBuf::from(&new_plugin.path))
                                };

                                let mut dialog = FileDialog::select_folder(initial_path);
                                dialog.open();
                                new_plugin.path_dialog = Some(dialog);
                            }
                        });

                        if dirty {
                            new_plugin.ready = validate_ident(&new_plugin.name).is_ok();

                            if new_plugin.ready {
                                match absolute(&new_plugin.path) {
                                    Err(_) => new_plugin.ready = false,
                                    Ok(real_path) => match Utf8PathBuf::from_path_buf(real_path) {
                                        Err(_) => {
                                            new_plugin.ready = false;
                                        }
                                        Ok(real_path) => {
                                            new_plugin.real_path = real_path;
                                            new_plugin.real_path.push(&new_plugin.name);
                                            new_plugin.ready = crate::project::is_available(
                                                new_plugin.real_path.as_std_path(),
                                            );
                                        }
                                    },
                                };
                            }
                        }

                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(new_plugin.ready, egui::Button::new("OK"))
                                .clicked()
                            {
                                let name = Ident::from_str(&new_plugin.name).unwrap();
                                match manager.new_plugin(name, &new_plugin.real_path, project) {
                                    Ok(()) => {
                                        close = true;
                                    }
                                    Err(error) => {
                                        errors.push_error(
                                            ui.ctx().viewport_id(),
                                            "New plugins errors",
                                            error,
                                        );
                                    }
                                }
                            }

                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });

                if close {
                    self.dialog = None;
                }
            }
        }

        if sync_project {
            project.sync_in_ui(ui.ctx(), errors);
            manager.refresh_enabled_plugins(project);
        }

        if rebuild_plugins {
            manager.start_new_build(project);
        }
    }
}

/// Adds new plugins library
fn add_plugin_with_path(path: Utf8PathBuf, project: &mut Project) -> Result<bool, Error> {
    let plugin = Plugin::open_local(path)?;
    project.add_plugin(plugin)
}
