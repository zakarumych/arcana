use arcana::{
    color_hash,
    edict::world::WorldLocal,
    work::{Access, JobId, WorkGraph},
};
use egui_snarl::{
    ui::{PinInfo, PinShape, SnarlViewer},
    InPin, OutPin, Snarl,
};

pub enum WorkGraphNode {
    Job(JobId),
}

pub struct WorkGraphViewer<'a> {
    world: &'a mut WorldLocal,
    workgraph: &'a mut WorkGraph,
}

impl SnarlViewer<WorkGraphNode> for WorkGraphViewer<'_> {
    fn title(&mut self, node: &WorkGraphNode) -> String {
        match *node {
            WorkGraphNode::Job(job) => self.workgraph.name(job).to_owned(),
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
            WorkGraphNode::Job(job) => {
                let name = self.workgraph.name(job);
                ui.label(format!("Pass: {}", name));
            }
        }
    }

    fn inputs(&mut self, node: &WorkGraphNode) -> usize {
        match node {
            WorkGraphNode::Job(job) => self.workgraph.inputs(*job).len(),
        }
    }

    fn outputs(&mut self, node: &WorkGraphNode) -> usize {
        match node {
            WorkGraphNode::Job(job) => self.workgraph.outputs(*job).len(),
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
            WorkGraphNode::Job(job) => {
                let (id, input) = self.workgraph.inputs(job).nth(pin.id.input).unwrap();

                ui.label(input.name);

                let shape = match input.access {
                    Access::Shared => PinShape::Square,
                    Access::Exclusive => PinShape::Triangle,
                };

                let target = self.workgraph.input_target(id);
                let [r, g, b] = color_hash(&target);

                PinInfo::default()
                    .with_shape(shape)
                    .with_fill(egui::Color32::from_rgb(r, g, b))
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
            WorkGraphNode::Job(job) => {
                let (id, output) = self.workgraph.outputs(job).nth(pin.id.output).unwrap();

                ui.label(output.name);

                let target = self.workgraph.output_target(id);
                let [r, g, b] = color_hash(&target);

                PinInfo::circle().with_fill(egui::Color32::from_rgb(r, g, b))
            }
        }
    }

    fn input_color(
        &mut self,
        pin: &InPin,
        _style: &egui::Style,
        snarl: &mut Snarl<WorkGraphNode>,
    ) -> egui::Color32 {
        match snarl.get_node(pin.id.node) {
            WorkGraphNode::Job(job) => {
                let (id, _) = self.workgraph.inputs(*job).nth(pin.id.input).unwrap();
                let target = self.workgraph.input_target(id);

                let [r, g, b] = color_hash(&target);
                egui::Color32::from_rgb(r, g, b)
            }
        }
    }

    fn output_color(
        &mut self,
        pin: &OutPin,
        _style: &egui::Style,
        snarl: &mut Snarl<WorkGraphNode>,
    ) -> egui::Color32 {
        match snarl.get_node(pin.id.node) {
            WorkGraphNode::Job(job) => {
                let (id, _) = self.workgraph.outputs(*job).nth(pin.id.output).unwrap();
                let target = self.workgraph.output_target(id);

                let [r, g, b] = color_hash(&target);
                egui::Color32::from_rgb(r, g, b)
            }
        }
    }
}
