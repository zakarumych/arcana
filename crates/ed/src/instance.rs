//! Running instance of the project.

use std::sync::Arc;

use arcana::{
    code::{builtin::emit_code_start, init_codes},
    edict::world::WorldLocal,
    events::init_events,
    flow::{init_flows, wake_flows, Flows},
    gametime::{ClockRate, FrequencyNumExt, TimeSpan},
    init_world,
    input::{DeviceId, Input, KeyCode, PhysicalKey, ViewportInput},
    mev,
    plugin::PluginsHub,
    texture::Texture,
    viewport::Viewport,
    work::{CommandStream, HookId, Image2D, Image2DInfo, PinId, Target, WorkGraph},
    Blink, ClockStep, EntityId, FrequencyTicker, World,
};
use egui::Ui;
use hashbrown::{HashMap, HashSet};
use parking_lot::Mutex;
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    code::CodeContext,
    container::Container,
    data::ProjectData,
    filters::Funnel,
    render::Rendering,
    systems::{self, Schedule, Systems},
};

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
    lim: FrequencyTicker,

    /// Instance rate.
    rate: ClockRate,

    /// Codes execution context.
    code: CodeContext,

    /// Flows to run on each tick.
    flows: Flows,

    /// Work graph.
    workgraph: WorkGraph,

    /// Which pin to present to viewport.
    present: Option<PinId>,

    /// Viewport to render into.
    viewport: Viewport,

    /// Container in which plugins reside.
    container: Option<Container>,
}

impl Instance {
    pub fn update_plugins(&mut self, c: &Container) {
        match self.container.take() {
            None => {
                for (_, p) in c.plugins() {
                    p.init(&mut self.world, &mut self.hub);
                }
                self.container = Some(c.clone());
            }
            Some(_old) => {
                self.world = World::new();
                init_world(&mut self.world);

                self.hub = PluginsHub::new();
                self.container = Some(c.clone());

                for (_, p) in c.plugins() {
                    p.init(&mut self.world, &mut self.hub);
                }
            }
        }
    }

    pub fn rate(&self) -> &ClockRate {
        &self.rate
    }

    pub fn rate_mut(&mut self) -> &mut ClockRate {
        &mut self.rate
    }

    pub fn tick(&mut self, span: TimeSpan, schedule: &Schedule, data: &ProjectData) {
        emit_code_start(&mut self.world);

        let last_now = self.rate.now();
        let step = self.rate.step(span);

        self.fix.with_ticks(step.step, |fix_now| {
            self.world.insert_resource(ClockStep {
                now: fix_now,
                step: fix_now - last_now,
            });
            schedule.run(systems::Category::Fix, &mut self.world, &mut self.hub);
        });

        self.world.insert_resource(step);
        if self.lim.tick_count(step.step) > 0 {
            schedule.run(systems::Category::Var, &mut self.world, &mut self.hub);
        }

        self.code.execute(&self.hub, data, &mut self.world);

        wake_flows(&mut self.world);
        self.flows.execute(&mut self.world);

        self.world.run_deferred();
        self.world.execute_received_actions();
    }

    pub fn on_input(&mut self, funnel: &Funnel, event: &Input) -> bool {
        funnel.filter(&mut self.hub, &self.blink, &mut self.world, event)
    }

    pub fn render(
        &mut self,
        device: &mev::Device,
        queue: &mut mev::Queue,
    ) -> Result<(), mev::SurfaceError> {
        let Some(pin) = self.present else {
            return Ok(());
        };

        let (image, frame) =
            match self
                .viewport
                .next_frame(device, queue, mev::PipelineStages::all())?
            {
                Some((image, frame)) => (image, frame),
                None => return Ok(()),
            };

        let info = Image2DInfo::from_image(&image);
        let target = Image2D(image);

        self.workgraph.set_sink(pin, target, info);

        self.workgraph
            .run(device, queue, &mut self.world, &mut self.hub)
            .unwrap();

        if let Some(frame) = frame {
            let mut encoder = queue.new_command_encoder().unwrap();
            encoder.present(frame, mev::PipelineStages::all());
            let buffer = encoder.finish().unwrap();

            queue.submit(std::iter::once(buffer), true).unwrap();
        }

        Ok(())
    }

