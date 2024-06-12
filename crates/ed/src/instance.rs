//! Running instance of the project.

use arcana::{
    code::builtin::emit_code_start,
    flow::{wake_flows, Flows},
    gametime::{ClockRate, FrequencyNumExt, TimeSpan, TimeStamp},
    init_world,
    input::{DeviceId, Input, KeyCode, PhysicalKey, ViewportInput},
    mev,
    plugin::PluginsHub,
    viewport::Viewport,
    work::{CommandStream, HookId, Image2D, Image2DInfo, PinId, Target, WorkGraph},
    Blink, ClockStep, FrequencyTicker, World,
};
use egui::Ui;
use hashbrown::{HashMap, HashSet};
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    code::CodeContext,
    container::Container,
    data::ProjectData,
    filters::Funnel,
    render::Rendering,
    systems::{self, Schedule, Systems},
    ui::UserTextures,
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
    limiter: FrequencyTicker,

    /// Instance rate.
    rate: ClockRate,

    /// Codes execution context.
    code: CodeContext,

    /// Flows to run on each tick.
    flows: Flows,

    /// Work graph.
    workgraph: WorkGraph,

    /// Systems schedule.
    schedule: Schedule,

    /// Which pin to present to viewport.
    present: Option<PinId>,

    /// Viewport to render into.
    viewport: Viewport,

    /// Container in which plugins reside.
    container: Option<Container>,
}

impl Instance {
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
                self.workgraph = WorkGraph::new(HashMap::new(), HashSet::new()).unwrap();
                self.present = None;
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

    pub fn tick(&mut self, span: TimeSpan, data: &ProjectData) {
        // tracing::error!("Tick {}", span);

        emit_code_start(&mut self.world);

        let step = self.rate.step(span);

        self.fix.with_ticks(step.step, |fix| {
            self.world.insert_resource(fix);
            self.schedule
                .run(systems::Category::Fix, &mut self.world, &mut self.hub);
        });

        self.world.insert_resource(step);
        if self.limiter.tick_count(span) > 0 {
            self.schedule
                .run(systems::Category::Var, &mut self.world, &mut self.hub);
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

    /// Render to texture.
    pub fn render_to_texture(
        &mut self,
        extent: mev::Extent2,
        queue: &mut mev::Queue,
    ) -> Result<Option<mev::Image>, mev::SurfaceError> {
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

        let Some(pin) = self.present else {
            return Ok(None);
        };

        if self
            .viewport
            .get_image()
            .map_or(true, |i| i.dimensions() != extent)
        {
            let new_image = new_image(extent, queue)?;

            tracing::debug!("Creating new image for viewport");
            self.viewport.set_image(new_image);
        }

        let image = match self
            .viewport
            .next_frame(queue, mev::PipelineStages::all())?
        {
            Some((image, None)) => image,
            _ => unreachable!(),
        };

        let info = Image2DInfo::from_image(&image);
        let target = Image2D(image.clone());

        self.workgraph.set_sink(pin, target, info);

        self.workgraph
            .run(queue, &mut self.world, &mut self.hub)
            .unwrap();

        Ok(Some(image))
    }
}

pub struct Main {
    instance: Instance,
    rendering_modifications: u64,
    systems_modifications: u64,

    focused: bool,
    view_id: Option<egui::TextureId>,
    view_extent: mev::Extent2,

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
        let fix = FrequencyTicker::new(20.hz(), rate.now());
        let limiter = FrequencyTicker::new(120.hz(), TimeStamp::start());

        let flows = Flows::new();
        let code = CodeContext::new();
        let workgraph = WorkGraph::new(HashMap::new(), HashSet::new()).unwrap();

        let schedule = Schedule::new();

        let present = None;
        let viewport = Viewport::new_image();

        init_world(&mut world);

        let instance = Instance {
            world,
            blink,
            hub,
            fix,
            limiter,
            rate,
            flows,
            code,
            workgraph,
            schedule,
            present,
            viewport,
            container: None,
        };

        Main {
            instance,
            rendering_modifications: 0,
            systems_modifications: 0,
            focused: false,
            view_id: None,
            view_extent: mev::Extent2::ZERO,
            rect: egui::Rect::NOTHING,
            pixel_per_point: 1.0,
            contains_cursors: HashSet::new(),
            window: None,
        }
    }

    pub fn show(&mut self, window: WindowId, textures: &mut UserTextures, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            let r = ui.button(egui_phosphor::regular::PLAY);
            if r.clicked() {
                self.instance.rate_mut().set_rate(1.0);
            }
            let r = ui.button(egui_phosphor::regular::PAUSE);
            if r.clicked() {
                self.instance.rate_mut().pause();
            }
            let r = ui.button(egui_phosphor::regular::FAST_FORWARD);
            if r.clicked() {
                self.instance.rate_mut().set_rate(2.0);
            }

            let mut rate = self.instance.rate().rate();

            let value = egui::Slider::new(&mut rate, 0.0..=10.0).clamp_to_range(false);
            let r = ui.add(value);
            if r.changed() {
                self.instance.rate_mut().set_rate(rate as f32);
            }
        });

