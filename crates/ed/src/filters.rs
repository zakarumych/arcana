use arcana::{
    edict::world::WorldLocal,
    events::{Event, FilterId},
    plugin::PluginsHub,
    project::Project,
    Blink, Ident, Name, World,
};
use egui::{Color32, Ui, WidgetText};
use hashbrown::HashMap;

use crate::{container::Container, data::ProjectData};

#[derive(Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
struct FilterInfo {
    plugin: Ident,
    name: Name,
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

                match project.manifest().has_plugin(filter.plugin) {
                    false => {
                        tooltip = "Plugin not found";
                        heading = heading.color(Color32::DARK_RED);
                    }
                    true => {
                        if filter.active {
                            heading = heading.color(Color32::GREEN);
                        } else {
                            heading = heading.color(Color32::YELLOW);
                            tooltip = "Plugin is not active";
                        }
                    }
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

                    if ui.button(egui_phosphor::regular::TRASH).clicked() {
                        remove_filter = Some(state.index);
                    }
                });
            },
        );

        if let Some(idx) = toggle_filter {
            data.funnel.filters[idx].enabled = !data.funnel.filters[idx].enabled;
            sync = true;
        }

        if let Some(idx) = remove_filter {
            let info = data.funnel.filters.remove(idx);
            filters.available.push(info);
            sync = true;
        }

        if let Some(update) = r.update {
            egui_dnd::utils::shift_vec(update.from, update.to, &mut data.funnel.filters);
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
                name,
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