    /// Makes this instance render into a texture.
    ///
    /// Returns image to which main presentation happens.
    pub fn set_texture(
        &mut self,
        world: &World,
        extent: mev::Extent2,
    ) -> Result<mev::Image, mev::OutOfMemory> {
        #[cold]
        fn new_image(extent: mev::Extent2, world: &World) -> Result<mev::Image, mev::OutOfMemory> {
            let device = world.expect_resource::<mev::Device>();
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

        if self
            .viewport
            .get_image()
            .map_or(true, |i| i.dimensions() != extent)
        {
            tracing::debug!("Creating new image for viewport");
            self.viewport.set_image(new_image(extent, world)?);
        }

        Ok(self.viewport.get_image().unwrap().clone())
    }
}

pub struct Main {
    instance: Instance,
    rendering_modifications: u64,

    focused: bool,
    view_id: Option<EntityId>,

    // Rect of the widget.
    rect: egui::Rect,

    // Pixel per point of the widget.
    pixel_per_point: f32,

    // Set of devices that have cursor inside the widget area.
    contains_cursors: HashSet<DeviceId>,

    // Which window shows this widget.
    window: Option<WindowId>,
}

impl Main {
    pub fn new() -> Self {
        let mut world = World::new();
        let hub = PluginsHub::new();
        let blink = Blink::new();

        let rate = ClockRate::new();
        let fix = FrequencyTicker::new(60.hz(), rate.now());
        let lim = FrequencyTicker::new(60.hz(), rate.now());

        let flows = Flows::new();
        let code = CodeContext::new();
        let workgraph = WorkGraph::new(HashMap::new(), HashSet::new()).unwrap();

        let present = None;
        let viewport = Viewport::new_image();

        init_world(&mut world);

        let instance = Instance {
            world,
            blink,
            hub,
            fix,
            lim,
            rate,
            flows,
            code,
            workgraph,
            present,
            viewport,
            container: None,
        };

        Main {
            instance,
            rendering_modifications: 0,
            focused: false,
            view_id: None,
            rect: egui::Rect::NOTHING,
            pixel_per_point: 1.0,
            contains_cursors: HashSet::new(),
            window: None,
        }
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui, window: WindowId) {
        let mut main = world.get_resource_mut::<Main>().unwrap();

        ui.horizontal_top(|ui| {
            let r = ui.button(egui_phosphor::regular::PLAY);
            if r.clicked() {
                main.instance.rate_mut().set_rate(1.0);
            }
            let r = ui.button(egui_phosphor::regular::PAUSE);
            if r.clicked() {
                main.instance.rate_mut().pause();
            }
            let r = ui.button(egui_phosphor::regular::FAST_FORWARD);
            if r.clicked() {
                main.instance.rate_mut().set_rate(2.0);
            }

            let mut rate = main.instance.rate().rate();

            let value = egui::Slider::new(&mut rate, 0.0..=10.0).clamp_to_range(false);
            let r = ui.add(value);
            if r.changed() {
                main.instance.rate_mut().set_rate(rate as f32);
            }
        });

        let game_frame = egui::Frame::none()
            .rounding(egui::Rounding::same(5.0))
            .stroke(egui::Stroke::new(
                1.0,
                if main.focused {
                    egui::Color32::LIGHT_GRAY
                } else {
                    egui::Color32::DARK_GRAY
                },
            ))
            .inner_margin(egui::Margin::same(10.0));

        game_frame.show(ui, |ui| {
            let size = ui.available_size();
            let extent = mev::Extent2::new(size.x as u32, size.y as u32);
            let Ok(image) = main.instance.set_texture(world, extent) else {
                ui.centered_and_justified(|ui| {
                    ui.label("GPU OOM");
                });
                return;
            };

            let view_id = *main.view_id.get_or_insert_with(|| world.allocate().id());

            world.insert_defer(view_id, Texture { image });

            let image = egui::Image::new(egui::load::SizedTexture {
                id: egui::TextureId::User(view_id.bits()),
                size: size.into(),
            });

            let r = ui.add(image.sense(egui::Sense::click()));

            if main.focused {
                if !r.has_focus() {
                    main.focused = false;
                } else {
                    main.rect = r.rect;
                    main.pixel_per_point = ui.ctx().pixels_per_point();
                }
            } else {
                if r.has_focus() {
                    r.surrender_focus();
                }

                let mut make_focused = false;
                if r.clicked() {
                    r.request_focus();
                    make_focused = !main.focused
                }

                if make_focused {
                    main.rect = r.rect;
                    main.pixel_per_point = ui.ctx().pixels_per_point();
                    main.window = Some(window);
                }
            }
        });
    }

