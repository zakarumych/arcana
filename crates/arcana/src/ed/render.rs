use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use arcana_names::{Ident, Name};
use edict::entity::EntityId;
use egui::Ui;
use egui_snarl::{
    ui::{AnyPins, PinInfo, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::HashMap;

use crate::{
    model::Value,
    plugin::{JobInfo, Location},
    project::Project,
    render::RenderGraphId,
    work::{Edge, HookId, Image2D, JobDesc, JobId, JobIdx, PinId},
    Stid,
};

use super::{
    container::Container,
    data::ProjectData,
    hue_hash,
    ide::Ide,
    instance::Instance,
    model::ValueProbe,
    sample::ImageSample,
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
    pub fn make_work_graph(&self) -> Result<arcana::work::WorkGraph, arcana::work::Cycle> {
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

        arcana::work::WorkGraph::new(jobs, edges)
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

struct Preview {
    image: Option<mev::Image>,
    id: egui::TextureId,
    hook: Option<HookId>,
    size: Option<mev::Extent2>,
}

pub struct Rendering {
    available: BTreeMap<Ident, Vec<JobInfo>>,
    preview: Option<Rc<RefCell<Preview>>>,
    render_graph: Option<RenderGraphId>,
    renderer: Option<EntityId>,
}

impl Rendering {
    pub fn new() -> Self {
        Rendering {
            available: BTreeMap::new(),
            preview: None,
            render_graph: None,
            renderer: None,
        }
    }

    pub fn update_plugins(&mut self, data: &mut ProjectData, container: &Container) {
        let mut all_jobs = HashMap::new();
        self.available.clear();

        for (name, plugin) in container.plugins() {
            let jobs = self.available.entry(name).or_default();

            for info in plugin.jobs() {
                all_jobs.insert(info.id, info.clone());
                jobs.push(info);
            }

            jobs.sort_by_key(|node| node.name);
        }

        for render_graph in data.render_graphs.values_mut() {
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

    pub fn show(
        &mut self,
        project: &Project,
        data: &mut ProjectData,
        sample: &ImageSample,
        device: &mev::Device,
        main: &mut Instance,
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

                selector.show(&mut self.render_graph, data.render_graphs.iter(), ui);
            });

            let Some(render_graph_id) = self.render_graph else {
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
                        if size != image.extent().expect_2d() {
                            preview.image = None;
                        }
                    }

                    let id = preview.id;
                    preview.image.get_or_insert_with(|| {
                        let image = device
                            .new_image(mev::ImageDesc {
                                extent: size.into(),
                                format: mev::PixelFormat::Rgba8Srgb,
                                usage: mev::ImageUsage::TARGET | mev::ImageUsage::SAMPLED,
                                layers: 1,
                                levels: 1,
                                name: "preview",
                            })
                            .unwrap();

                        textures.set(id, image.clone(), Sampler::LinearLinear);

                        image
                    });
                }
            }

            let mut viewer = RenderGraphViewer {
                modified: false,
                available: &mut self.available,
                main,
                sample: &sample,
                preview,
                ide,
            };

            let style = SnarlStyle {
                wire_style: Some(egui_snarl::ui::WireStyle::AxisAligned { corner_radius: 5.0 }),
                ..SnarlStyle::new()
            };

            let render_graph = data.render_graphs.get_mut(&render_graph_id).unwrap();

            render_graph
                .snarl
                .show(&mut viewer, &style, "work-graph", ui);

            if viewer.modified {
                render_graph.modification += 1;
                try_log_err!(data.sync(&project));
            }
        });
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RenderGraphNode {
    Job {
        job: JobId,
        name: Name,
        plugin: Ident,
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
    available: &'a mut BTreeMap<Ident, Vec<JobInfo>>,
    main: &'a mut Instance,
    sample: &'a ImageSample,
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
        _: f32,
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
        _scale: f32,
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
        _scale: f32,
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
        _scale: f32,
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
                            params: job.desc.default_params(),
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

                    ui.close_menu();
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
        _scale: f32,
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
                            params: job.desc.default_params(),
                            location: job.location.clone(),
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

// fn show_preview(
//     main: &mut Instance,
//     sample: &ImageSample,
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
//                 let sample = sample.clone();
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
//                             sample
//                                 .sample(target.0.clone(), image.clone(), encoder)
//                                 .unwrap();
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
