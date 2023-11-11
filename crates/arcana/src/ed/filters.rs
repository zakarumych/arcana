use arcana_project::{IdentBuf, Item, Project};
use edict::World;
use egui::{Color32, Ui, WidgetText};
use hashbrown::HashMap;

use crate::{move_element, try_log_err};

use super::{plugins::Plugins, Tab};

pub struct Filters;

impl Filters {
    pub fn new() -> Self {
        Filters
    }

    pub fn tab() -> Tab {
        Tab::Filters
    }

    pub fn show(world: &mut World, ui: &mut Ui) {
        let world = world.local();
        let mut project = world.expect_resource_mut::<Project>();
        let mut plugins = world.expect_resource_mut::<Plugins>();

        let mut toggle_filter = None;
        let mut remove_filter = None;
        let r = egui_dnd::dnd(ui, "filter-list").show(
            project.manifest().filters.iter(),
            |ui, filter, handle, state| {
                let mut heading = WidgetText::from(filter.name.as_str());
                let mut tooltip = "";
                let mut removeable = false;

                match project.manifest().get_plugin(&filter.plugin) {
                    None => {
                        tooltip = "Plugin not found";
                        heading = heading.color(Color32::DARK_RED);
                    }
                    Some(plugin) => match plugins.get_plugin(&filter.plugin) {
                        Some(a) => {
                            if a.filters().iter().any(|f| **f == *filter.name) {
                                if plugins.is_active(&plugin.name) {
                                    heading = heading.color(Color32::GREEN);
                                } else {
                                    heading = heading.color(Color32::YELLOW);
                                    tooltip = "Plugin is not active";
                                }
                            } else {
                                heading = heading.color(Color32::DARK_RED);
                                tooltip = "Plugin doesn't export this filter";
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
                    let mut enabled = filter.enabled;
                    let r = ui.checkbox(&mut enabled, heading);
                    if filter.enabled != enabled {
                        toggle_filter = Some(state.index);
                    }
                    if !tooltip.is_empty() {
                        r.on_hover_text(tooltip);
                    }

                    if removeable {
                        if ui.button(egui_phosphor::regular::TRASH).clicked() {
                            remove_filter = Some(state.index);
                        }
                    }
                });
            },
        );

        let filters = &mut project.manifest_mut().filters;

        let mut sync = false;
        if let Some(idx) = toggle_filter {
            filters[idx].enabled = !filters[idx].enabled;
            sync = true;
        }
        if let Some(idx) = remove_filter {
            filters.remove(idx);
            sync = true;
        }
        if let Some(update) = r.update {
            move_element(filters, update.from, update.to);
            sync = true;
        }
        if sync {
            try_log_err!(project.sync());
        }
    }
}
