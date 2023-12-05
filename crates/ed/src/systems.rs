use std::cell::RefCell;

use arcana::{edict::world::WorldLocal, plugin::ArcanaPlugin, project::Project, World};
use arcana_project::{Ident, IdentBuf};
use egui::{Color32, InnerResponse, Ui, WidgetText};
use egui_snarl::{
    ui::{Effects, Forbidden, InPin, OutPin, PinInfo, PinShape, SnarlViewer},
    Snarl,
};
use hashbrown::HashSet;

use crate::{data::ProjectData, move_element};

use super::{plugins::Plugins, Tab};

fn run_systems(world: &mut World, snarl: Snarl<SystemNode>) {}

pub struct Systems;

impl Systems {
    pub fn tab() -> Tab {
        Tab::Systems
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut project = world.expect_resource_mut::<Project>();
        let plugins = world.expect_resource::<Plugins>();

        let mut data = world.expect_resource_mut::<ProjectData>();

        // Update system graph;
    }

    pub fn update_plugins<'a>(
        systems: &mut SystemGraph,
        plugins: impl Iterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)>,
    ) {
        let mut effects = Effects::new();

        let mut all_systems = HashSet::new();

        for (name, plugin) in plugins {
            for &system in plugin.systems() {
                all_systems.insert((name, system));
            }
        }

        for (idx, node) in systems.snarl.nodes_indices_mut() {
            match node {
                SystemNode::System { plugin, system } => {
                    if !all_systems.remove(&(&**plugin, &**system)) {
                        effects.remove_node(idx);
                    }
                }
                _ => {}
            }
        }

        for (plugin, system) in all_systems {
            effects.insert_node(
                SystemNode::System {
                    plugin: plugin.to_buf(),
                    system: system.to_buf(),
                },
                Default::default(),
            );
        }

        systems.snarl.apply_effects(effects);
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SystemGraph {
    snarl: Snarl<SystemNode>,
}

impl SystemGraph {
    pub fn new() -> Self {
        let mut snarl = Snarl::new();

        snarl.add_node(SystemNode::Begin, egui::pos2(-100.0, -100.0));
        snarl.add_node(SystemNode::FixedBegin, egui::pos2(-100.0, 0.0));
        snarl.add_node(SystemNode::FixedEnd, egui::pos2(100.0, 0.0));
        snarl.add_node(SystemNode::End, egui::pos2(100.0, 100.0));

        SystemGraph { snarl }
    }
}

impl Default for SystemGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SystemNode {
    /// Exclusive node that represents beginning of the system graph.
    /// It has no inputs and one output.
    Begin,

    /// Exclusive node that represents beginning of the fixed systems.
    /// It has two inputs and one output.
    /// First input is connected to the `Begin` node.
    /// Second input is connected to the `FixedEnd` node, creating a loop.
    FixedBegin,

    /// Exclusive node that represents end of the fixed systems and beginning of the variable systems.
    /// It has exactly one
    FixedEnd,

    /// Exclusive node that represents end of the system graph.
    End,
    /// Node that represents particular system.
    /// Addressed by plugin name and system name.
    System { plugin: IdentBuf, system: IdentBuf },
}

struct SystemViewer;

impl SnarlViewer<SystemNode> for SystemViewer {
    fn title<'a>(&'a mut self, node: &'a SystemNode) -> String {
        match node {
            SystemNode::Begin => "Begin".to_owned(),
            SystemNode::FixedBegin => "Fixed Begin".to_owned(),
            SystemNode::FixedEnd => "Fixed End".to_owned(),
            SystemNode::End => "End".to_owned(),
            SystemNode::System { plugin, system } => {
                format!("{}::{}", plugin, system)
            }
        }
    }

    fn inputs(&mut self, node: &SystemNode) -> usize {
        match node {
            SystemNode::Begin => 0,
            SystemNode::FixedBegin => 2,
            SystemNode::FixedEnd => 1,
            SystemNode::End => 1,
            SystemNode::System { .. } => 1,
        }
    }

    fn outputs(&mut self, node: &SystemNode) -> usize {
        match node {
            SystemNode::Begin => 1,
            SystemNode::FixedBegin => 1,
            SystemNode::FixedEnd => 2,
            SystemNode::End => 0,
            SystemNode::System { .. } => 1,
        }
    }

    fn input_color(&mut self, _: &InPin<SystemNode>, style: &egui::Style) -> Color32 {
        style.visuals.widgets.noninteractive.bg_fill
    }

    fn output_color(&mut self, _: &OutPin<SystemNode>, style: &egui::Style) -> Color32 {
        style.visuals.widgets.noninteractive.bg_fill
    }

    fn show_input(
        &mut self,
        pin: &InPin<SystemNode>,
        ui: &mut Ui,
        _scale: f32,
        _effects: &mut Effects<SystemNode>,
    ) -> egui::InnerResponse<PinInfo> {
        let pin_fill = ui.visuals().widgets.noninteractive.bg_fill;
        let pin_stroke = ui.visuals().widgets.noninteractive.fg_stroke;

        let pin_info = PinInfo::default()
            .with_fill(pin_fill)
            .with_stroke(pin_stroke);

        match (&*pin.node.borrow(), pin.id.input) {
            (SystemNode::FixedBegin, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Triangle), r)
            }
            (SystemNode::FixedBegin, 1) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Square), r)
            }
            (SystemNode::FixedEnd, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Square), r)
            }
            (SystemNode::End, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Triangle), r)
            }
            (SystemNode::System { .. }, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Circle), r)
            }
            _ => unreachable!(),
        }
    }

    fn show_output(
        &mut self,
        pin: &OutPin<SystemNode>,
        ui: &mut Ui,
        _scale: f32,
        _effects: &mut Effects<SystemNode>,
    ) -> egui::InnerResponse<PinInfo> {
        let pin_fill = ui.visuals().widgets.noninteractive.bg_fill;
        let pin_stroke = ui.visuals().widgets.noninteractive.fg_stroke;

        let pin_info = PinInfo::default()
            .with_fill(pin_fill)
            .with_stroke(pin_stroke);

        match (&*pin.node.borrow(), pin.id.output) {
            (SystemNode::Begin, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Triangle), r)
            }
            (SystemNode::FixedBegin, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Square), r)
            }
            (SystemNode::FixedEnd, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Square), r)
            }
            (SystemNode::FixedEnd, 1) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Triangle), r)
            }
            (SystemNode::System { .. }, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(PinShape::Circle), r)
            }
            _ => unreachable!(),
        }
    }

    fn connect(
        &mut self,
        from: &OutPin<SystemNode>,
        to: &InPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        match (
            &*from.node.borrow(),
            from.id.output,
            &*to.node.borrow(),
            to.id.input,
        ) {
            (SystemNode::FixedEnd, 1, SystemNode::System { .. }, 0) => {
                effects.connect(from.id, to.id);
            }
            (SystemNode::System { .. }, 0, SystemNode::System { .. }, 0) => {
                effects.connect(from.id, to.id);
            }
            (SystemNode::System { .. }, 0, SystemNode::End, 0) => {
                effects.connect(from.id, to.id);
            }
            _ => return Err(Forbidden),
        }

        Ok(())
    }

    fn disconnect(
        &mut self,
        from: &OutPin<SystemNode>,
        to: &InPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        Err(Forbidden)
    }

    fn drop_outputs(
        &mut self,
        pin: &OutPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        match (&*pin.node.borrow(), pin.id.output) {
            (SystemNode::FixedEnd, 1) => {
                effects.drop_outputs(pin.id);
            }
            (SystemNode::System { .. }, 0) => {
                effects.drop_outputs(pin.id);
            }
            _ => return Err(Forbidden),
        }
        Ok(())
    }

    fn drop_inputs(
        &mut self,
        pin: &InPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        match (&*pin.node.borrow(), pin.id.input) {
            (SystemNode::End, 0) => {
                effects.drop_inputs(pin.id);
            }
            (SystemNode::System { .. }, 0) => {
                effects.drop_inputs(pin.id);
            }
            _ => return Err(Forbidden),
        }
        Ok(())
    }

    fn graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        scale: f32,
        effects: &mut Effects<SystemNode>,
    ) {
        let _ = (pos, ui, scale, effects);
    }

    fn node_menu(
        &mut self,
        idx: usize,
        node: &RefCell<SystemNode>,
        inputs: &[InPin<SystemNode>],
        outputs: &[OutPin<SystemNode>],
        ui: &mut Ui,
        scale: f32,
        effects: &mut Effects<SystemNode>,
    ) {
    }
}
