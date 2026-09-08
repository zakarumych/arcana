use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use arcana::{
    Name,
    hash::HashMap,
    id::{ShufIdGen, Stid, hash_id},
    mev,
    model::Value,
    plugin::{JobInfo, Location},
    render::RenderGraphId,
    validate_name,
    work_graph::{Cycle, Edge, HookId, Image2D, JobDesc, JobId, JobIdx, PinId, WorkGraph},
};
use egui::Ui;
use egui_snarl::{
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
    ui::{AnyPins, PinInfo, SnarlStyle, SnarlViewer},
};
use winit::window::WindowId;

use crate::{
    model::ValueProbe,
    project::{Project, ProjectData},
    tool::{Tool, ToolContext, ToolTemplate},
};

use super::{
    hue_hash,
    ide::Ide,
    plugins::Plugins,
    simulation::Simulation,
    ui::{Sampler, Selector, UserTextures},
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RenderGraph {
    pub name: Name,
    pub snarl: Snarl<RenderGraphNode>,

    #[serde(skip)]
    pub modification: u64,
}

impl RenderGraph {
    pub fn new(name: Name) -> Self {
        RenderGraph {
            name,
            snarl: Snarl::new(),
            modification: 1,
        }
    }

    pub fn make_work_graph(&self) -> Result<WorkGraph, Cycle> {
        let jobs = self
            .snarl
            .node_ids()
            .filter_map(|(id, node)| match node {
                RenderGraphNode::Job {
                    job, desc, params, ..
                } => Some((JobIdx(id.0), (*job, desc.clone(), params.clone()))),
                _ => None,
            })
            .collect();

        let edges = self
            .snarl
            .wires()
            .filter_map(|(from, to)| {
                let from = match self.snarl.get_node(from.node) {
                    Some(&RenderGraphNode::Job { .. }) => PinId {
                        job: JobIdx(from.node.0),
                        pin: from.output,
                    },
                    _ => return None,
                };
                let to = match self.snarl.get_node(to.node) {
                    Some(&RenderGraphNode::Job { .. }) => PinId {
                        job: JobIdx(to.node.0),
                        pin: to.input,
                    },
                    _ => return None,
                };
                Some(Edge { from, to })
            })
            .collect();

        WorkGraph::new(jobs, edges)
    }

    pub fn get_present(&self) -> Option<PinId> {
        for (from, to) in self.snarl.wires() {
            if let Some(RenderGraphNode::MainPresent) = self.snarl.get_node(to.node) {
                if let Some(&RenderGraphNode::Job { .. }) = self.snarl.get_node(from.node) {
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

pub struct RenderManager {
    available: BTreeMap<Name, Vec<JobInfo>>,
    renders: HashMap<RenderGraphId, RenderGraph>,
}

impl RenderManager {
    pub fn new() -> Self {
        RenderManager {
            available: BTreeMap::new(),
            renders: HashMap::default(),
        }
    }

    pub fn load(&mut self, _project: &Project, data: &ProjectData) {
        self.renders = data.renders.clone();
    }

    pub fn save(&self, _project: &mut Project, data: &mut ProjectData) {
        data.renders = self.renders.clone();
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        let mut all_jobs = HashMap::default();
        self.available.clear();

        for (name, plugin) in plugins.iter() {
            let jobs = self.available.entry(name).or_default();

            for info in plugin.jobs() {
                all_jobs.insert(info.id, info.clone());
                jobs.push(info.clone());
            }

            jobs.sort_by_key(|node| node.name);
        }

        for render_graph in self.renders.values_mut() {
            let mut add_present_node = true;
            for node in render_graph.snarl.nodes_mut() {
                match node {
                    RenderGraphNode::Job {
                        job,
                        desc,
                        location,
                        active,
                        ..
                    } => {
                        if let Some(info) = all_jobs.get(&*job) {
                            *active = true;

                            if *desc != info.desc {
                                *desc = info.desc.clone();
                            }

                            *location = info.location.clone();
                        }
                    }
                    RenderGraphNode::MainPresent => {
                        add_present_node = false;
                    }
                }
            }

            if add_present_node {
                render_graph
                    .snarl
                    .insert_node(egui::Pos2::new(0.0, 0.0), RenderGraphNode::MainPresent);
            }
            render_graph.modification += 1;
        }
    }

    pub fn get_render_graph(&self, id: RenderGraphId) -> Option<&RenderGraph> {
        self.renders.get(&id)
    }

    pub fn new_render(&mut self, name: Name) -> RenderGraphId {
        let id = hash_id!(name);
        self.renders.insert(id, RenderGraph::new(name));
        id
    }
}

struct Preview {
    image: Option<mev::Image2D>,
    id: egui::TextureId,
    hook: Option<HookId>,
    size: Option<mev::Extent2>,
}

struct NewRenderWidget {
    name: String,
    problem: String,
}

pub struct RenderWidget {
    preview: Option<Rc<RefCell<Preview>>>,
    render: Option<RenderGraphId>,
    new_render: Option<NewRenderWidget>,
}

impl RenderWidget {
    pub fn new() -> Self {
        RenderWidget {
            preview: None,
            render: None,
            new_render: None,
        }
    }

    pub fn show(
        &mut self,
        manager: &mut RenderManager,
        cvt: &mev::kernels::Cvt,
        device: &mev::Device,
        main: &mut Simulation,
        textures: &mut UserTextures,
        ide: Option<&dyn Ide>,
        ui: &mut Ui,
    ) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let selector =
                    Selector::<_, RenderGraph>::new("selected-render-graph", |_, graph| {
                        graph.name.as_str()
                    });

                selector.show(&mut self.render, manager.renders.iter(), ui);

                let mut clear_new_render = false;

                if self.new_render.is_none() {
                    if ui
                        .button(egui_phosphor::regular::PLUS)
                        .on_hover_text("Create new render")
                        .clicked()
                    {
                        self.new_render = Some(NewRenderWidget {
                            name: String::new(),
                            problem: String::new(),
                        });

                        ui.request_repaint();
                    }
                }

                if let Some(new_render) = &mut self.new_render {
                    let edit = egui::TextEdit::singleline(&mut new_render.name)
                        .lock_focus(true)
                        .hint_text("New render name");

                    let r = ui.add(edit);

                    if r.changed() {
                        match validate_name(&new_render.name) {
                            Ok(_) => {
                                new_render.problem.clear();
                            }
                            Err(err) => {
                                new_render.problem = err.to_string();
                            }
                        };
                    }

                    if r.lost_focus() {
                        let (enter_pressed, esc_pressed) = ui.input(|i| {
                            (
                                i.key_pressed(egui::Key::Enter),
                                i.key_pressed(egui::Key::Escape),
                            )
                        });

                        if enter_pressed {
                            match Name::from_str(&new_render.name) {
                                Ok(name) => {
                                    let id = manager.new_render(name);
                                    self.render = Some(id);
                                    clear_new_render = true;
                                }
                                Err(err) => {
                                    new_render.problem = err.to_string();
                                    r.request_focus();
                                }
                            }
                        } else if esc_pressed {
                            clear_new_render = true;
                        } else {
                            r.request_focus();
                        }
                    }

                    if !new_render.problem.is_empty() {
                        ui.weak(
                            egui::RichText::new(&new_render.problem)
                                .color(ui.style().visuals.warn_fg_color),
                        );
                    }
                }

                if clear_new_render {
                    self.new_render = None;
                }
            });

            let Some(render_graph_id) = self.render else {
                return;
            };

            let preview = self.preview.get_or_insert_with(|| {
                let id = textures.new_id();

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
                        if size != image.extent() {
                            preview.image = None;
                        }
                    }

                    let id = preview.id;
                    preview.image.get_or_insert_with(|| {
                        let image = device.new_image(mev::ImageDesc {
                            dims: size,
                            format: mev::PixelFormat::Rgba8Srgb,
                            usage: mev::ImageUsage::TARGET | mev::ImageUsage::SAMPLED,

                            levels: 1,
                            name: "preview",
                        });

                        textures.set(id, image.clone(), Sampler::LinearLinear);

                        image
                    });
                }
            }

            let mut viewer = RenderGraphViewer {
                modified: false,
                available: &mut manager.available,
                main,
                cvt,
                preview,
                ide,
            };

            let style = SnarlStyle {
                wire_style: Some(egui_snarl::ui::WireStyle::AxisAligned { corner_radius: 5.0 }),
                ..SnarlStyle::new()
            };

            let render_graph = manager.renders.get_mut(&render_graph_id).unwrap();

            render_graph
                .snarl
                .show(&mut viewer, &style, "work-graph", ui);

            if viewer.modified {
                render_graph.modification += 1;
            }
        });
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RenderGraphNode {
    Job {
        job: JobId,
        name: Name,
        plugin: Name,
        desc: JobDesc,
        params: HashMap<Name, Value>,

        #[serde(skip)]
        location: Option<Location>,

        #[serde(skip)]
        active: bool,
    },
    MainPresent,
}

pub struct RenderGraphViewer<'a> {
    modified: bool,
    available: &'a mut BTreeMap<Name, Vec<JobInfo>>,
    main: &'a mut Simulation,
    cvt: &'a mev::kernels::Cvt,
    preview: &'a Rc<RefCell<Preview>>,
    ide: Option<&'a dyn Ide>,
}

impl SnarlViewer<RenderGraphNode> for RenderGraphViewer<'_> {
    fn title(&mut self, node: &RenderGraphNode) -> String {
        match *node {
            RenderGraphNode::Job { ref name, .. } => name.as_str().to_owned(),
            RenderGraphNode::MainPresent => "Present".to_owned(),
        }
    }

    fn show_header(
        &mut self,
        id: NodeId,
        _: &[egui_snarl::InPin],
        _: &[egui_snarl::OutPin],
        ui: &mut egui::Ui,
        snarl: &mut egui_snarl::Snarl<RenderGraphNode>,
    ) {
        let mut remove = false;

        match snarl[id] {
            RenderGraphNode::Job {
                ref name,
                ref plugin,
                ref location,
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

                        let r = ui.add_enabled(
                            location.is_some() && self.ide.is_some(),
                            egui::Button::new(egui_phosphor::regular::CODE).small(),
                        );

                        let r = r.on_hover_ui(|ui| {
                            ui.label("Open system in IDE");

                            if self.ide.is_none() {
                                ui.weak("No IDE configured");
                            }

                            if location.is_none() {
                                ui.weak("No location information");
                            }
                        });

                        let r = r.on_disabled_hover_ui(|ui| {
                            ui.label("Open system in IDE");

                            if self.ide.is_none() {
                                ui.weak("No IDE configured");
                            }

                            if location.is_none() {
                                ui.weak("No location information");
                            }
                        });

                        if r.clicked() {
                            let loc = location.as_ref().unwrap();
                            self.ide.unwrap().open(loc.file.as_ref(), Some(loc.line));
                        }
                    });
                });
            }
            RenderGraphNode::MainPresent => {
                ui.label("Present");
            }
        }

        if remove {
            snarl.remove_node(id);
            self.modified = true;
        }
    }

    fn inputs(&mut self, node: &RenderGraphNode) -> usize {
        match *node {
            RenderGraphNode::Job { ref desc, .. } => desc.input_count(),
            RenderGraphNode::MainPresent => 1,
        }
    }

    fn outputs(&mut self, node: &RenderGraphNode) -> usize {
        match *node {
            RenderGraphNode::Job { ref desc, .. } => desc.output_count(),
            RenderGraphNode::MainPresent => 0,
        }
    }

    #[allow(refining_impl_trait)]
    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<RenderGraphNode>,
    ) -> PinInfo {
        match snarl[pin.id.node] {
            RenderGraphNode::Job {
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
                            self.modified |= egui_probe::Probe::new(&mut probe)
                                .with_header(name.as_str())
                                .show(ui)
                                .changed();
                        });
                        PinInfo::square()
                    }
                    a => unreachable!("{a:?}"),
                }
            }
            RenderGraphNode::MainPresent => {
                ui.label("presents");
                PinInfo::circle().with_fill(present_pin_color())
            }
        }
    }

    #[allow(refining_impl_trait)]
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<RenderGraphNode>,
    ) -> PinInfo {
        match snarl[pin.id.node] {
            RenderGraphNode::Job { ref desc, .. } => {
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

                        // show_preview(
                        //     self.main,
                        //     &self.sample,
                        //     r,
                        //     PinId {
                        //         job: JobIdx(pin.id.node.0),
                        //         pin: pin.id.output,
                        //     },
                        //     update.ty,
                        //     &mut self.preview,
                        // );

                        PinInfo::square().with_fill(hue_hash(&update.ty))
                    }
                    (_, Some(create)) => {
                        let create = &desc.creates[create];
                        let r = ui.label("creates");

                        // show_preview(
                        //     self.main,
                        //     &self.sample,
                        //     r,
                        //     PinId {
                        //         job: JobIdx(pin.id.node.0),
                        //         pin: pin.id.output,
                        //     },
                        //     create.ty,
                        //     &mut self.preview,
                        // );
                        PinInfo::triangle().with_fill(hue_hash(&create.ty))
                    }
                    _ => unreachable!(),
                }
            }
            RenderGraphNode::MainPresent => {
                unreachable!()
            }
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<RenderGraphNode>) {
        let from_node = &snarl[from.id.node];
        let to_node = &snarl[to.id.node];
        match (from_node, to_node) {
            (
                RenderGraphNode::Job { desc: from_job, .. },
                RenderGraphNode::Job { desc: to_job, .. },
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
            (RenderGraphNode::Job { desc: from_job, .. }, RenderGraphNode::MainPresent) => {
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

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<RenderGraphNode>) {
        snarl.disconnect(from.id, to.id);
        self.modified = true;
    }

    /// Checks if the snarl has something to show in context menu if wire drag is stopped at `pos`.
    #[inline(always)]
    fn has_dropped_wire_menu(&mut self, _: AnyPins, _: &mut Snarl<RenderGraphNode>) -> bool {
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
        snarl: &mut Snarl<RenderGraphNode>,
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
                        RenderGraphNode::Job {
                            job: job.id,
                            name: job.name,
                            plugin,
                            desc: job.desc.clone(),
                            params: default_params(&job.desc),
                            location: job.location.clone(),
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

                    ui.close();
                    return;
                }
            }
        }
    }

    #[inline(always)]
    fn has_graph_menu(&mut self, _: egui::Pos2, _: &mut Snarl<RenderGraphNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        snarl: &mut Snarl<RenderGraphNode>,
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
                        RenderGraphNode::Job {
                            job: job.id,
                            name: job.name,
                            plugin,
                            desc: job.desc.clone(),
                            params: default_params(&job.desc),
                            location: job.location.clone(),
                            active: true,
                        },
                    );

                    ui.close();
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

fn default_params(desc: &JobDesc) -> HashMap<Name, Value> {
    desc.params
        .iter()
        .map(|(name, model)| (name.clone(), model.default_value()))
        .collect()
}

pub struct RenderTemplate;

impl ToolTemplate for RenderTemplate {
    fn title(&self) -> &str {
        "Render"
    }

    fn key(&self) -> &str {
        "render"
    }

    fn create(
        &self,
        _state: Option<serde_json::Value>,
    ) -> Result<Box<dyn Tool>, serde_json::Error> {
        Ok(Box::new(RenderWidget::new()))
    }
}

impl Tool for RenderWidget {
    fn title(&self) -> &str {
        "Render"
    }

    fn ui(&mut self, ui: &mut egui::Ui, _window: WindowId, context: &mut ToolContext<'_>) {
        self.show(
            context.renders,
            context.cvt,
            context.queue.device(),
            context.simulation,
            &mut context.textures,
            context.ide,
            ui,
        );
    }
}

// fn show_preview(
//     main: &mut Instance,
//     cvt: &mev::kernels::Cvt,
//     r: egui::Response,
//     pin: PinId,
//     ty: Stid,
//     preview: &Rc<RefCell<Preview>>,
// ) {
//     if ty == Image2D::stid() {
//         r.on_hover_ui(|ui| {
//             let mut hook = match preview.borrow().hook {
//                 None => None,
//                 Some(hook) if hook.pin == pin && main.has_work_graph_hook(hook) => Some(hook),
//                 Some(hook) => {
//                     main.remove_work_graph_hook(hook);
//                     None
//                 }
//             };

//             if hook.is_none() {
//                 let preview = preview.clone();

//                 let new_hook =
//                     main.add_work_graph_hook::<Image2D>(pin, move |target, _device, commands| {
//                         let mut target_size = target.extent().expect_2d();
//                         if target_size.width() == 0 || target_size.height() == 0 {
//                             return;
//                         }

//                         if target_size.width() > 128 || target_size.height() > 128 {
//                             if target_size.width() > target_size.height() {
//                                 target_size = mev::Extent2::new(
//                                     128,
//                                     (128 * target_size.height()) / target_size.width(),
//                                 );
//                             } else {
//                                 target_size = mev::Extent2::new(
//                                     (128 * target_size.width()) / target_size.height(),
//                                     128,
//                                 );
//                             }
//                         }

//                         preview.borrow_mut().size = Some(target_size);

//                         let encoder = commands.new_encoder();

//                         if let Some(image) = &preview.borrow().image {
//                             // sample
//                             //     .sample(target.0.clone(), image.clone(), encoder)
//                             //     .unwrap();

//                             cvt.image_to_image_with(
//                                 encoder,
//                                 image.clone(),
//                                 target.0.clone(),
//                                 CvtOptions::default().filter(mev::Filter::Linear),
//                             );
//                         }
//                     });

//                 hook = Some(new_hook);
//             }

//             preview.borrow_mut().hook = hook;
//             if let Some(size) = preview.borrow().size {
//                 ui.image(egui::load::SizedTexture {
//                     id: preview.borrow().id,
//                     size: egui::vec2(size.width() as f32, size.height() as f32),
//                 });
//             }
//         });
//     }
// }
