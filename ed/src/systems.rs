use std::collections::VecDeque;

use arcana::{
    Name,
    ecs::{SystemId, action::ActionBufferSliceExt, world::World},
    hash::{HashMap, HashSet},
    plugin::{Location, PluginsHub},
};
use egui::{Color32, Ui};
use egui_snarl::{
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
    ui::{AnyPins, PinInfo, PinShape, SnarlStyle, SnarlViewer},
};
use winit::window::WindowId;

use crate::{
    project::{Project, ProjectData},
    tool::{Tool, ToolContext, ToolTemplate},
};

use super::{ide::Ide, plugins::Plugins, toggle_ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Category {
    Fix,
    Var,
}

impl Category {
    fn pin_shape(&self) -> PinShape {
        match self {
            Category::Fix => PinShape::Square,
            Category::Var => PinShape::Triangle,
        }
    }
}

#[derive(Clone)]
pub struct Schedule {
    fix_schedule: Vec<SystemId>,
    var_schedule: Vec<SystemId>,
}

impl Schedule {
    pub fn new() -> Self {
        Schedule {
            fix_schedule: Vec::new(),
            var_schedule: Vec::new(),
        }
    }

    /// Run systems in dependency order.
    /// Reschedules systems if graph is modified.
    pub fn run(&self, category: Category, world: &mut World, hub: &mut PluginsHub) {
        let schedule = match category {
            Category::Fix => &*self.fix_schedule,
            Category::Var => &*self.var_schedule,
        };

        let mut buffers = Vec::new();

        for id in schedule {
            let system = hub.systems.get_mut(id).unwrap();
            system.run(world, &mut buffers);
        }

        buffers.execute_all(world);
    }
}

fn order_systems(snarl: &Snarl<SystemNode>, category: Category) -> Vec<SystemId> {
    let mut order = Vec::new();

    let mut queue = VecDeque::new();
    let mut scheduled = HashSet::default();

    for (idx, node) in snarl.node_ids() {
        if node.category != category {
            continue;
        }
        queue.push_back(idx);
    }

    'outer: while let Some(idx) = queue.pop_front() {
        let in_pin = snarl.in_pin(InPinId {
            node: idx,
            input: 0,
        });

        for remote in in_pin.remotes {
            if !scheduled.contains(&remote.node) {
                queue.push_back(idx);
                continue 'outer;
            }
        }

        let node = &snarl[idx];

        if node.active && node.enabled {
            order.push(node.system);
        }
        scheduled.insert(idx);
    }

    order
}

pub struct SystemManager {
    available: Vec<SystemNode>,

    /// Systems graph.
    systems: SystemGraph,

    modification: u64,
}

impl SystemManager {
    pub fn new() -> Self {
        SystemManager {
            available: Vec::new(),
            systems: SystemGraph::new(),
            modification: 1,
        }
    }

    pub fn load(&mut self, _project: &Project, data: &ProjectData) {
        self.systems = data.systems.clone();
        self.modification += 1;
    }

    pub fn save(&self, _project: &mut Project, data: &mut ProjectData) {
        data.systems = self.systems.clone();
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        let mut all_systems = HashMap::default();

        for (name, plugin) in plugins.iter() {
            for info in plugin.systems() {
                all_systems.insert(info.id, (name, info));
            }
        }

        for node in self.systems.nodes_mut() {
            if let Some((_, info)) = all_systems.remove(&node.system) {
                node.location = info.location.clone();
                node.active = true;
            } else {
                node.active = false;
            }
        }

        let new_systems = all_systems
            .into_iter()
            .map(|(id, (plugin, info))| SystemNode {
                system: id,
                name: info.name,
                plugin,
                active: true,
                category: Category::Fix,
                location: info.location.clone(),
                enabled: false,
            })
            .collect::<Vec<_>>();

        self.available = new_systems;
        self.available.sort_by_cached_key(|node| node.name);

        self.modification += 1;
    }