        let game_frame = egui::Frame::none()
            .rounding(egui::Rounding::same(5.0))
            .stroke(egui::Stroke::new(
                1.0,
                if self.focused {
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

    pub fn handle_event(
        &mut self,
        data: &ProjectData,
        window: WindowId,
        event: &WindowEvent,
    ) -> bool {
        if self.window != Some(window) {
            return false;
        }

        if let Ok(event) = ViewportInput::try_from(event) {
            let mut consume = true;

            match event {
                ViewportInput::CursorEntered { .. } => return false,
                ViewportInput::CursorLeft { .. } => return false,
                ViewportInput::CursorMoved { device_id, x, y } => {
                    let px = x / self.pixel_per_point;
                    let py = y / self.pixel_per_point;

                    let gx = px - self.rect.min.x;
                    let gy = py - self.rect.min.y;

                    if self.rect.contains(egui::pos2(px, py)) {
                        if self.contains_cursors.insert(device_id) {
                            self.instance.on_input(
                                &data.funnel,
                                &Input::ViewportInput {
                                    input: ViewportInput::CursorEntered { device_id },
                                },
                            );
                        }

                        self.instance.on_input(
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

                        if self.contains_cursors.remove(&device_id) {
                            self.instance.on_input(
                                &data.funnel,
                                &Input::ViewportInput {
                                    input: ViewportInput::CursorLeft { device_id },
                                },
                            );
                        }
                    }
                }
                ViewportInput::MouseWheel { device_id, .. }
                    if !self.contains_cursors.contains(&device_id) =>
                {
                    consume = false;
                }
                ViewportInput::MouseInput { device_id, .. }
                    if !self.contains_cursors.contains(&device_id) =>
                {
                    consume = false;
                }
                ViewportInput::Resized { .. } | ViewportInput::ScaleFactorChanged { .. } => {
                    consume = false;
                }
                ViewportInput::KeyboardInput { event, .. }
                    if event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
                {
                    self.focused = false;
                }
                ViewportInput::KeyboardInput {
                    device_id, event, ..
                } if self.focused => {
                    self.instance.on_input(
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

    pub fn tick(&mut self, data: &ProjectData, systems: &Systems, step: ClockStep) {
        if systems.modification() > self.systems_modifications {
            self.instance.schedule = data.systems.make_schedule();
            self.systems_modifications = systems.modification();
        }

        self.instance.tick(step.step, &data);
    }

    pub fn render(
        &mut self,
        data: &ProjectData,
        rendering: &Rendering,
        textures: &mut UserTextures,
        queue: &mut mev::Queue,
    ) {
        let Some(view_id) = self.view_id else {
            return;
        };

        if rendering.modification() > self.rendering_modifications {
            match data.workgraph.make_workgraph() {
                Ok(workgraph) => self.instance.workgraph = workgraph,
                Err(err) => {
                    tracing::error!("Failed to make workgraph: {err:?}");
                }
            }
            self.instance.present = data.workgraph.get_present();
            self.rendering_modifications = rendering.modification();
        }

        let image = self
            .instance
            .render_to_texture(self.view_extent, queue)
            .unwrap();

        if let Some(image) = image {
            textures.set(view_id, image, crate::ui::Sampler::NearestNearest);
        }
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
