use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use arcana::{
    edict::world::WorldLocal,
    mev,
    model::Value,
    plugin::JobInfo,
    project::Project,
    texture::Texture,
    work::{Edge, HookId, Image2D, JobDesc, JobId, JobIdx, PinId},
    EntityId, Ident, Name, Stid, WithStid,
};
use egui::Ui;
use egui_snarl::{
    ui::{AnyPins, PinInfo, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::HashMap;

use crate::{
    container::Container, data::ProjectData, hue_hash, instance::Main, model::ValueProbe,
    sample::ImageSample,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct WorkGraph {
    snarl: Snarl<WorkGraphNode>,
}

impl WorkGraph {
    pub fn make_workgraph(&self) -> Result<arcana::work::WorkGraph, arcana::work::Cycle> {
        let jobs = self
            .snarl
            .node_ids()
            .filter_map(|(id, node)| match node {
                WorkGraphNode::Job {
                    job, desc, params, ..
                } => Some((JobIdx(id.0), (*job, desc.clone(), dbg!(params.clone())))),
                _ => None,
            })
            .collect();

        let edges = self
            .snarl
            .wires()
            .filter_map(|(from, to)| {
                let from = match self.snarl.get_node(from.node) {
                    Some(&WorkGraphNode::Job { .. }) => PinId {
                        job: JobIdx(from.node.0),
                        pin: from.output,
                    },
                    _ => return None,
                };
                let to = match self.snarl.get_node(to.node) {
                    Some(&WorkGraphNode::Job { .. }) => PinId {
                        job: JobIdx(to.node.0),
                        pin: to.input,
                    },
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
                if let Some(&WorkGraphNode::Job { .. }) = self.snarl.get_node(from.node) {
                    return Some(PinId {
                        job: JobIdx(from.node.0),
                        pin: from.output,
                    });
                }
            }
        }
        None
    }
}

struct Preview {
    image: Option<mev::Image>,
    id: EntityId,
    hook: Option<HookId>,
    size: Option<mev::Extent2>,
}

pub struct Rendering {
    available: BTreeMap<Ident, Vec<JobInfo>>,
    modification: u64,
    preview: Option<Rc<RefCell<Preview>>>,
}

impl Rendering {
    pub fn new() -> Self {
        Rendering {
            available: BTreeMap::new(),
            modification: 1,
            preview: None,
        }
    }

    pub fn modification(&self) -> u64 {
        self.modification
    }

    pub fn update_plugins(&mut self, data: &mut ProjectData, container: &Container) {
        let mut all_jobs = HashMap::new();
        self.available.clear();

        for (name, plugin) in container.plugins() {
            let jobs = self.available.entry(name).or_default();

            for job in plugin.jobs() {
                all_jobs.insert(job.id, job.desc.clone());
                jobs.push(job);
            }

            jobs.sort_by_key(|node| node.name);
        }

        let mut add_present_node = true;
        for node in data.workgraph.snarl.nodes_mut() {
            match node {
                WorkGraphNode::Job {
                    job, desc, active, ..
                } => {
                    if let Some(new_job_desc) = all_jobs.get(&*job) {
                        *active = true;

                        if *desc != *new_job_desc {
                            *desc = new_job_desc.clone();
                        }
                    }
                }
                WorkGraphNode::MainPresent => {
                    add_present_node = false;
                }
            }
        }

        if add_present_node {
            data.workgraph
                .snarl
                .insert_node(egui::Pos2::new(0.0, 0.0), WorkGraphNode::MainPresent);
        }
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut data = world.expect_resource_mut::<ProjectData>();
        let project = world.expect_resource::<Project>();
        let mut rendering = world.expect_resource_mut::<Rendering>();
        let mut main = world.expect_resource_mut::<Main>();
        let sample = world.expect_resource::<ImageSample>();
        let device = world.expect_resource::<mev::Device>();
        let rendering = &mut *rendering;

        let preview = rendering.preview.get_or_insert_with(|| {
            let id = world.allocate().id();

            Rc::new(RefCell::new(Preview {
                image: None,
                id,
                hook: None,
                size: None,
            }))
        });

        {
            let mut preview = preview.borrow_mut();

            if let Some(size) = preview.size {
                if let Some(image) = &preview.image {
                    if size != image.dimensions().expect_2d() {
                        preview.image = None;
                    }
                }

                let id = preview.id;
                preview.image.get_or_insert_with(|| {
                    let image = device
                        .new_image(mev::ImageDesc {
                            dimensions: size.into(),
                            format: mev::PixelFormat::Rgba8Srgb,
                            usage: mev::ImageUsage::TARGET | mev::ImageUsage::SAMPLED,
                            layers: 1,
                            levels: 1,
                            name: "preview",
                        })
                        .unwrap();

                    world.insert_defer(
                        id,
                        Texture {
                            image: image.clone(),
                        },
                    );

                    image
                });
            }
        }

        const STYLE: SnarlStyle = SnarlStyle::new();

        let mut viewer = WorkGraphViewer {
            modified: false,
            available: &mut rendering.available,
            main: &mut main,
            sample: &sample,
            preview,
        };

        data.workgraph
            .snarl
            .show(&mut viewer, &STYLE, "work-graph", ui);

        if viewer.modified {
            rendering.modification += 1;
            try_log_err!(data.sync(&project));
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WorkGraphNode {
    Job {
        job: JobId,
        name: Name,
        plugin: Ident,
        desc: JobDesc,
        params: HashMap<Name, Value>,

        #[serde(skip)]
        active: bool,
    },
    MainPresent,
}

pub struct WorkGraphViewer<'a> {
    modified: bool,
    available: &'a mut BTreeMap<Ident, Vec<JobInfo>>,
    main: &'a mut Main,
    sample: &'a ImageSample,
    preview: &'a Rc<RefCell<Preview>>,
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
        let mut remove = false;

        match snarl[id] {
            WorkGraphNode::Job {
                ref name,
                ref plugin,
                ..
            } => {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(name.as_str());
                        ui.weak(egui_phosphor::regular::AT);
                        ui.label(plugin.as_str());

                        let r = ui.small_button(egui_phosphor::regular::TRASH_SIMPLE);

                        remove = r.clicked();

                        r.on_hover_ui(|ui| {
                            ui.label("Remove job from graph");
                        });
                    });
                });
            }
            WorkGraphNode::MainPresent => {
                ui.label("Present");
            }
        }

        if remove {
            snarl.remove_node(id);
            self.modified = true;
        }
    }

    fn inputs(&mut self, node: &WorkGraphNode) -> usize {
        match *node {
            WorkGraphNode::Job { ref desc, .. } => desc.input_count(),
            WorkGraphNode::MainPresent => 1,
        }
    }

    fn outputs(&mut self, node: &WorkGraphNode) -> usize {
        match *node {
            WorkGraphNode::Job { ref desc, .. } => desc.output_count(),
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
            WorkGraphNode::Job {
                ref desc,
                ref mut params,
                ..
            } => {
                match (
                    desc.update_idx(pin.id.input),
                    desc.read_idx(pin.id.input),
                    desc.param_idx(pin.id.input),
                ) {
                    (Some(update), _, _) => {
                        let update = &desc.updates[update];
                        ui.label("updates");
                        PinInfo::square().with_fill(hue_hash(&update.ty))
                    }
                    (_, Some(read), _) => {
                        let read = &desc.reads[read];
                        ui.label("reads");
                        PinInfo::circle().with_fill(hue_hash(&read.ty))
                    }
                    (_, _, Some(param)) => {
                        let (name, ref model) = desc.params[param];

                        ui.horizontal(|ui| {
                            let value = params.entry(name).or_insert_with(|| model.default_value());

                            let mut probe = ValueProbe::new(Some(model), value, name);
                            self.modified |= egui_probe::Probe::new(name.as_str(), &mut probe)
                                .show(ui)
                                .changed();
                        });
                        PinInfo::square().with_size(0.0)
                    }
                    a => unreachable!("{a:?}"),
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
                if pin.id.output >= desc.output_count() {
                    unreachable!()
                }

                match (
                    desc.update_idx(pin.id.output),
                    desc.create_idx(pin.id.output),
                ) {
                    (Some(update), _) => {
                        let update = &desc.updates[update];
                        let r = ui.label("updates");

                        show_preview(
                            self.main,
                            &self.sample,
                            r,
                            PinId {
                                job: JobIdx(pin.id.node.0),
                                pin: pin.id.output,
                            },
                            update.ty,
                            &mut self.preview,
                        );

                        PinInfo::square().with_fill(hue_hash(&update.ty))
                    }
                    (_, Some(create)) => {
                        let create = &desc.creates[create];
                        let r = ui.label("creates");

                        show_preview(
                            self.main,
                            &self.sample,
                            r,
                            PinId {
                                job: JobIdx(pin.id.node.0),
                                pin: pin.id.output,
                            },
                            create.ty,
                            &mut self.preview,
                        );
                        PinInfo::triangle().with_fill(hue_hash(&create.ty))
                    }
                    _ => unreachable!(),
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

        if self.available.is_empty() {
            ui.separator();
            ui.weak("No available jobs");
            return;
        }

        for (&plugin, jobs) in self.available.iter() {
            if jobs.is_empty() {
                continue;
            }

            ui.separator();
            ui.weak(plugin.as_str());

            for job in jobs {
                if ui.button(job.name.as_str()).clicked() {
                    let new_node = snarl.insert_node(
                        pos,
                        WorkGraphNode::Job {
                            job: job.id,
                            name: job.name,
                            plugin,
                            desc: job.desc.clone(),
                            params: job.desc.default_params(),
                            active: true,
                        },
                    );

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

                    ui.close_menu();
                    return;
                }
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

        if self.available.is_empty() {
            ui.separator();
            ui.weak("No available jobs");
        }

        for (&plugin, jobs) in self.available.iter() {
            if jobs.is_empty() {
                continue;
            }

            ui.separator();
            ui.weak(plugin.as_str());

            for job in jobs {
                if ui.button(job.name.as_str()).clicked() {
                    snarl.insert_node(
                        pos,
                        WorkGraphNode::Job {
                            job: job.id,
                            name: job.name,
                            plugin,
                            desc: job.desc.clone(),
                            params: job.desc.default_params(),
                            active: true,
                        },
                    );

                    ui.close_menu();
                    return;
                }
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

fn show_preview(
    main: &mut Main,
    sample: &ImageSample,
    r: egui::Response,
    pin: PinId,
    ty: Stid,
    preview: &Rc<RefCell<Preview>>,
) {
    if ty == Image2D::stid() {
        r.on_hover_ui(|ui| {
            let mut hook = match preview.borrow().hook {
                None => None,
                Some(hook) if hook.pin == pin && main.has_workgraph_hook(hook) => Some(hook),
                Some(hook) => {
                    main.remove_workgraph_hook(hook);
                    None
                }
            };

            if hook.is_none() {
                let sample = sample.clone();
                let preview = preview.clone();

                let new_hook =
                    main.add_workgraph_hook::<Image2D>(pin, move |target, _device, commands| {
                        let mut target_size = target.dimensions().expect_2d();
                        if target_size.width() == 0 || target_size.height() == 0 {
                            return;
                        }

                        if target_size.width() > 128 || target_size.height() > 128 {
                            if target_size.width() > target_size.height() {
                                target_size = mev::Extent2::new(
                                    128,
                                    (128 * target_size.height()) / target_size.width(),
                                );
                            } else {
                                target_size = mev::Extent2::new(
                                    (128 * target_size.width()) / target_size.height(),
                                    128,
                                );
                            }
                        }

                        preview.borrow_mut().size = Some(target_size);

                        let encoder = commands.new_encoder();

                        if let Some(image) = &preview.borrow().image {
                            sample
                                .sample(target.0.clone(), image.clone(), encoder)
                                .unwrap();
                        }
                    });

                hook = Some(new_hook);
            }

            preview.borrow_mut().hook = hook;
            if let Some(size) = preview.borrow().size {
                ui.image(egui::load::SizedTexture {
                    id: egui::TextureId::User(preview.borrow().id.bits()),
                    size: egui::vec2(size.width() as f32, size.height() as f32),
                });
            }
        });
    }
}
