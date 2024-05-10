use arcana::{
    edict::world::WorldLocal,
    project::{ident, Ident, IdentBuf, Project},
    work::{Edge, Image2D, JobDesc, JobId, PinId},
    Stid, World,
};
use egui::Ui;
use egui_snarl::{
    ui::{AnyPins, PinInfo, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::HashMap;

use crate::{container::Container, data::ProjectData, hue_hash};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct WorkGraph {
    snarl: Snarl<WorkGraphNode>,
}

impl WorkGraph {
    pub fn make_workgraph(&self) -> Result<arcana::work::WorkGraph, arcana::work::Cycle> {
        let jobs = self
            .snarl
            .nodes()
            .filter_map(|node| match node {
                WorkGraphNode::Job { job, desc, .. } => Some((*job, desc.clone())),
                _ => None,
            })
            .collect();

        let edges = self
            .snarl
            .wires()
            .filter_map(|(from, to)| {
                let from = match self.snarl.get_node(from.node) {
                    Some(&WorkGraphNode::Job { job, .. }) => PinId {
                        job,
                        idx: from.output,
                    },
                    _ => return None,
                };
                let to = match self.snarl.get_node(to.node) {
                    Some(&WorkGraphNode::Job { job, .. }) => PinId { job, idx: to.input },
                    _ => return None,
                };
                Some(Edge { from, to })
            })
            .collect();

        arcana::work::WorkGraph::new(jobs, edges)
    }

    pub fn get_present(&self) -> Option<PinId> {
        for (from, to) in self.snarl.wires() {
            if let Some(WorkGraphNode::MainPresent) = self.snarl.get_node(to.node) {
                if let Some(&WorkGraphNode::Job { job, .. }) = self.snarl.get_node(from.node) {
                    return Some(PinId {
                        job,
                        idx: from.output,
                    });
                }
            }
        }
        None
    }
}

pub struct Rendering {
    available: Vec<WorkGraphNode>,
    modification: u64,
}

impl Rendering {
    pub fn new() -> Self {
        Rendering {
            available: vec![WorkGraphNode::MainPresent],
            modification: 1,
        }
    }

    pub fn modification(&self) -> u64 {
        self.modification
    }

    pub fn update_plugins(&mut self, data: &mut ProjectData, container: &Container) {
        let mut all_jobs = HashMap::new();

        for (name, plugin) in container.plugins() {
            for job in plugin.jobs() {
                all_jobs.insert(job.id, (name, job.name, job.desc));
            }
        }

        let mut main_present_available = true;

        for node in data.workgraph.snarl.nodes_mut() {
            match node {
                WorkGraphNode::Job { job, active, .. } => {
                    *active = all_jobs.remove(&*job).is_some()
                }
                WorkGraphNode::MainPresent => {
                    main_present_available = false;
                }
            }
        }

        let new_jobs = all_jobs
            .into_iter()
            .map(|(id, (plugin, job, desc))| WorkGraphNode::Job {
                job: id,
                name: job.into_owned(),
                plugin: plugin.to_owned(),
                desc,
                active: true,
            })
            .collect::<Vec<_>>();

        self.available = new_jobs;

        if main_present_available {
            self.available.push(WorkGraphNode::MainPresent);
        }

        self.available
            .sort_by_cached_key(|node| node.name().to_owned());
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut data = world.expect_resource_mut::<ProjectData>();
        let project = world.expect_resource::<Project>();
        let mut rendering = world.expect_resource_mut::<Rendering>();

        const STYLE: SnarlStyle = SnarlStyle::new();

        let mut viewer = WorkGraphViewer {
            modified: false,
            available: &mut rendering.available,
        };

        data.workgraph
            .snarl
            .show(&mut viewer, &STYLE, "work-graph", ui);

        if viewer.modified {
            rendering.modification += 1;
        }

        try_log_err!(data.sync(&project));
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WorkGraphNode {
    Job {
        job: JobId,
        name: IdentBuf,
        plugin: IdentBuf,
        desc: JobDesc,
        active: bool,
    },
    MainPresent,
}

impl WorkGraphNode {
    pub fn name(&self) -> &Ident {
        match self {
            WorkGraphNode::Job { name, .. } => name,
            WorkGraphNode::MainPresent => ident!(Present),
        }
    }
}

pub struct WorkGraphViewer<'a> {
    modified: bool,
    available: &'a mut Vec<WorkGraphNode>,
}

impl SnarlViewer<WorkGraphNode> for WorkGraphViewer<'_> {
    fn title(&mut self, node: &WorkGraphNode) -> String {
        match *node {
            WorkGraphNode::Job { ref name, .. } => name.as_str().to_owned(),
            WorkGraphNode::MainPresent => "Present".to_owned(),
        }
    }

    fn show_header(
        &mut self,
        id: NodeId,
        _: &[egui_snarl::InPin],
        _: &[egui_snarl::OutPin],
        ui: &mut egui::Ui,
        _: f32,
        snarl: &mut egui_snarl::Snarl<WorkGraphNode>,
    ) {
        let remove;

        match snarl[id] {
            WorkGraphNode::Job {
                ref name,
                ref plugin,
                ..
            } => {
                ui.label(name.as_str());
                ui.weak(egui_phosphor::regular::AT);
                ui.label(plugin.as_str());

                let r = ui.small_button(egui_phosphor::regular::TRASH_SIMPLE);

                remove = r.clicked();

                r.on_hover_ui(|ui| {
                    ui.label("Remove job from graph");
                });
            }
            WorkGraphNode::MainPresent => {
                ui.label("Present");

                let r = ui.small_button(egui_phosphor::regular::TRASH_SIMPLE);

                remove = r.clicked();

                r.on_hover_ui(|ui| {
                    ui.label("Remove present from graph");
                });
            }
        }

        if remove {
            let node = snarl.remove_node(id);
            self.available.push(node);
            self.available
                .sort_by_cached_key(|node| node.name().to_owned());
            self.modified = true;
        }
    }

    fn inputs(&mut self, node: &WorkGraphNode) -> usize {
        match *node {
            WorkGraphNode::Job { ref desc, .. } => desc.updates.len() + desc.reads.len(),
            WorkGraphNode::MainPresent => 1,
        }
    }

    fn outputs(&mut self, node: &WorkGraphNode) -> usize {
        match *node {
            WorkGraphNode::Job { ref desc, .. } => desc.updates.len() + desc.creates.len(),
            WorkGraphNode::MainPresent => 0,
        }
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<WorkGraphNode>,
    ) -> PinInfo {
        match snarl[pin.id.node] {
            WorkGraphNode::Job { ref desc, .. } => {
                if pin.id.input >= desc.updates.len() + desc.reads.len() {
                    unreachable!()
                }
                if pin.id.input < desc.updates.len() {
                    let update = &desc.updates[pin.id.input];
                    ui.label("updates");
                    PinInfo::square().with_fill(hue_hash(&update.ty))
                } else {
                    let read = &desc.reads[pin.id.input - desc.updates.len()];
                    ui.label("reads");
                    PinInfo::circle().with_fill(hue_hash(&read.ty))
                }
            }
            WorkGraphNode::MainPresent => {
                ui.label("presents");
                PinInfo::circle().with_fill(present_pin_color())
            }
        }
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<WorkGraphNode>,
    ) -> PinInfo {
        match snarl[pin.id.node] {
            WorkGraphNode::Job { ref desc, .. } => {
                if pin.id.output >= desc.updates.len() + desc.creates.len() {
                    unreachable!()
                }
                if pin.id.output < desc.updates.len() {
                    let update = &desc.updates[pin.id.output];
                    ui.label("updates");
                    PinInfo::square().with_fill(hue_hash(&update.ty))
                } else {
                    let create = &desc.creates[pin.id.output - desc.updates.len()];
                    ui.label("creates");
                    PinInfo::triangle().with_fill(hue_hash(&create.ty))
                }
            }
            WorkGraphNode::MainPresent => {
                unreachable!()
            }
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<WorkGraphNode>) {
        let from_node = &snarl[from.id.node];
        let to_node = &snarl[to.id.node];
        match (from_node, to_node) {
            (
                WorkGraphNode::Job { desc: from_job, .. },
                WorkGraphNode::Job { desc: to_job, .. },
            ) => {
                if from_job.output_type(from.id.output) == to_job.input_type(to.id.input) {
                    debug_assert!(to.remotes.len() <= 1);
                    for &r in &to.remotes {
                        snarl.disconnect(r, to.id);
                    }
                    snarl.connect(from.id, to.id);
                    self.modified = true;
                }
            }
            (WorkGraphNode::Job { desc: from_job, .. }, WorkGraphNode::MainPresent) => {
                if from_job.output_type(from.id.output) == present_kind() {
                    debug_assert!(to.remotes.len() <= 1);
                    for &r in &to.remotes {
                        snarl.disconnect(r, to.id);
                    }
                    snarl.connect(from.id, to.id);
                    self.modified = true;
                }
            }
            _ => unreachable!(),
        }
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<WorkGraphNode>) {
        snarl.disconnect(from.id, to.id);
        self.modified = true;
    }

    /// Checks if the snarl has something to show in context menu if wire drag is stopped at `pos`.
    #[inline(always)]
    fn has_dropped_wire_menu(&mut self, _: AnyPins, _: &mut Snarl<WorkGraphNode>) -> bool {
        true
    }

    /// Show context menu for the snarl. This menu is opened when releasing a pin to empty
    /// space. It can be used to implement menu for adding new node, and directly
    /// connecting it to the released wire.
    #[cfg_attr(inline_more, inline)]
    fn show_dropped_wire_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        src_pins: AnyPins,
        snarl: &mut Snarl<WorkGraphNode>,
    ) {
        ui.label("Add job");
        ui.separator();

        if self.available.is_empty() {
            ui.weak("No available jobs");
        }

        for idx in 0..self.available.len() {
            let s = &self.available[idx];
            if ui.button(s.name().as_str()).clicked() {
                ui.close_menu();
                let s = self.available.remove(idx);
                let new_node = snarl.insert_node(pos, s);

                match src_pins {
                    AnyPins::In(pins) => {
                        for &pin in pins {
                            self.connect(
                                &snarl.out_pin(OutPinId {
                                    node: new_node,
                                    output: 0,
                                }),
                                &snarl.in_pin(pin),
                                snarl,
                            );
                        }
                    }
                    AnyPins::Out(pins) => {
                        for &pin in pins {
                            self.connect(
                                &snarl.out_pin(pin),
                                &snarl.in_pin(InPinId {
                                    node: new_node,
                                    input: 0,
                                }),
                                snarl,
                            );
                        }
                    }
                }

                return;
            }
        }
    }

    #[inline(always)]
    fn has_graph_menu(&mut self, _: egui::Pos2, _: &mut Snarl<WorkGraphNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<WorkGraphNode>,
    ) {
        ui.label("Add job");
        ui.separator();

        if self.available.is_empty() {
            ui.weak("No available jobs");
        }

        for idx in 0..self.available.len() {
            let s = &self.available[idx];
            if ui.button(s.name().as_str()).clicked() {
                ui.close_menu();
                let s = self.available.remove(idx);
                snarl.insert_node(pos, s);
                return;
            }
        }
    }
}

#[inline(always)]
fn present_kind() -> Stid {
    Stid::of::<Image2D>()
}

#[inline(always)]
fn present_pin_color() -> egui::Color32 {
    hue_hash(&present_kind())
}
