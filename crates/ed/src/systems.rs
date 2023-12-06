use std::cell::RefCell;

use arcana::{edict::world::WorldLocal, plugin::ArcanaPlugin, project::Project, World};
use arcana_project::{Ident, IdentBuf};
use egui::{Color32, InnerResponse, Ui, WidgetText};
use egui_snarl::{
    ui::{Effects, Forbidden, InPin, OutPin, PinInfo, PinShape, SnarlStyle, SnarlViewer},
    Snarl,
};
use hashbrown::HashSet;

use crate::{data::ProjectData, move_element, sync_project};

use super::{plugins::Plugins, Tab};

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

fn pin_shape(category: Option<Category>) -> PinShape {
    match category {
        None => PinShape::Circle,
        Some(Category::Fix) => PinShape::Square,
        Some(Category::Var) => PinShape::Triangle,
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
        let mut effects = Effects::new();

        let mut all_systems = HashSet::new();

        for (name, plugin) in plugins {
            for &system in plugin.systems() {
                all_systems.insert((name, system));
            }
        }

        for (idx, node) in systems.snarl.nodes_indices_mut() {
            match *node {
                SystemNode::System {
                    ref plugin,
                    ref system,
                    ref mut active,
                    ..
                } => {
                    *active = all_systems.remove(&(&**plugin, &**system));
                }
                _ => {}
            }
        }

        for (plugin, system) in all_systems {
            effects.insert_node(
                Default::default(),
                SystemNode::System {
                    plugin: plugin.to_buf(),
                    system: system.to_buf(),
                    active: true,
                    enabled: false,
                    category: None,
                },
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

        snarl.insert_node(
            egui::pos2(0.0, 0.0),
            SystemNode::Begin {
                category: Category::Fix,
            },
        );
        snarl.insert_node(
            egui::pos2(0.0, 100.0),
            SystemNode::Begin {
                category: Category::Var,
            },
        );

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
    Begin { category: Category },

    /// Node that represents particular system.
    /// Addressed by plugin name and system name.
    System {
        plugin: IdentBuf,
        system: IdentBuf,
        enabled: bool,
        active: bool,
        category: Option<Category>,
    },
}

impl SystemNode {
    fn category(&self) -> Option<Category> {
        match *self {
            SystemNode::Begin { category } => Some(category),
            SystemNode::System { category, .. } => category,
        }
    }
}

struct SystemViewer;

impl SnarlViewer<SystemNode> for SystemViewer {
    fn title<'a>(&'a mut self, node: &'a SystemNode) -> String {
        match node {
            SystemNode::Begin { .. } => "Begin".to_owned(),
            SystemNode::System { plugin, system, .. } => {
                format!("{}@{}", system, plugin)
            }
        }
    }

    fn show_header(
        &mut self,
        idx: usize,
        node: &RefCell<SystemNode>,
        _inputs: &[InPin<SystemNode>],
        _utputs: &[OutPin<SystemNode>],
        ui: &mut Ui,
        _scale: f32,
        effects: &mut Effects<SystemNode>,
    ) -> egui::Response {
        match *node.borrow_mut() {
            SystemNode::Begin {
                category: Category::Fix,
            } => ui.label("Fix Begin"),
            SystemNode::Begin {
                category: Category::Var,
            } => ui.label("Var Begin"),
            SystemNode::System {
                ref plugin,
                ref system,
                ref mut enabled,
                active,
                ..
            } => {
                ui.horizontal(|ui| {
                    let cb = egui::Checkbox::new(enabled, system.as_str());
                    let r = ui.add_enabled(active, cb);

                    r.on_hover_text("Enable/disable system");

                    ui.weak(egui_phosphor::regular::AT);
                    ui.label(plugin.as_str());
                    let r = ui.add_enabled(
                        !active,
                        egui::Button::new(egui_phosphor::regular::TRASH_SIMPLE).small(),
                    );

                    if r.clicked() {
                        effects.remove_node(idx);
                    }

                    r.on_hover_ui(|ui| {
                        ui.label("Remove system from graph");
                        ui.label("The system is not found in active plugins");
                        ui.label(
                            "If plugins is reactivated and system is found, it will be added back",
                        );
                    });
                })
                .response
            }
        }
    }

    fn inputs(&mut self, node: &SystemNode) -> usize {
        match node {
            SystemNode::Begin { .. } => 0,
            SystemNode::System { .. } => 1,
        }
    }

    fn outputs(&mut self, node: &SystemNode) -> usize {
        match node {
            SystemNode::Begin { .. } => 1,
            SystemNode::System { .. } => 1,
        }
    }

    fn input_color(&mut self, _: &InPin<SystemNode>, _style: &egui::Style) -> Color32 {
        Color32::LIGHT_GRAY
    }

    fn output_color(&mut self, _: &OutPin<SystemNode>, _style: &egui::Style) -> Color32 {
        Color32::LIGHT_GRAY
    }

    fn show_input(
        &mut self,
        pin: &InPin<SystemNode>,
        ui: &mut Ui,
        _scale: f32,
        _effects: &mut Effects<SystemNode>,
    ) -> egui::InnerResponse<PinInfo> {
        let pin_fill = Color32::LIGHT_GRAY;
        let pin_stroke = ui.visuals().widgets.inactive.fg_stroke;

        let pin_info = PinInfo::default()
            .with_fill(pin_fill)
            .with_stroke(pin_stroke);

        match (&mut *pin.node.borrow_mut(), pin.id.input) {
            (SystemNode::Begin { category }, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(pin_shape(Some(*category))), r)
            }
            (SystemNode::System { category, .. }, 0) => {
                let pin_shape = pin_shape(*category);

                *category = None;

                for remote in &pin.remotes {
                    if let Some(c) = remote.node.borrow().category() {
                        *category = Some(c);
                    }
                }

                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(pin_shape), r)
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
        let pin_fill = Color32::LIGHT_GRAY;
        let pin_stroke = ui.visuals().widgets.noninteractive.fg_stroke;

        let pin_info = PinInfo::default()
            .with_fill(pin_fill)
            .with_stroke(pin_stroke);

        match (&mut *pin.node.borrow_mut(), pin.id.output) {
            (SystemNode::Begin { category }, 0) => {
                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(pin_shape(Some(*category))), r)
            }
            (SystemNode::System { category, .. }, 0) => {
                if category.is_none() {
                    for remote in &pin.remotes {
                        if let Some(c) = remote.node.borrow().category() {
                            *category = Some(c);
                        }
                    }
                }

                let r = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                InnerResponse::new(pin_info.with_shape(pin_shape(*category)), r)
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
        if from.id.node == to.id.node {
            return Err(Forbidden);
        }

        let from_cat = from.node.borrow().category();
        let to_cat = to.node.borrow().category();
        match (from_cat, to_cat) {
            (None, _) | (_, None) => {}
            (Some(from), Some(to)) if from == to => {}
            _ => return Err(Forbidden),
        }

        effects.connect(from.id, to.id);
        Ok(())
    }

    fn disconnect(
        &mut self,
        from: &OutPin<SystemNode>,
        to: &InPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        effects.disconnect(from.id, to.id);
        Ok(())
    }

    fn drop_outputs(
        &mut self,
        pin: &OutPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        effects.drop_outputs(pin.id);
        Ok(())
    }

    fn drop_inputs(
        &mut self,
        pin: &InPin<SystemNode>,
        effects: &mut Effects<SystemNode>,
    ) -> Result<(), Forbidden> {
        effects.drop_inputs(pin.id);
        Ok(())
    }

    fn graph_menu(
        &mut self,
        _pos: egui::Pos2,
        _ui: &mut Ui,
        _scale: f32,
        _effects: &mut Effects<SystemNode>,
    ) {
    }

    fn node_menu(
        &mut self,
        _idx: usize,
        _node: &RefCell<SystemNode>,
        _inputs: &[InPin<SystemNode>],
        _outputs: &[OutPin<SystemNode>],
        _ui: &mut Ui,
        _scale: f32,
        _effects: &mut Effects<SystemNode>,
    ) {
    }
}
