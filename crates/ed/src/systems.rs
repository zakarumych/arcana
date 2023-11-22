use arcana::{edict::world::WorldLocal, project::Project};
use egui::{Color32, Ui, WidgetText};

use crate::move_element;

use super::{plugins::Plugins, Tab};

pub struct Systems;

impl Systems {
    pub fn tab() -> Tab {
        Tab::Systems
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut project = world.expect_resource_mut::<Project>();
        let plugins = world.expect_resource::<Plugins>();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let mut toggle_system = None;
                let mut remove_system = None;

                let r = egui_dnd::dnd(ui, "var-system-list").show(
                    project.manifest().var_systems.iter(),
                    |ui, system, handle, state| {
                        let mut heading = WidgetText::from(system.name.as_str());
                        let mut tooltip = "";
                        let mut removeable = false;

                        match project.manifest().get_plugin(&system.plugin) {
                            None => {
                                tooltip = "Plugin not found";
                                heading = heading.color(Color32::DARK_RED);
                            }
                            Some(plugin) => match plugins.get_plugin(&system.plugin) {
                                Some(a) => {
                                    if a.systems().iter().any(|s| **s == *system.name) {
                                        if plugins.is_active(&plugin.name) {
                                            heading = heading.color(Color32::GREEN);
                                        } else {
                                            heading = heading.color(Color32::YELLOW);
                                            tooltip = "Plugin is not active";
                                        }
                                    } else {
                                        heading = heading.color(Color32::DARK_RED);
                                        tooltip = "Plugin doesn't export this system";
                                        removeable = true;
                                    }
                                }
                                None => {
                                    heading = heading.color(Color32::DARK_GRAY);
                                    tooltip = "Plugin not found";
                                    removeable = true;
                                }
                            },
                        }

                        ui.horizontal(|ui| {
                            handle.ui(ui, |ui| {
                                ui.label(egui_phosphor::regular::DOTS_SIX_VERTICAL);
                            });
                            let mut enabled = system.enabled;
                            let r = ui.checkbox(&mut enabled, heading);
                            if system.enabled != enabled {
                                toggle_system = Some(state.index);
                            }

                            if !tooltip.is_empty() {
                                r.on_hover_text(tooltip);
                            }

                            if removeable {
                                if ui.button(egui_phosphor::regular::TRASH).clicked() {
                                    remove_system = Some(state.index);
                                }
                            }
                        });
                    },
                );

                let manifest = project.manifest_mut();
                let systems = &mut manifest.var_systems;

                let mut sync = false;
                if let Some(idx) = toggle_system {
                    let system = &mut systems[idx];

                    if !system.enabled {
                        let fix_system = manifest.fix_systems.iter_mut().find(|s| **s == *system);

                        if let Some(fix_system) = fix_system {
                            fix_system.enabled = false;
                        }

                        system.enabled = true;
                    } else {
                        system.enabled = false;
                    }

                    sync = true;
                }
                if let Some(idx) = remove_system {
                    systems.remove(idx);
                    sync = true;
                }
                if let Some(update) = r.update {
                    move_element(systems, update.from, update.to);
                    sync = true;
                }

                if sync {
                    try_log_err!(project.sync());
                }
            });

            ui.vertical(|ui| {
                let mut toggle_system = None;
                let mut remove_system = None;

                let r = egui_dnd::dnd(ui, "var-system-list").show(
                    project.manifest().fix_systems.iter(),
                    |ui, system, handle, state| {
                        let mut heading = WidgetText::from(system.name.as_str());
                        let mut tooltip = "";
                        let mut removeable = false;

                        match project.manifest().get_plugin(&system.plugin) {
                            None => {
                                tooltip = "Plugin not found";
                                heading = heading.color(Color32::DARK_RED);
                            }
                            Some(plugin) => match plugins.get_plugin(&system.plugin) {
                                Some(a) => {
                                    if a.systems().iter().any(|s| **s == *system.name) {
                                        if plugins.is_active(&plugin.name) {
                                            heading = heading.color(Color32::GREEN);
                                        } else {
                                            heading = heading.color(Color32::YELLOW);
                                            tooltip = "Plugin is not active";
                                        }
                                    } else {
                                        heading = heading.color(Color32::DARK_RED);
                                        tooltip = "Plugin doesn't export this system";
                                        removeable = true;
                                    }
                                }
                                None => {
                                    heading = heading.color(Color32::DARK_GRAY);
                                    tooltip = "Plugin not found";
                                    removeable = true;
                                }
                            },
                        }

                        ui.horizontal(|ui| {
                            handle.ui(ui, |ui| {
                                ui.label(egui_phosphor::regular::DOTS_SIX_VERTICAL);
                            });
                            let mut enabled = system.enabled;
                            let r = ui.checkbox(&mut enabled, heading);
                            if system.enabled != enabled {
                                toggle_system = Some(state.index);
                            }

                            if !tooltip.is_empty() {
                                r.on_hover_text(tooltip);
                            }

                            if removeable {
                                if ui.button(egui_phosphor::regular::TRASH).clicked() {
                                    remove_system = Some(state.index);
                                }
                            }
                        });
                    },
                );

                let manifest = project.manifest_mut();
                let systems = &mut manifest.fix_systems;

                let mut sync = false;
                if let Some(idx) = toggle_system {
                    let system = &mut systems[idx];

                    if !system.enabled {
                        let var_system = manifest.var_systems.iter_mut().find(|s| **s == *system);

                        if let Some(var_system) = var_system {
                            var_system.enabled = false;
                        }

                        system.enabled = true;
                    } else {
                        system.enabled = false;
                    }

                    sync = true;
                }
                if let Some(idx) = remove_system {
                    systems.remove(idx);
                    sync = true;
                }
                if let Some(update) = r.update {
                    move_element(systems, update.from, update.to);
                    sync = true;
                }

                if sync {
                    try_log_err!(project.sync());
                }
            });
        });
    }
}
