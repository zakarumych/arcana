//! Running instance of the project.

use arcana::{
    code::{builtin::emit_code_start, init_codes},
    edict::{flow::Flows, query::Cpy},
    events::init_events,
    flow::{init_flows, wake_flows},
    gametime::{ClockRate, FrequencyNumExt, TimeSpan, TimeStamp},
    input::{DeviceId, Input, KeyCode, PhysicalKey, ViewInput},
    make_id, mev,
    plugin::PluginsHub,
    render::{CurrentRenderer, RenderGraphId, Renderer},
    viewport::{ViewId, Viewport},
    work::{CommandStream, HookId, Image2D, Image2DInfo, PinId, Target, WorkGraph},
    Blink, ClockStep, EntityId, FrequencyTicker, IdGen, Name, World,
};
use egui::Ui;
use hashbrown::{HashMap, HashSet};
use winit::{event::WindowEvent, window::WindowId};

use crate::ed::ui::Sampler;

use super::{
    code::CodeContext,
    container::Container,
    data::ProjectData,
    systems::{self, Schedule, Systems},
    ui::{Selector, UserTextures},
};

make_id! {
    /// ID of the instance.
    pub InstanceId;
}

struct InstanceView {
    name: Name,

    viewport: Viewport,

    /// Chosen renderer.
    renderer: Option<EntityId>,

    /// Current render graph id.
    last_render_graph: Option<RenderGraphId>,

    /// Modification id of the render graph.
    last_render_modification: u64,

    /// View work graph.
    work_graph: WorkGraph,

    /// Pin that presents to the view.
    present: Option<PinId>,

    // Which window shows this view.
    window: Option<WindowId>,

    /// Current view extent.
    extent: mev::Extent2,

    /// Which egui texture references this view.
    texture_id: Option<egui::TextureId>,

    /// Is view focused.
    focused: bool,

    // Rect of the widget.
    rect: egui::Rect,

    // Pixel per point of the widget.
    pixel_per_point: f32,

    // Set of devices that have cursor inside the widget area.
    contains_cursors: HashSet<DeviceId>,
}

/// Instance of the project.
pub struct Instance {
    /// Own ECS world.
    world: World,

    blink: Blink,

    /// Plugins initialization hub.
    hub: PluginsHub,

    /// Specifies frequency of fixed updates.
    fix: FrequencyTicker,

    /// Limits variable updates.
    limiter: FrequencyTicker,

    /// Instance rate.
    rate: ClockRate,

    /// Codes execution context.
    code: CodeContext,

    /// Flows to run on each tick.
    flows: Flows,

    /// Container in which plugins reside.
    container: Option<Container>,

    /// Modification id of the systems graph.
    systems_modification: u64,

    /// Systems schedule.
    schedule: Schedule,

    /// Instance views.
    views: HashMap<ViewId, InstanceView>,

    view_id_gen: IdGen,
}

impl Instance {
    pub fn new() -> Self {
        let mut world = World::new();
        let hub = PluginsHub::new();
        let blink = Blink::new();

        let rate = ClockRate::new();
        let fix = FrequencyTicker::new(20.hz(), rate.now());
        let limiter = FrequencyTicker::new(120.hz(), TimeStamp::start());

        let flows = Flows::new();
        let code: CodeContext = CodeContext::new();

        let schedule = Schedule::new();

        init_world(&mut world);

        Instance {
            world,
            blink,
            hub,
            fix,
            limiter,
            rate,
            flows,
            code,
            systems_modification: 0,
            schedule,
            container: None,
            views: HashMap::new(),
            view_id_gen: IdGen::new(),
        }
    }

    pub fn update_plugins(&mut self, new: &Container) {
        tracing::info!("Updating plugins container");

        match self.container.take() {
            None => {
                self.container = Some(new.clone());

                for (_, p) in new.plugins() {
                    p.init(&mut self.world, &mut self.hub);
                }
            }
            Some(old) => {
                self.world = World::new();
                init_world(&mut self.world);

                self.rate.reset();
                self.code.reset();

                for view in self.views.values_mut() {
                    view.work_graph = WorkGraph::new(HashMap::new(), HashSet::new()).unwrap();
                    view.present = None;
                    view.last_render_modification = 0;
                }

                self.hub = PluginsHub::new();
                self.container = Some(new.clone());
                self.blink.reset();
                self.fix = FrequencyTicker::new(20.hz(), self.rate.now());
                self.limiter = FrequencyTicker::new(120.hz(), TimeStamp::start());

                for (_, p) in new.plugins() {
                    p.init(&mut self.world, &mut self.hub);
                }

                drop(old);
            }
        }
    }

    pub fn new_view(&mut self) -> ViewId {
        let id = self.view_id_gen.next();

        self.views.insert(
            id,
            InstanceView {
                name: Name::from_str(&format!("New view {id}")).unwrap(),
                viewport: Viewport::new_image(),
                renderer: None,
                last_render_graph: None,
                last_render_modification: 0,
                work_graph: WorkGraph::new(HashMap::new(), HashSet::new()).unwrap(),
                present: None,
                window: None,
                extent: mev::Extent2::new(0, 0),
                texture_id: None,
                focused: false,
                rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(0.0, 0.0)),
                pixel_per_point: 1.0,
                contains_cursors: HashSet::new(),
            },
        );