    pub fn update_schedule(&self, modification: &mut u64, schedule: &mut Schedule) {
        if self.modification == *modification {
            return;
        }

        debug_assert!(self.modification > *modification);

        schedule.fix_schedule = order_systems(&self.systems, Category::Fix);
        schedule.var_schedule = order_systems(&self.systems, Category::Var);
        *modification = self.modification;
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SystemsWidget {}

impl SystemsWidget {
    pub fn new() -> Self {
        SystemsWidget {}
    }

    pub fn show(&mut self, manager: &mut SystemManager, ide: Option<&dyn Ide>, ui: &mut Ui) {
        const STYLE: SnarlStyle = SnarlStyle::new();

        let mut viewer = SystemViewer {
            modified: false,
            available: &mut manager.available,
            ide,
        };

        manager.systems.show(&mut viewer, &STYLE, "systems", ui);

        if viewer.modified {
            manager.modification += 1;
        }
    }
}

pub type SystemGraph = Snarl<SystemNode>;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SystemNode {
    system: SystemId,
    name: Name,
    plugin: Name,
    enabled: bool,
    category: Category,

    #[serde(skip)]
    location: Option<Location>,

    #[serde(skip)]
    active: bool,
}

struct SystemViewer<'a> {
    modified: bool,
    available: &'a mut Vec<SystemNode>,
    ide: Option<&'a dyn Ide>,
}

impl SnarlViewer<SystemNode> for SystemViewer<'_> {
    fn title<'a>(&'a mut self, node: &'a SystemNode) -> String {
        format!("{}@{}", &*node.name, &*node.plugin)
    }

    fn show_header(
        &mut self,
        id: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<SystemNode>,
    ) {
        let mut remove = false;
        let mut toggle = false;
        let node = &mut snarl[id];

        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let cb = egui::Checkbox::new(&mut node.enabled, node.name.as_str());
                let r = ui.add_enabled(node.active, cb);

                self.modified |= r.changed();

                r.on_hover_text("Enable/disable system");

                ui.weak(egui_phosphor::regular::AT);
                ui.label(node.plugin.as_str());
                let r = ui.small_button(egui_phosphor::regular::TRASH_SIMPLE);

                remove = r.clicked();

                r.on_hover_ui(|ui| {
                    ui.label("Remove system from graph");
                });

                let r = ui.add_enabled(
                    node.location.is_some() && self.ide.is_some(),
                    egui::Button::new(egui_phosphor::regular::CODE).small(),
                );

                let r = r.on_hover_ui(|ui| {
                    ui.label("Open system in IDE");

                    if self.ide.is_none() {
                        ui.weak("No IDE configured");
                    }

                    if node.location.is_none() {
                        ui.weak("No location information");
                    }
                });

                let r = r.on_disabled_hover_ui(|ui| {
                    ui.label("Open system in IDE");

                    if self.ide.is_none() {
                        ui.weak("No IDE configured");
                    }

                    if node.location.is_none() {
                        ui.weak("No location information");
                    }
                });

                if r.clicked() {
                    let loc = node.location.as_ref().unwrap();
                    self.ide.unwrap().open(loc.file.as_ref(), Some(loc.line));
                }
            });

            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let mut is_fix = node.category == Category::Fix;
                ui.label("Variable rate");
                let r = toggle_ui(ui, &mut is_fix);
                r.on_hover_text("Execute system with variable or fixed rate");
                ui.label("Fixed rate");
                toggle = is_fix != (node.category == Category::Fix);
            });
        });

        if toggle {
            node.category = match node.category {
                Category::Fix => Category::Var,
                Category::Var => Category::Fix,
            };

            snarl.drop_inputs(InPinId { node: id, input: 0 });
            snarl.drop_outputs(OutPinId {
                node: id,
                output: 0,
            });
            self.modified = true;
        }

        if remove {
            let node = snarl.remove_node(id);
            self.available.push(node);
            self.available.sort_by_cached_key(|node| node.name);
            self.modified = true;
        }
    }

    fn inputs(&mut self, _node: &SystemNode) -> usize {
        1
    }

    fn outputs(&mut self, _node: &SystemNode) -> usize {
        1
    }

    #[allow(refining_impl_trait)]
    fn show_input(&mut self, pin: &InPin, ui: &mut Ui, snarl: &mut Snarl<SystemNode>) -> PinInfo {
        assert_eq!(pin.id.input, 0);

        let pin_fill = Color32::LIGHT_GRAY;
        let pin_stroke = ui.visuals().widgets.inactive.fg_stroke;

        let pin_info = PinInfo::default()
            .with_fill(pin_fill)
            .with_stroke(pin_stroke);

        let node = &snarl[pin.id.node];

        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        pin_info.with_shape(node.category.pin_shape())
    }

    #[allow(refining_impl_trait)]
    fn show_output(&mut self, pin: &OutPin, ui: &mut Ui, snarl: &mut Snarl<SystemNode>) -> PinInfo {
        assert_eq!(pin.id.output, 0);

        let pin_fill = Color32::LIGHT_GRAY;
        let pin_stroke = ui.visuals().widgets.noninteractive.fg_stroke;

        let pin_info = PinInfo::default()
            .with_fill(pin_fill)
            .with_stroke(pin_stroke);

        let node = &mut snarl[pin.id.node];

        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        pin_info.with_shape(node.category.pin_shape())
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<SystemNode>) {
        if from.id.node == to.id.node {
            return;
        }

        let from_cat = snarl[from.id.node].category;
        let to_cat = snarl[to.id.node].category;
        if from_cat != to_cat {
            return;
        }

        snarl.connect(from.id, to.id);
        self.modified = true;
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<SystemNode>) {
        snarl.disconnect(from.id, to.id);
        self.modified = true;
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<SystemNode>) {
        if pin.remotes.is_empty() {
            return;
        }
        snarl.drop_outputs(pin.id);
        self.modified = true;
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<SystemNode>) {
        if pin.remotes.is_empty() {
            return;
        }
        snarl.drop_inputs(pin.id);
        self.modified = true;
    }

    /// Checks if the snarl has something to show in context menu if wire drag is stopped at `pos`.
    #[inline(always)]
    fn has_dropped_wire_menu(&mut self, _: AnyPins, _: &mut Snarl<SystemNode>) -> bool {
        true
    }

    /// Show context menu for the snarl. This menu is opened when releasing a pin to empty
    /// space. It can be used to implement menu for adding new node, and directly
    /// connecting it to the released wire.
    fn show_dropped_wire_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        src_pins: AnyPins,
        snarl: &mut Snarl<SystemNode>,
    ) {
        ui.label("Add system");
        ui.separator();

        if self.available.is_empty() {
            ui.weak("No available systems");
        }

        for idx in 0..self.available.len() {
            let s = &self.available[idx];
            if ui.button(s.name.as_str()).clicked() {
                ui.close();
                let s = self.available.remove(idx);
                let new_node = snarl.insert_node(pos, s);

                match src_pins {
                    AnyPins::In(pins) => {
                        for &pin in pins {
                            snarl.connect(
                                OutPinId {
                                    node: new_node,
                                    output: 0,
                                },
                                pin,
                            );
                        }
                    }
                    AnyPins::Out(pins) => {
                        for &pin in pins {
                            snarl.connect(
                                pin,
                                InPinId {
                                    node: new_node,
                                    input: 0,
                                },
                            );
                        }
                    }
                }

                return;
            }
        }
    }

    #[inline(always)]
    fn has_graph_menu(&mut self, _: egui::Pos2, _: &mut Snarl<SystemNode>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut Ui, snarl: &mut Snarl<SystemNode>) {
        ui.label("Add system");
        ui.separator();

        if self.available.is_empty() {
            ui.weak("No available systems");
        }

        for idx in 0..self.available.len() {
            let s = &self.available[idx];
            if ui.button(s.name.as_str()).clicked() {
                ui.close();
                let s = self.available.remove(idx);
                snarl.insert_node(pos, s);
                return;
            }
        }
    }
}

pub struct SystemsTemplate;

impl ToolTemplate for SystemsTemplate {
    fn key(&self) -> &str {
        "systems"
    }

    fn title(&self) -> &str {
        "Systems"
    }

    fn create(
        &self,
        _state: Option<serde_json::Value>,
    ) -> Result<Box<dyn Tool>, serde_json::Error> {
        Ok(Box::new(SystemsWidget::new()))
    }
}

impl Tool for SystemsWidget {
    fn title(&self) -> &str {
        "Systems"
    }

    fn ui(&mut self, ui: &mut egui::Ui, _window: WindowId, context: &mut ToolContext<'_>) {
        self.show(context.systems, context.ide, ui);
    }

    fn save(&self) -> Option<serde_json::Value> {
        None
    }
}
