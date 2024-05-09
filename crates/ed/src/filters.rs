use arcana::{
    edict::world::WorldLocal,
    events::{Event, FilterId},
    plugin::PluginsHub,
    project::{IdentBuf, Project},
    Blink, World,
};
use egui::{Color32, Ui, WidgetText};
use hashbrown::{HashMap, HashSet};

use crate::{container::Container, data::ProjectData, move_element};

use super::plugins::Plugins;

#[derive(Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
struct FilterInfo {
    plugin: IdentBuf,
    name: IdentBuf,
    id: FilterId,
    enabled: bool,

    #[serde(skip)]
    active: bool,
}

pub struct Filters {
    available: Vec<FilterInfo>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Funnel {
    filters: Vec<FilterInfo>,
}

impl Funnel {
    pub fn filter(
        &self,
        hub: &mut PluginsHub,
        blink: &Blink,
        world: &mut World,
        event: &Event,
    ) -> bool {
        for filter in self.filters.iter() {
            if filter.enabled {
                if let Some(filter) = hub.filters.get_mut(&filter.id) {
                    if filter.filter(blink, world, event) {
                        return true;
                    }
                }
            }
        }

        false
    }
}

impl Filters {
    pub fn new() -> Self {
        Filters {
            available: Vec::new(),
        }
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut filters = world.expect_resource_mut::<Filters>();
        let project = world.expect_resource_mut::<Project>();
        let mut data = world.expect_resource_mut::<ProjectData>();
        let plugins = world.expect_resource::<Plugins>();

        let mut sync = false;
        let mut add_filter = None;

        ui.menu_button(egui_phosphor::regular::PLUS, |ui| {
            if filters.available.is_empty() {
                ui.weak("No available systems");
            }

            for (idx, filter) in filters.available.iter().enumerate() {
                let r = ui.button(filter.name.as_str());
                if r.clicked() {
                    add_filter = Some(idx);
                    ui.close_menu();
                }
                r.on_hover_text(format!("From {}", filter.plugin.as_str()));
            }
        });

        if let Some(idx) = add_filter {
            let filter = filters.available.remove(idx);
            data.funnel.filters.push(filter);
            sync = true;
        }

        let mut toggle_filter = None;
        let mut remove_filter = None;
        let r = egui_dnd::dnd(ui, "filter-list").show(
            data.funnel.filters.iter(),
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

        if let Some(idx) = toggle_filter {
            data.funnel.filters[idx].enabled = !data.funnel.filters[idx].enabled;
            sync = true;
        }

        if let Some(idx) = remove_filter {
            data.funnel.filters.remove(idx);
            sync = true;
        }

        if let Some(update) = r.update {
            move_element(&mut data.funnel.filters, update.from, update.to);
            sync = true;
        }

        if sync {
            try_log_err!(data.sync(&project));
        }
    }

    pub fn update_plugins(&mut self, data: &mut ProjectData, container: &Container) {
        let mut all_filters = HashMap::new();

        for (name, plugin) in container.plugins() {
            for filter in plugin.filters() {
                all_filters.insert(filter.id, (name, filter.name));
            }
        }

        for info in data.funnel.filters.iter_mut() {
            info.active = all_filters.remove(&info.id).is_some();
        }

        let new_filters = all_filters
            .into_iter()
            .map(|(id, (plugin, name))| FilterInfo {
                name: name.into_owned(),
                plugin: plugin.to_owned(),
                id,
                enabled: false,
                active: true,
            })
            .collect::<Vec<_>>();

        self.available = new_filters;
        self.available.sort_by_cached_key(|info| info.name.clone());
    }
}
