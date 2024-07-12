//! Running instance of the project.

use arcana::{
    code::builtin::emit_code_start,
    edict::{flow::Flows, query::Cpy, view},
    flow::wake_flows,
    gametime::{ClockRate, FrequencyNumExt, TimeSpan, TimeStamp},
    init_world,
    input::{DeviceId, Input, KeyCode, PhysicalKey, ViewInput},
    make_id, mev,
    plugin::PluginsHub,
    render::{RenderGraphId, Renderer},
    viewport::{ViewId, Viewport},
    work::{CommandStream, HookId, Image2D, Image2DInfo, PinId, Target, WorkGraph},
    Blink, ClockStep, EntityId, FrequencyTicker, World,
};
use egui::Ui;
use hashbrown::{HashMap, HashSet};
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    code::CodeContext,
    container::Container,
    data::ProjectData,
    filters::Funnel,
    systems::{self, Schedule, Systems},
    ui::UserTextures,
};

make_id! {
    /// ID of the instance.
    pub InstanceId;
}

struct InstanceView {
    viewport: Viewport,

    /// Chosen renderer.
    renderer: Option<EntityId>,

    /// Current render graph id.
    render_graph: RenderGraphId,

    /// Modification id of the render graph.
    render_modification: u64,

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
                    view.render_modification = 0;
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
        view: ViewId,
        extent: mev::Extent2,
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
                dimensions: extent.into(),
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

        let Some(view) = self.views.get_mut(&view) else {
            // View is not found.
            return Ok(());
        };

        let Some(renderer) = view.renderer else {
            // View does not have a renderer
            return Ok(());
        };

        let Ok(renderer) = self.world.get::<Cpy<Renderer>>(renderer) else {
            // View renderer is not found
            return Ok(());
        };

        let Some(render_graph) = data.render_graphs.get(&renderer.graph) else {
            // View render graph is not found
            return Ok(());
        };

        if renderer.graph != view.render_graph
            || view.render_modification < render_graph.modification
        {
            match render_graph.make_workgraph() {
                Ok(work_graph) => view.work_graph = work_graph,
                Err(err) => {
                    tracing::error!("Failed to make work graph: {err:?}");
                    return Ok(());
                }
            }

            view.present = render_graph.get_present();
        }

        let Some(pin) = view.present else {
            // View does not have a present pin
            return Ok(());
        };

        if view
            .viewport
            .get_image()
            .map_or(true, |i| i.dimensions() != extent)
        {
            let new_image = new_image(extent, queue)?;

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

        view.work_graph
            .run(queue, &mut self.world, &mut self.hub)
            .unwrap();

        if let Some(texture_id) = view.texture_id {
            textures.set(texture_id, image, crate::ui::Sampler::NearestNearest);
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

    pub fn add_workgraph_hook<T>(
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

    pub fn has_workgraph_hook(&self, view: ViewId, hook: HookId) -> bool {
        self.views
            .get(&view)
            .map_or(false, |view| view.work_graph.has_hook(hook))
    }

    pub fn remove_workgraph_hook(&mut self, view: ViewId, hook: HookId) {
        if let Some(view) = self.views.get_mut(&view) {
            view.work_graph.remove_hook(hook);
        }
    }
}

pub struct Simulation {
    viewport: ViewId,
}

impl Simulation {
    pub fn show(
        &mut self,
        instance: &mut Instance,
        window: WindowId,
        textures: &mut UserTextures,
        ui: &mut Ui,
    ) {
        ui.horizontal_top(|ui| {
            let r = ui.button(egui_phosphor::regular::PLAY);
            if r.clicked() {
                instance.rate.set_rate(1.0);
            }
            let r = ui.button(egui_phosphor::regular::PAUSE);
            if r.clicked() {
                instance.rate.pause();
            }
            let r = ui.button(egui_phosphor::regular::FAST_FORWARD);
            if r.clicked() {
                instance.rate.set_rate(2.0);
            }

            let mut rate = instance.rate.rate();

            let value = egui::Slider::new(&mut rate, 0.0..=10.0).clamp_to_range(false);
            let r = ui.add(value);
            if r.changed() {
                instance.rate.set_rate(rate as f32);
            }
        });

        let game_frame = egui::Frame::none()
            .rounding(egui::Rounding::same(5.0))
            .stroke(egui::Stroke::new(
                1.0,
                if instance.focused {
                    egui::Color32::LIGHT_GRAY
                } else {
                    egui::Color32::DARK_GRAY
                },
            ))
            .inner_margin(egui::Margin::same(10.0));

        game_frame.show(ui, |ui| {
            let size = ui.available_size();
            self.view_extent = mev::Extent2::new(size.x as u32, size.y as u32);

            let view_id = *self.view_id.get_or_insert_with(|| textures.new_id());

            let image = egui::Image::new(egui::load::SizedTexture {
                id: view_id,
                size: size.into(),
            });

            let r = ui.add(image.sense(egui::Sense::click()));

            if self.focused {
                if !r.has_focus() {
                    self.focused = false;
                } else {
                    self.rect = r.rect;
                    self.pixel_per_point = ui.ctx().pixels_per_point();
                }
            } else {
                if r.has_focus() {
                    r.surrender_focus();
                }

                let mut make_focused = false;
                if r.clicked() {
                    r.request_focus();
                    make_focused = !self.focused
                }

                if make_focused {
                    self.rect = r.rect;
                    self.pixel_per_point = ui.ctx().pixels_per_point();
                    self.window = Some(window);
                }
            }
        });
    }
}
