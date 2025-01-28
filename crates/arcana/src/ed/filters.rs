use arcana::{
    input::{FilterId, Input},
    plugin::{Location, PluginsHub},
    project::Project,
    Blink, Ident, Name, World,
};
use egui::{Color32, Ui, WidgetText};
use hashbrown::HashMap;

use super::{container::Container, data::ProjectData, ide::Ide};

#[derive(Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
struct Filter {
    plugin: Ident,
    name: Name,
    id: FilterId,
    enabled: bool,

    #[serde(skip)]
    location: Option<Location>,

    #[serde(skip)]
    active: bool,
}

pub struct Filters {
    available: Vec<Filter>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Funnel {
    filters: Vec<Filter>,
}

impl Funnel {
    pub fn filter(
        &self,
        hub: &mut PluginsHub,
        blink: &Blink,
        world: &mut World,
        input: &Input,
    ) -> bool {
        for filter in self.filters.iter() {
            if filter.enabled {
                if let Some(filter) = hub.filters.get_mut(&filter.id) {
                    if filter.filter(blink, world, input) {
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

    pub fn show(
        &mut self,
        project: &Project,
        data: &mut ProjectData,
        ide: Option<&dyn Ide>,
        ui: &mut Ui,
    ) {
        let mut sync = false;
        let mut add_filter = None;

        ui.menu_button(egui_phosphor::regular::PLUS, |ui| {
            if self.available.is_empty() {
                ui.weak("No available filters");
            }

            for (idx, filter) in self.available.iter().enumerate() {
                let r = ui.button(filter.name.as_str());
                if r.clicked() {
                    add_filter = Some(idx);
                    ui.close_menu();
                }
                r.on_hover_text(format!("From {}", filter.plugin.as_str()));
            }
        });

        if let Some(idx) = add_filter {
            let filter = self.available.remove(idx);
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

                    let r = ui.add_enabled(
                        filter.location.is_some() && ide.is_some(),
                        egui::Button::new(egui_phosphor::regular::CODE).small(),
                    );

                    let r = r.on_hover_ui(|ui| {
                        ui.label("Open system in IDE");

                        if ide.is_none() {
                            ui.weak("No IDE configured");
                        }

                        if filter.location.is_none() {
                            ui.weak("No location information");
                        }
                    });

                    let r = r.on_disabled_hover_ui(|ui| {
                        ui.label("Open system in IDE");

                        if ide.is_none() {
                            ui.weak("No IDE configured");
                        }

                        if filter.location.is_none() {
                            ui.weak("No location information");
                        }
                    });

                    if r.clicked() {
                        let loc = filter.location.as_ref().unwrap();
                        ide.unwrap().open(loc.file.as_ref(), Some(loc.line));
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
            self.available.push(info);
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
            for info in plugin.filters() {
                all_filters.insert(info.id, (name, info));
            }
        }

        for filter in data.funnel.filters.iter_mut() {
            if let Some((_, info)) = all_filters.remove(&filter.id) {
                filter.location = info.location;
                filter.active = true;
            } else {
                filter.active = false;
            }
        }

        let new_filters = all_filters
            .into_iter()
            .map(|(id, (plugin, info))| Filter {
                name: info.name,
                plugin: plugin.to_owned(),
                id,
                enabled: false,
                location: info.location,
                active: true,
            })
            .collect::<Vec<_>>();

        self.available = new_filters;
        self.available.sort_by_cached_key(|info| info.name.clone());
    }
}
