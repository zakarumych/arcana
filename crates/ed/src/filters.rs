use arcana::{edict::world::WorldLocal, plugin::FilterId, project::Project};
use arcana_project::IdentBuf;
use egui::{Color32, Ui, WidgetText};

use crate::{data::ProjectData, move_element};

use super::plugins::Plugins;

#[derive(Hash)]
struct FilterInfo {
    plugin: IdentBuf,
    name: IdentBuf,
    id: FilterId,
    enabled: bool,
}

pub struct Filters {
    infos: Vec<FilterInfo>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Funnel {
    filters: Vec<FilterId>,
}

impl Filters {
    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut filters = world.expect_resource_mut::<Filters>();
        let project = world.expect_resource_mut::<Project>();
        let mut data = world.expect_resource_mut::<ProjectData>();
        let plugins = world.expect_resource::<Plugins>();

        let mut toggle_filter = None;
        let mut remove_filter = None;
        let r = egui_dnd::dnd(ui, "filter-list").show(
            filters.infos.iter(),
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
                            if a.filters().iter().any(|f| *f.name == *filter.name) {
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

        let mut sync = false;

        if let Some(idx) = toggle_filter {
            filters.infos[idx].enabled = !filters.infos[idx].enabled;
            sync = true;
        }

        if let Some(idx) = remove_filter {
            filters.infos.remove(idx);
            sync = true;
        }

        if let Some(update) = r.update {
            move_element(&mut filters.infos, update.from, update.to);
            sync = true;
        }

        if sync {
            data.funnel.filters = filters
                .infos
                .iter()
                .filter(|f| f.enabled && plugins.is_active(&f.plugin))
                .map(|f| f.id)
                .collect();

            try_log_err!(data.sync(&project));
        }
    }
}
