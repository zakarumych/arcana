use arcana::{
    Name,
    ecs::world::World,
    hash::HashMap,
    input::{FilterId, Input},
    plugin::{Location, PluginsHub},
};
use egui::{Color32, Ui, WidgetText};
use winit::window::WindowId;

use crate::{
    project::{Project, ProjectData},
    tool::{Tool, ToolContext, ToolTemplate},
};

use super::{ide::Ide, plugins::Plugins};

#[derive(Clone, Debug, Default)]
pub struct Funnel {
    filters: Vec<FilterId>,
}

impl Funnel {
    pub fn new() -> Self {
        Funnel {
            filters: Vec::new(),
        }
    }

    pub fn filter(&self, hub: &mut PluginsHub, world: &mut World, input: &Input) -> bool {
        for filter in self.filters.iter() {
            if let Some(filter) = hub.filters.get_mut(filter) {
                if filter.filter(world, input) {
                    return true;
                }
            }
        }

        false
    }
}

#[derive(Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
struct Filter {
    plugin: Name,
    name: Name,
    id: FilterId,
    enabled: bool,

    #[serde(skip)]
    location: Option<Location>,

    #[serde(skip)]
    active: bool,
}

#[derive(Clone, Debug, Default, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct FilterOrder {
    filters: Vec<Filter>,
}

pub struct FilterManager {
    available: Vec<Filter>,
    order: FilterOrder,
    modification: u64,
}

impl FilterManager {
    pub fn new() -> Self {
        FilterManager {
            available: Vec::new(),
            order: FilterOrder {
                filters: Vec::new(),
            },
            modification: 1,
        }
    }

    pub fn load(&mut self, project: &Project, data: &ProjectData) {
        self.order = data.filters.clone();
    }

    pub fn save(&self, _project: &mut Project, data: &mut ProjectData) {
        data.filters = self.order.clone();
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        let mut all_filters = HashMap::default();

        for (name, plugin) in plugins.iter() {
            for info in plugin.filters() {
                all_filters.insert(info.id, (name, info));
            }
        }

        for filter in self.order.filters.iter_mut() {
            if let Some((_, info)) = all_filters.remove(&filter.id) {
                filter.location = info.location.clone();
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
                location: info.location.clone(),
                active: true,
            })
            .collect::<Vec<_>>();

        self.available = new_filters;
        self.available.sort_by_cached_key(|info| info.name);
    }

    pub fn update_funnel(&self, modification: &mut u64, funnel: &mut Funnel) {
        if self.modification == *modification {
            return;
        }

        debug_assert!(self.modification > *modification);

        funnel.filters.clear();
        for filter in &self.order.filters {
            if filter.enabled && filter.active {
                funnel.filters.push(filter.id);
            }
        }
        *modification = self.modification;
    }
}

pub struct FiltersWidget {}

impl FiltersWidget {
    pub fn new() -> Self {
        FiltersWidget {}
    }

    pub fn show(
        &mut self,
        manager: &mut FilterManager,
        plugins: Option<&Plugins>,
        ide: Option<&dyn Ide>,
        ui: &mut Ui,
    ) {
        let mut add_filter = None;

        ui.menu_button(egui_phosphor::regular::PLUS, |ui| {
            if manager.available.is_empty() {
                ui.weak("No available filters");
            }

            for (idx, filter) in manager.available.iter().enumerate() {
                let r = ui.button(filter.name.as_str());
                if r.clicked() {
                    add_filter = Some(idx);
                    ui.close();
                }
                r.on_hover_text(format!("From {}", filter.plugin.as_str()));
            }
        });

        if let Some(idx) = add_filter {
            let filter = manager.available.remove(idx);
            manager.order.filters.push(filter);
        }

        let mut toggle_filter = None;
        let mut remove_filter = None;
        let r = egui_dnd::dnd(ui, "filter-list").show(
            manager.order.filters.iter(),
            |ui, filter, handle, state| {
                let mut heading = WidgetText::from(filter.name.as_str());
                let mut tooltip = "";

                if let Some(plugins) = plugins {
                    match plugins.has(filter.plugin) {
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
                } else {
                    tooltip = "Plugin information unavailable";
                    heading = heading.color(Color32::DARK_GRAY);
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
            manager.order.filters[idx].enabled = !manager.order.filters[idx].enabled;
        }

        if let Some(idx) = remove_filter {
            let info = manager.order.filters.remove(idx);
            manager.available.push(info);
        }

        if let Some(update) = r.update {
            egui_dnd::utils::shift_vec(update.from, update.to, &mut manager.order.filters);
        }
    }
}

pub struct FiltersTemplate;

impl ToolTemplate for FiltersTemplate {
    fn key(&self) -> &str {
        "filters"
    }

    fn title(&self) -> &str {
        "Filters"
    }

    fn create(
        &self,
        _state: Option<serde_json::Value>,
    ) -> Result<Box<dyn Tool>, serde_json::Error> {
        Ok(Box::new(FiltersWidget::new()))
    }
}

impl Tool for FiltersWidget {
    fn title(&self) -> &str {
        "Filters"
    }

    fn ui(&mut self, ui: &mut egui::Ui, _window: WindowId, context: &mut ToolContext<'_>) {
        self.show(context.filters, context.plugins.linked(), context.ide, ui);
    }
}
