use std::collections::VecDeque;

use arcana::{
    edict::world::WorldLocal,
    plugin::{PluginsHub, SystemId},
    project::{IdentBuf, Project},
    ActionBufferSliceExt, World,
};
use egui::{Color32, Ui};
use egui_snarl::{
    ui::{AnyPins, PinInfo, PinShape, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::{HashMap, HashSet};

use crate::{container::Container, data::ProjectData, toggle_ui};

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
    let mut scheduled = HashSet::new();

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

pub struct Systems {
    schedule: Schedule,
    available: Vec<SystemNode>,
}

impl Systems {
    pub fn new() -> Self {
        Systems {
            schedule: Schedule {
                fix_schedule: Vec::new(),
                var_schedule: Vec::new(),
            },
            available: Vec::new(),
        }
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut me = world.expect_resource_mut::<Self>();
        let mut data = world.expect_resource_mut::<ProjectData>();
        let project = world.expect_resource::<Project>();

        const STYLE: SnarlStyle = SnarlStyle::new();

        let mut viewer = SystemViewer {
            modified: false,
            available: &mut me.available,
        };

        data.systems.snarl.show(&mut viewer, &STYLE, "systems", ui);

        if viewer.modified {
            me.schedule.fix_schedule = order_systems(&data.systems.snarl, Category::Fix);
            me.schedule.var_schedule = order_systems(&data.systems.snarl, Category::Var);

            try_log_err!(data.sync(&project));
        }
    }

    pub fn update_plugins(&mut self, data: &mut ProjectData, container: &Container) {
        let mut all_systems = HashMap::new();

        for (name, plugin) in container.iter() {
            for system in plugin.systems() {
                all_systems.insert(system.id, (name, system.name));
            }
        }

        for node in data.systems.snarl.nodes_mut() {
            node.active = all_systems.remove(&node.system).is_some();
        }

        let new_systems = all_systems
            .into_iter()
            .map(|(id, (plugin, system))| SystemNode {
                system: id,
                name: system.into_owned(),
                plugin: plugin.to_owned(),
                active: true,
                enabled: false,
                category: Category::Fix,
            })
            .collect::<Vec<_>>();

        self.available = new_systems;
        self.available.sort_by_cached_key(|node| node.name.clone());
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct SystemGraph {
    snarl: Snarl<SystemNode>,
}

impl SystemGraph {
    pub fn new() -> Self {
        SystemGraph {
            snarl: Snarl::new(),
        }
    }
}

impl Default for SystemGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SystemNode {
    name: IdentBuf,
    plugin: IdentBuf,
    system: SystemId,
    enabled: bool,
    category: Category,

    #[serde(skip)]
    active: bool,
}

struct SystemViewer<'a> {
    modified: bool,
    available: &'a mut Vec<SystemNode>,
}

impl SnarlViewer<SystemNode> for SystemViewer<'_> {
    fn title<'a>(&'a mut self, node: &'a SystemNode) -> String {
        format!("{}@{}", &*node.name, &*node.plugin)
    }

    fn show_header(
        &mut self,
        id: NodeId,
        _inputs: &[InPin],
        _utputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
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
                let r = ui.add_enabled(
                    !node.active,
                    egui::Button::new(egui_phosphor::regular::TRASH_SIMPLE).small(),
                );

                remove = r.clicked();

                r.on_hover_ui(|ui| {
                    ui.label("Remove system from graph");
                    ui.label("The system is not found in active plugins");
                    ui.label(
                        "If plugins is reactivated and system is found, it will be added back",
                    );
                });
            });

            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let mut is_fix = node.category == Category::Fix;
                ui.label("Variable rate");
                let r = toggle_ui(ui, &mut is_fix);
                ui.label("Fixed rate");
                toggle = is_fix != (node.category == Category::Fix);

                self.modified |= r.changed();
            });
        });

        if remove {
            snarl.remove_node(id);
            self.modified = true;
        } else if toggle {
            node.category = match node.category {
                Category::Fix => Category::Var,
                Category::Var => Category::Fix,
            };

            snarl.drop_inputs(InPinId { node: id, input: 0 });
            snarl.drop_outputs(OutPinId {
                node: id,
                output: 0,
            });
        }
    }

    fn inputs(&mut self, _node: &SystemNode) -> usize {
        1
    }

    fn outputs(&mut self, _node: &SystemNode) -> usize {
        1
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) -> PinInfo {
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

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) -> PinInfo {
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
    #[inline]
    fn has_dropped_wire_menu(&mut self, _: AnyPins, _: &mut Snarl<SystemNode>) -> bool {
        true
    }

    /// Show context menu for the snarl. This menu is opened when releasing a pin to empty
    /// space. It can be used to implement menu for adding new node, and directly
    /// connecting it to the released wire.
    #[inline]
    fn show_dropped_wire_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
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
                ui.close_menu();
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

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<SystemNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) {
        ui.label("Add system");
        ui.separator();

        for idx in 0..self.available.len() {
            let s = &self.available[idx];
            if ui.button(s.name.as_str()).clicked() {
                ui.close_menu();
                let s = self.available.remove(idx);
                snarl.insert_node(pos, s);
                return;
            }
        }
    }

    fn has_node_menu(&mut self, _node: &SystemNode) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        id: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) {
        if ui.button("Remove").clicked() {
            let node = snarl.remove_node(id);
            self.available.push(node);
            self.available.sort_by_cached_key(|node| node.name.clone());

            ui.close_menu();
        }
    }
}
