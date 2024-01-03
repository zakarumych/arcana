use arcana::{
    color_hash,
    edict::world::WorldLocal,
    plugin::JobId,
    work::{Image2D, JobDesc, Target, WorkGraph},
    Stid,
};
use arcana_project::IdentBuf;
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, OutPin, Snarl,
};

pub enum WorkGraphNode {
    Job {
        id: JobId,
        name: IdentBuf,
        job: JobDesc,
    },
    MainPresent,
}

pub struct WorkGraphViewer<'a> {
    world: &'a mut WorldLocal,
    workgraph: &'a mut WorkGraph,
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
        idx: usize,
        inputs: &[egui_snarl::InPin],
        outputs: &[egui_snarl::OutPin],
        ui: &mut egui::Ui,
        scale: f32,
        snarl: &mut egui_snarl::Snarl<WorkGraphNode>,
    ) {
        let node = snarl.get_node(idx);
        match *node {
            WorkGraphNode::Job { ref name, .. } => {
                ui.label(format!("Job: {}", name));
            }
            WorkGraphNode::MainPresent => {
                ui.label("Present");
            }
        }
    }

    fn inputs(&mut self, node: &WorkGraphNode) -> usize {
        match *node {
            WorkGraphNode::Job { ref job, .. } => job.updates.len() + job.reads.len(),
            WorkGraphNode::MainPresent => 1,
        }
    }

    fn outputs(&mut self, node: &WorkGraphNode) -> usize {
        match *node {
            WorkGraphNode::Job { ref job, .. } => job.updates.len() + job.creates.len(),
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
        let node = snarl.get_node(pin.id.node);
        match *node {
            WorkGraphNode::Job { ref job, .. } => {
                if pin.id.input >= job.updates.len() + job.reads.len() {
                    unreachable!()
                }
                if pin.id.input < job.updates.len() {
                    let update = &job.updates[pin.id.input];
                    ui.label("updates");
                    let [r, g, b] = color_hash(&update.kind);
                    PinInfo::square().with_fill(egui::Color32::from_rgb(r, g, b))
                } else {
                    let read = &job.reads[pin.id.input - job.updates.len()];
                    ui.label("reads");
                    let [r, g, b] = color_hash(&read.kind);
                    PinInfo::circle().with_fill(egui::Color32::from_rgb(r, g, b))
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
        let node = snarl.get_node(pin.id.node);
        match *node {
            WorkGraphNode::Job { ref job, .. } => {
                if pin.id.output >= job.updates.len() + job.creates.len() {
                    unreachable!()
                }
                if pin.id.output < job.updates.len() {
                    let update = &job.updates[pin.id.output];
                    ui.label("updates");
                    let [r, g, b] = color_hash(&update.kind);
                    PinInfo::square().with_fill(egui::Color32::from_rgb(r, g, b))
                } else {
                    let create = &job.creates[pin.id.output - job.updates.len()];
                    ui.label("creates");
                    let [r, g, b] = color_hash(&create.kind);
                    PinInfo::triangle().with_fill(egui::Color32::from_rgb(r, g, b))
                }
            }
            WorkGraphNode::MainPresent => {
                unreachable!()
            }
        }
    }

    fn input_color(
        &mut self,
        pin: &InPin,
        _style: &egui::Style,
        snarl: &mut Snarl<WorkGraphNode>,
    ) -> egui::Color32 {
        let node = snarl.get_node(pin.id.node);
        match *node {
            WorkGraphNode::Job { ref job, .. } => {
                if pin.id.input < job.updates.len() {
                    let update = &job.updates[pin.id.input];
                    let [r, g, b] = color_hash(&update.kind);
                    egui::Color32::from_rgb(r, g, b)
                } else {
                    let read = &job.reads[pin.id.input - job.updates.len()];
                    let [r, g, b] = color_hash(&read.kind);
                    egui::Color32::from_rgb(r, g, b)
                }
            }
            WorkGraphNode::MainPresent => present_pin_color(),
        }
    }

    fn output_color(
        &mut self,
        pin: &OutPin,
        _style: &egui::Style,
        snarl: &mut Snarl<WorkGraphNode>,
    ) -> egui::Color32 {
        let node = snarl.get_node(pin.id.node);
        match *node {
            WorkGraphNode::Job { ref job, .. } => {
                if pin.id.output < job.updates.len() {
                    let update = &job.updates[pin.id.output];
                    let [r, g, b] = color_hash(&update.kind);
                    egui::Color32::from_rgb(r, g, b)
                } else {
                    let create = &job.creates[pin.id.output - job.updates.len()];
                    let [r, g, b] = color_hash(&create.kind);
                    egui::Color32::from_rgb(r, g, b)
                }
            }
            WorkGraphNode::MainPresent => present_pin_color(),
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<WorkGraphNode>) {
        let from_node = snarl.get_node(from.id.node);
        let to_node = snarl.get_node(to.id.node);
        match (from_node, to_node) {
            (
                WorkGraphNode::Job {
                    job: ref from_job, ..
                },
                WorkGraphNode::Job {
                    job: ref to_job, ..
                },
            ) => {
                if from_job.output_kind(from.id.output) == to_job.input_kind(to.id.input) {
                    for &r in &to.remotes {
                        snarl.disconnect(r, to.id);
                    }
                    snarl.connect(from.id, to.id);
                }
            }
            (
                WorkGraphNode::Job {
                    job: ref to_job, ..
                },
                WorkGraphNode::MainPresent,
            ) => {
                let to = &to_job.updates[to.id.input];
                assert_eq!(present_kind(), to.kind);
            }
            _ => unreachable!(),
        }
    }
}

fn present_kind() -> Stid {
    Stid::of::<Image2D>()
}

fn present_pin_color() -> egui::Color32 {
    let [r, g, b] = color_hash(&present_kind());
    egui::Color32::from_rgb(r, g, b)
}
