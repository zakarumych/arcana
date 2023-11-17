use arcana::{edict::world::WorldLocal, project::Project, World};
use egui::{Color32, Ui, WidgetText};

use crate::move_element;

use super::{plugins::Plugins, Tab};

pub struct Systems;

impl Systems {
    pub fn new() -> Self {
        Systems
    }

    pub fn tab() -> Tab {
        Tab::Systems
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut project = world.expect_resource_mut::<Project>();
        let mut plugins = world.expect_resource_mut::<Plugins>();

        let mut toggle_system = None;
        let mut remove_system = None;
        let r = egui_dnd::dnd(ui, "system-list").show(
            project.manifest().systems.iter(),
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

        let systems = &mut project.manifest_mut().systems;

        let mut sync = false;
        if let Some(idx) = toggle_system {
            systems[idx].enabled = !systems[idx].enabled;
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
    }
}