        id
    }

    pub fn rate(&self) -> &ClockRate {
        &self.rate
    }

    pub fn rate_mut(&mut self) -> &mut ClockRate {
        &mut self.rate
    }

    pub fn tick(&mut self, data: &ProjectData, systems: &Systems, step: ClockStep) {
        if self.systems_modification < systems.modification() {
            self.schedule = data.systems.make_schedule();
            self.systems_modification = systems.modification();
        }

        emit_code_start(&mut self.world);

        let step = self.rate.step(step.step);

        self.fix.with_ticks(step.step, |fix| {
            self.world.insert_resource(fix);
            self.schedule
                .run(systems::Category::Fix, &mut self.world, &mut self.hub);
        });

        self.world.insert_resource(step);
        if self.limiter.tick_count(step.step) > 0 {
            self.schedule
                .run(systems::Category::Var, &mut self.world, &mut self.hub);
        }

        self.code.execute(&self.hub, data, &mut self.world);

        wake_flows(&mut self.world);
        self.flows.execute(&mut self.world);

        self.world.run_deferred();
        self.world.execute_received_actions();
    }

    /// Render instance view to a texture.
    pub fn render(
        &mut self,
        queue: &mut mev::Queue,
        data: &ProjectData,
        textures: &mut UserTextures,
    ) -> Result<(), mev::SurfaceError> {
        #[cold]
        fn new_image(
            extent: mev::Extent2,
            device: &mev::Device,
        ) -> Result<mev::Image, mev::OutOfMemory> {
            let image = device.new_image(mev::ImageDesc {
                extent: extent.into(),
                format: mev::PixelFormat::Rgba8Srgb,
                usage: mev::ImageUsage::TARGET
                    | mev::ImageUsage::SAMPLED
                    | mev::ImageUsage::STORAGE,
                layers: 1,
                levels: 1,
                name: "Game Viewport",
            })?;
            Ok(image)
        }

        for view in self.views.values_mut() {
            if view.extent.width() == 0 || view.extent.height() == 0 {
                // View has ZERO extent.
                return Ok(());
            }

            let Some(renderer_id) = view.renderer else {
                // View does not have a renderer
                return Ok(());
            };

            let Ok(renderer) = self.world.get::<Cpy<Renderer>>(renderer_id) else {
                // View renderer is not found
                return Ok(());
            };

            let Some(render_graph) = data.render_graphs.get(&renderer.graph) else {
                // View render graph is not found
                return Ok(());
            };

            if view.last_render_graph != Some(renderer.graph)
                || view.last_render_modification < render_graph.modification
            {
                let work_graph = match render_graph.make_work_graph() {
                    Ok(work_graph) => work_graph,
                    Err(err) => {
                        tracing::error!("Failed to make work graph: {err:?}");
                        return Ok(());
                    }
                };

                view.work_graph = work_graph;
                view.present = render_graph.get_present();
                view.last_render_graph = Some(renderer.graph);
            }

            let Some(pin) = view.present else {
                // View does not have a present pin
                return Ok(());
            };

            if view
                .viewport
                .get_image()
                .map_or(true, |i| i.extent() != view.extent)
            {
                let new_image = new_image(view.extent, queue)?;

                tracing::debug!("Creating new image for viewport");
                view.viewport.set_image(new_image);
            }

            let image = match view
                .viewport
                .next_frame(queue, mev::PipelineStages::all())?
            {
                Some((image, None)) => image,
                _ => unreachable!(),
            };

            let info = Image2DInfo::from_image(&image);
            let target = Image2D(image.clone());

            view.work_graph.set_sink(pin, target, info);

            self.world.insert_resource(CurrentRenderer {
                entity: renderer_id,
            });

            view.work_graph
                .run(queue, &mut self.world, &mut self.hub)
                .unwrap();

            if let Some(texture_id) = view.texture_id {
                textures.set(texture_id, image, Sampler::NearestNearest);
            }
        }

        Ok(())
    }

    pub fn handle_event(
        &mut self,
        data: &ProjectData,
        window: WindowId,
        event: &WindowEvent,
    ) -> bool {
        let Ok(event) = ViewInput::try_from(event) else {
            return false;
        };

        for (&view_id, view) in self.views.iter_mut() {
            if view.window != Some(window) {
                continue;
            }

            match event {
                ViewInput::CursorEntered { .. } => return false,
                ViewInput::CursorLeft { .. } => return false,
                ViewInput::CursorMoved { device_id, x, y } => {
                    let px = x / view.pixel_per_point;
                    let py = y / view.pixel_per_point;

                    let gx = px - view.rect.min.x;
                    let gy = py - view.rect.min.y;

                    if view.rect.contains(egui::pos2(px, py)) {
                        if view.contains_cursors.insert(device_id) {
                            data.funnel.filter(
                                &mut self.hub,
                                &self.blink,
                                &mut self.world,
                                &Input::ViewInput {
                                    id: view_id,
                                    input: ViewInput::CursorEntered { device_id },
                                },
                            );
                        }

                        data.funnel.filter(
                            &mut self.hub,
                            &self.blink,
                            &mut self.world,
                            &Input::ViewInput {
                                id: view_id,
                                input: ViewInput::CursorMoved {
                                    device_id,
                                    x: gx,
                                    y: gy,
                                },
                            },
                        );
                    } else {
                        if view.contains_cursors.remove(&device_id) {
                            data.funnel.filter(
                                &mut self.hub,
                                &self.blink,
                                &mut self.world,
                                &Input::ViewInput {
                                    id: view_id,
                                    input: ViewInput::CursorLeft { device_id },
                                },
                            );
                        }
                    }
                }

                ViewInput::Resized { .. } | ViewInput::ScaleFactorChanged { .. } => {}

                ViewInput::MouseInput { device_id, .. }
                | ViewInput::MouseWheel { device_id, .. }
                    if view.focused && view.contains_cursors.contains(&device_id) =>
                {
                    data.funnel.filter(
                        &mut self.hub,
                        &self.blink,
                        &mut self.world,
                        &Input::ViewInput {
                            id: view_id,
                            input: event,
                        },
                    );

                    return true;
                }

                ViewInput::KeyboardInput { ref event, .. }
                    if view.focused && event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
                {
                    view.focused = false;
                }

                ViewInput::KeyboardInput { .. } if view.focused => {
                    data.funnel.filter(
                        &mut self.hub,
                        &self.blink,
                        &mut self.world,
                        &Input::ViewInput {
                            id: view_id,
                            input: event,
                        },
                    );

                    return true;
                }
                _ => {}
            }
        }

        false
    }

    pub fn add_work_graph_hook<T>(
        &mut self,
        view: ViewId,
        pin: PinId,
        hook: impl FnMut(&T, &mev::Device, &CommandStream) + 'static,
    ) -> Option<HookId>
    where
        T: Target,
    {
        if let Some(view) = self.views.get_mut(&view) {
            Some(view.work_graph.add_hook::<T>(pin, hook))
        } else {
            None
        }
    }

    pub fn has_work_graph_hook(&self, view: ViewId, hook: HookId) -> bool {
        self.views
            .get(&view)
            .map_or(false, |view| view.work_graph.has_hook(hook))
    }

    pub fn remove_work_graph_hook(&mut self, view: ViewId, hook: HookId) {
        if let Some(view) = self.views.get_mut(&view) {
            view.work_graph.remove_hook(hook);
        }
    }
}

