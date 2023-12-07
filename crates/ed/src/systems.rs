use arcana::{edict::world::WorldLocal, plugin::ArcanaPlugin, project::Project, World};
use arcana_project::{Ident, IdentBuf};
use egui::{Color32, Ui};
use egui_snarl::{
    ui::{InPin, OutPin, PinInfo, PinShape, SnarlStyle, SnarlViewer},
    InPinId, OutPinId, Snarl,
};
use hashbrown::HashSet;

use crate::{data::ProjectData, sync_project, toggle_ui};

use super::Tab;

/// Walk over snarl and run fixed systems in order.
fn run_fix_systems(world: &mut World, snarl: &Snarl<SystemNode>) {
    for (idx, node) in snarl.node_indices() {}
}

/// Walk over snarl and run variable systems in order.
fn run_var_systems(world: &mut World, snarl: &Snarl<SystemNode>) {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
enum Category {
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

    fn title(&self) -> &'static str {
        match self {
            Category::Fix => "Fixed",
            Category::Var => "Variable",
        }
    }
}

pub struct Systems;

impl Systems {
    pub fn tab() -> Tab {
        Tab::Systems
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut data = world.expect_resource_mut::<ProjectData>();
        let project = world.expect_resource::<Project>();

        const STYLE: SnarlStyle = SnarlStyle::new();

        data.systems
            .snarl
            .show(&mut SystemViewer, &STYLE, "systems", ui);

        try_log_err!(sync_project(&project, &mut data));
    }

    pub fn update_plugins<'a>(
        systems: &mut SystemGraph,
        plugins: impl Iterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)>,
    ) {
        let mut all_systems = HashSet::new();

        for (name, plugin) in plugins {
            for &system in plugin.systems() {
                all_systems.insert((name, system));
            }
        }

        let mut bb = egui::Rect::NOTHING;

        for (pos, node) in systems.snarl.nodes_pos_mut() {
            node.active = all_systems.remove(&(&*node.plugin, &*node.system));
            bb.extend_with(pos);
            bb.extend_with(pos + egui::vec2(100.0, 100.0));
        }

        if bb.is_negative() {
            bb = egui::Rect::ZERO;
        }

        let new_systems = all_systems
            .into_iter()
            .map(|(plugin, system)| SystemNode {
                plugin: plugin.to_buf(),
                system: system.to_buf(),
                active: true,
                enabled: false,
                category: Category::Fix,
                deps: HashSet::new(),
            })
            .collect::<Vec<_>>();

        for system in new_systems {
            let off = rand::random::<f32>();
            let pos = match rand::random::<u8>() % 4 {
                0 => bb.min + egui::vec2(off * bb.width(), -20.0),
                1 => bb.min + egui::vec2(-20.0, off * bb.height()),
                2 => bb.max - egui::vec2(off * bb.width(), -20.0),
                3 => bb.max - egui::vec2(-20.0, off * bb.height()),
                _ => unreachable!(),
            };

            systems.snarl.insert_node(pos, system);
            bb.extend_with(pos);
            bb.extend_with(pos + egui::vec2(100.0, 100.0));
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    plugin: IdentBuf,
    system: IdentBuf,
    enabled: bool,
    category: Category,

    #[serde(skip)]
    active: bool,

    #[serde(skip)]
    deps: HashSet<usize>,
}

struct SystemViewer;

impl SnarlViewer<SystemNode> for SystemViewer {
    fn title<'a>(&'a mut self, node: &'a SystemNode) -> String {
        format!("{}@{}", node.system, node.plugin)
    }

    fn show_header(
        &mut self,
        idx: usize,
        _inputs: &[InPin],
        _utputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) {
        let mut remove = false;
        let mut toggle = false;
        let node = snarl.get_node_mut(idx);

        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let cb = egui::Checkbox::new(&mut node.enabled, node.system.as_str());
                let r = ui.add_enabled(node.active, cb);

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
                toggle_ui(ui, &mut is_fix);
                ui.label("Fixed rate");
                toggle = is_fix != (node.category == Category::Fix);
            });
        });

        if remove {
            snarl.remove_node(idx);
        } else if toggle {
            node.category = match node.category {
                Category::Fix => Category::Var,
                Category::Var => Category::Fix,
            };

            snarl.drop_inputs(InPinId {
                node: idx,
                input: 0,
            });
            snarl.drop_outputs(OutPinId {
                node: idx,
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

    fn input_color(
        &mut self,
        _: &InPin,
        _style: &egui::Style,
        _snarl: &mut Snarl<SystemNode>,
    ) -> Color32 {
        Color32::LIGHT_GRAY
    }

    fn output_color(
        &mut self,
        _: &OutPin,
        _style: &egui::Style,
        _snarl: &mut Snarl<SystemNode>,
    ) -> Color32 {
        Color32::LIGHT_GRAY
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

        let node = snarl.get_node_mut(pin.id.node);

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

        let node = snarl.get_node_mut(pin.id.node);

        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        pin_info.with_shape(node.category.pin_shape())
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<SystemNode>) {
        if from.id.node == to.id.node {
            return;
        }

        let from_cat = snarl.get_node(from.id.node).category;
        let to_cat = snarl.get_node(to.id.node).category;
        if from_cat != to_cat {
            return;
        }

        snarl.connect(from.id, to.id);
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<SystemNode>) {
        snarl.disconnect(from.id, to.id);
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<SystemNode>) {
        snarl.drop_outputs(pin.id);
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<SystemNode>) {
        snarl.drop_inputs(pin.id);
    }

    fn graph_menu(
        &mut self,
        _pos: egui::Pos2,
        _ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) {
    }

    fn node_menu(
        &mut self,
        _idx: usize,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        _ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<SystemNode>,
    ) {
    }
}