    pub fn handle_event(world: &mut World, window: WindowId, event: &WindowEvent) -> bool {
        let world = world.local();
        let mut main = world.get_resource_mut::<Main>().unwrap();
        let data = world.expect_resource::<ProjectData>();

        if main.window != Some(window) {
            return false;
        }

        if let Ok(event) = ViewportInput::try_from(event) {
            let mut consume = true;

            match event {
                ViewportInput::CursorEntered { .. } => return false,
                ViewportInput::CursorLeft { .. } => return false,
                ViewportInput::CursorMoved { device_id, x, y } => {
                    let px = x / main.pixel_per_point;
                    let py = y / main.pixel_per_point;

                    let gx = px - main.rect.min.x;
                    let gy = py - main.rect.min.y;

                    if main.rect.contains(egui::pos2(px, py)) {
                        if main.contains_cursors.insert(device_id) {
                            main.instance.on_input(
                                &data.funnel,
                                &Input::ViewportInput {
                                    input: ViewportInput::CursorEntered { device_id },
                                },
                            );
                        }

                        main.instance.on_input(
                            &data.funnel,
                            &Input::ViewportInput {
                                input: ViewportInput::CursorMoved {
                                    device_id,
                                    x: gx,
                                    y: gy,
                                },
                            },
                        );
                    } else {
                        consume = false;

                        if main.contains_cursors.remove(&device_id) {
                            main.instance.on_input(
                                &data.funnel,
                                &Input::ViewportInput {
                                    input: ViewportInput::CursorLeft { device_id },
                                },
                            );
                        }
                    }
                }
                ViewportInput::MouseWheel { device_id, .. }
                    if !main.contains_cursors.contains(&device_id) =>
                {
                    consume = false;
                }
                ViewportInput::MouseInput { device_id, .. }
                    if !main.contains_cursors.contains(&device_id) =>
                {
                    consume = false;
                }
                ViewportInput::Resized { .. } | ViewportInput::ScaleFactorChanged { .. } => {
                    consume = false;
                }
                ViewportInput::KeyboardInput { event, .. }
                    if event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
                {
                    main.focused = false;
                }
                ViewportInput::KeyboardInput {
                    device_id, event, ..
                } if main.focused => {
                    main.instance.on_input(
                        &data.funnel,
                        &Input::ViewportInput {
                            input: ViewportInput::KeyboardInput { device_id, event },
                        },
                    );
                }
                _ => {}
            }

            return consume;
        }

        false
    }

    pub fn tick(world: &mut World, step: ClockStep) {
        let world = world.local();
        let mut main = world.expect_resource_mut::<Main>();
        let systems = world.expect_resource::<Systems>();
        let data = world.expect_resource::<ProjectData>();
        main.instance.tick(step.step, systems.schedule(), &data);
    }

    pub fn render(world: &mut World) {
        let world = world.local();
        let mut main = world.expect_resource_mut::<Main>();
        let device = world.expect_resource::<mev::Device>();
        let queue = world.expect_resource_mut::<Arc<Mutex<mev::Queue>>>();
        let rendering = world.expect_resource::<Rendering>();
        let data = world.expect_resource::<ProjectData>();

        if rendering.modification() > main.rendering_modifications {
            match data.workgraph.make_workgraph() {
                Ok(workgraph) => main.instance.workgraph = workgraph,
                Err(err) => {
                    tracing::error!("Failed to make workgraph: {err:?}");
                }
            }
            main.instance.present = data.workgraph.get_present();
            main.rendering_modifications = rendering.modification();
        }

        main.instance.render(&device, &mut queue.lock()).unwrap();
    }

    pub fn update_plugins(&mut self, c: &Container) {
        self.instance.update_plugins(c);
    }

    pub fn add_workgraph_hook<T>(
        &mut self,
        pin: PinId,
        hook: impl FnMut(&T, &mev::Device, &CommandStream) + 'static,
    ) -> HookId
    where
        T: Target,
    {
        self.instance.workgraph.add_hook::<T>(pin, hook)
    }

    pub fn has_workgraph_hook(&mut self, hook: HookId) -> bool {
        self.instance.workgraph.has_hook(hook)
    }

    pub fn remove_workgraph_hook(&mut self, hook: HookId) {
        self.instance.workgraph.remove_hook(hook)
    }
}