pub struct Simulation {
    view: Option<ViewId>,
}

impl Simulation {
    pub fn new() -> Self {
        Simulation { view: None }
    }

    pub fn show(
        &mut self,
        instance: &mut Instance,
        window: WindowId,
        textures: &mut UserTextures,
        ui: &mut Ui,
    ) {
        ui.horizontal_top(|ui| {
            let selector =
                Selector::<_, InstanceView>::new("Simulation view", |_, view| view.name.as_str())
                    .pick_first();
            selector.show(&mut self.view, instance.views.iter(), ui);

            if ui
                .button(egui_phosphor::regular::PLUS)
                .on_hover_text("Create new view")
                .clicked()
            {
                self.view = Some(instance.new_view());
            }
        });

        let view = match self.view {
            Some(id) => match instance.views.get_mut(&id) {
                Some(view) => view,
                None => unreachable!(),
            },
            None => return,
        };

        view.window = Some(window);

        let game_frame = egui::Frame::none()
            .rounding(egui::Rounding::same(5.0))
            .stroke(egui::Stroke::new(
                1.0,
                if view.focused {
                    egui::Color32::LIGHT_GRAY
                } else {
                    egui::Color32::DARK_GRAY
                },
            ))
            .inner_margin(egui::Margin::same(10.0));

        game_frame.show(ui, |ui| {
            let size = ui.available_size();
            view.extent = mev::Extent2::new(size.x as u32, size.y as u32);

            let texture_id = *view.texture_id.get_or_insert_with(|| textures.new_id());

            let image = egui::Image::new(egui::load::SizedTexture {
                id: texture_id,
                size: size.into(),
            });

            let r = ui.add(image.sense(egui::Sense::click()));

            if view.focused {
                if !r.has_focus() {
                    view.focused = false;
                } else {
                    view.rect = r.rect;
                    view.pixel_per_point = ui.ctx().pixels_per_point();
                }
            } else {
                if r.has_focus() {
                    r.surrender_focus();
                }

                let mut make_focused = false;
                if r.clicked() {
                    r.request_focus();
                    make_focused = !view.focused
                }

                if make_focused {
                    view.rect = r.rect;
                    view.pixel_per_point = ui.ctx().pixels_per_point();
                }
            }
        });
    }
}

fn init_world(world: &mut World) {
    init_flows(world);
    init_events(world);
    init_codes(world);
    world.insert_resource(ClockStep {
        now: TimeStamp::start(),
        step: TimeSpan::ZERO,
    });
}
