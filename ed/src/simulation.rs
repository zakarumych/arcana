//! Running instance of the project.

use arcana::{
    Gpu, Name,
    assets::{AssetBuildContext, Assets, Loader},
    ecs::{
        entity::EntityId,
        flow::{Flows, init_flows, wake_flows},
        query::Entities,
        world::World,
    },
    format_name,
    gametime::{ClockRate, FrequencyNumExt, FrequencyTicker, Limiter, Tick, TimeSpan, TimeStamp},
    hash::{HashMap, HashSet},
    id::{ShufIdGen, make_id},
    input::{DeviceId, Input, KeyCode, PhysicalKey, ViewInput},
    mev,
    plugin::{ComponentInfo, PluginsHub},
    render::Renderer,
    tokio::runtime::{Builder as RuntimeBuilder, Runtime},
    work_graph::{CommandStream, HookId, PinId, Target},
};
use egui::Ui;
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    assets::AssetRegistry,
    entity_ids::EntityIds,
    filters::{FilterManager, Funnel},
    render::RenderManager,
    render_view::RenderView,
    systems::SystemManager,
    tool::{Tool, ToolContext, ToolTemplate},
    viewport::ViewId,
};

use super::{
    plugins::Plugins,
    systems::{self, Schedule},
    ui::{Selector, UserTextures},
};

make_id! {
    /// ID of the instance.
    pub InstanceId;
}

struct SimulationView {
    name: Name,

    render: RenderView,

    /// Entity whose Renderer selects this view's render graph.
    renderer: Option<EntityId>,

    // Which window shows this view.
    window: Option<WindowId>,

    /// Is view focused.
    focused: bool,

    // Rect of the widget.
    rect: egui::Rect,

    // Pixel per point of the widget.
    pixel_per_point: f32,

    // Set of devices that have cursor inside the widget area.
    contains_cursors: HashSet<DeviceId>,
}

impl SimulationView {
    fn clear_input(&mut self) {
        self.window = None;
        self.focused = false;
        self.contains_cursors.clear();
    }
}

/// Running instance of the project.
pub struct Simulation {
    /// Own ECS world.
    world: World,

    /// Plugins initialization hub.
    hub: PluginsHub,

    /// Specifies frequency of fixed updates.
    fix: FrequencyTicker,

    /// Limits variable updates.
    limiter: Limiter,

    /// Instance rate.
    rate: ClockRate,

    /// Flows to run on each tick.
    flows: Flows,

    /// Systems schedule.
    schedule: Schedule,
    schedule_modification: u64,

    /// Filters funnel.
    funnel: Funnel,
    funnel_modification: u64,

    /// Instance views.
    views: HashMap<ViewId, SimulationView>,

    default_renderer: Option<EntityId>,
    id_gen: ShufIdGen,
    ids: EntityIds,

    assets: Assets,
    asset_registry: Option<AssetRegistry>,
    asset_revision: u64,
    runtime: Option<Runtime>,

    /// Keeps the originating device alive until world and cached GPU assets are dropped.
    asset_device: Option<mev::Device>,

    /// Keeps plugin code loaded until every plugin-owned value has been dropped.
    plugins: Option<Plugins>,
}

impl Simulation {
    pub fn new(ids: EntityIds) -> Self {
        let mut world = ids.new_world();
        let hub = new_hub();

        let rate = ClockRate::new();
        let fix = FrequencyTicker::new(20.hz(), rate.last_stamp());
        let limiter = Limiter::new(120.hz(), rate.last_stamp());

        let flows = Flows::new();

        let schedule = Schedule::new();
        let funnel = Funnel::new();

        init_world(&mut world);
        let assets = Assets::new([]);
        world.insert_resource(assets.clone());

        Simulation {
            world,
            hub,
            fix,
            limiter,
            rate,
            flows,
            schedule,
            schedule_modification: 0,
            funnel,
            funnel_modification: 0,
            views: HashMap::default(),
            default_renderer: None,
            id_gen: ShufIdGen::new(),
            ids,
            assets,
            asset_registry: None,
            asset_revision: 0,
            runtime: Some(new_runtime()),
            asset_device: None,
            plugins: None,
        }
    }

    /// Connects the project's registry before plugin initialization.
    pub(crate) fn connect_assets(&mut self, registry: AssetRegistry) {
        assert!(
            self.plugins.is_none(),
            "Connect assets before initializing plugins"
        );
        self.asset_revision = registry.revision();
        self.assets =
            Assets::new([std::sync::Arc::new(registry.clone()) as std::sync::Arc<dyn Loader>]);
        self.asset_registry = Some(registry);
        self.world.insert_resource(self.assets.clone());
    }

    pub(crate) fn sync_assets(&mut self) {
        if let Some(registry) = &self.asset_registry {
            let revision = registry.revision();
            if revision != self.asset_revision {
                self.assets.invalidate_all();
                self.asset_revision = revision;
            }
        }
    }

    /// Advances loading and explicit CPU/GPU builds independently of simulation speed.
    pub(crate) fn pump_assets(
        &mut self,
        context: &mut AssetBuildContext,
        gpu: &mut Gpu,
    ) -> Result<(), mev::DeviceError> {
        if let Some(device) = &self.asset_device {
            debug_assert_eq!(
                device, &gpu.device,
                "Asset builds require the originating device"
            );
        } else {
            self.asset_device = Some(gpu.device.clone());
        }
        self.sync_assets();
        for _ in 0..4 {
            self.runtime
                .as_ref()
                .unwrap()
                .block_on(arcana::tokio::task::yield_now());
            if let Err(error) = context.build_assets(&self.assets, gpu) {
                self.assets.invalidate_all();
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        tracing::info!("Updating plugins container");

        self.clear_state();

        self.plugins = Some(plugins.clone());
        let _entered = self.runtime.as_ref().unwrap().enter();
        for (_, p) in plugins.iter() {
            p.init(&mut self.world, &mut self.hub);
        }
    }

    /// Reinitializes the runtime from its active plugins, retaining views and speed.
    pub fn reset(&mut self, textures: &mut UserTextures<'_>) {
        self.clear_textures(textures);
        self.clear_state();
        let _entered = self.runtime.as_ref().unwrap().enter();
        if let Some(plugins) = &self.plugins {
            for (_, plugin) in plugins.iter() {
                plugin.init(&mut self.world, &mut self.hub);
            }
        }
    }

    fn clear_state(&mut self) {
        // The library guard remains alive while plugin-owned state is dropped.
        drop(self.runtime.take());
        self.assets.invalidate_all();
        self.runtime = Some(new_runtime());
        for view in self.views.values_mut() {
            view.render.reset();
            view.renderer = None;
            view.clear_input();
        }
        self.flows = Flows::new();
        self.schedule = Schedule::new();
        self.schedule_modification = 0;
        self.funnel = Funnel::new();
        self.funnel_modification = 0;
        self.world = self.ids.new_world();
        init_world(&mut self.world);
        self.world.insert_resource(self.assets.clone());
        self.hub = new_hub();
        self.default_renderer = None;
        self.rate.reset();
        self.fix = FrequencyTicker::new(20.hz(), self.rate.last_stamp());
        self.limiter = Limiter::new(120.hz(), self.rate.last_stamp());
    }

    pub fn world_and_hub_mut(&mut self) -> (&mut World, &mut PluginsHub) {
        (&mut self.world, &mut self.hub)
    }

    pub fn clear_textures(&mut self, textures: &mut UserTextures<'_>) {
        for view in self.views.values_mut() {
            view.render.clear(textures);
        }
    }

    /// Chooses the renderer inherited by newly created simulation views.
    pub fn set_default_renderer(&mut self, renderer: Option<EntityId>) {
        self.default_renderer = renderer;
    }

    pub fn new_view(&mut self) -> ViewId {
        let id = ViewId::generate(&mut self.id_gen);

        self.views.insert(
            id,
            SimulationView {
                name: format_name!("{id}"),
                render: RenderView::new(),
                renderer: select_renderer(&self.world, self.default_renderer),
                window: None,
                focused: false,
                rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(0.0, 0.0)),
                pixel_per_point: 1.0,
                contains_cursors: HashSet::default(),
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

    pub fn tick(&mut self, systems: &SystemManager, tick: Tick) {
        if self.rate.rate() == 0.0 {
            return;
        }
        let tick = self.rate.advance(tick.span);
        let _entered = self.runtime.as_ref().unwrap().enter();

        systems.update_schedule(&mut self.schedule_modification, &mut self.schedule);

        self.fix.advance_with(tick.span, |fix| {
            self.world.insert_resource(fix);
            self.schedule
                .run(systems::Category::Fix, &mut self.world, &mut self.hub);
        });

        if let Some(tick) = self.limiter.advance(tick.span) {
            self.world.insert_resource(tick);
            self.schedule
                .run(systems::Category::Var, &mut self.world, &mut self.hub);
        }

        wake_flows(&mut self.world);
        self.flows.execute(&mut self.world);

        self.world.run_deferred();
        self.world.execute_received_actions();
    }

    /// Render instance view to a texture.
    fn render_view(
        &mut self,
        id: ViewId,
        queue: &mut mev::Queue,
        renders: &RenderManager,
        textures: &mut UserTextures,
    ) -> Result<(), mev::SurfaceError> {
        let _entered = self.runtime.as_ref().unwrap().enter();
        let Some(view) = self.views.get_mut(&id) else {
            return Ok(());
        };
        view.renderer = select_renderer(&self.world, view.renderer);
        view.render.render(
            &mut self.world,
            &mut self.hub,
            self.plugins.as_ref(),
            view.renderer,
            queue,
            renders,
            textures,
        )
    }

    fn handle_view_event(
        &mut self,
        id: ViewId,
        filters: &FilterManager,
        window: WindowId,
        event: &WindowEvent,
    ) -> bool {
        let _entered = self.runtime.as_ref().unwrap().enter();
        filters.update_funnel(&mut self.funnel_modification, &mut self.funnel);

        let Ok(event) = ViewInput::try_from(event) else {
            return false;
        };

        {
            let Some(view) = self.views.get_mut(&id) else {
                return false;
            };
            if view.window != Some(window) {
                return false;
            }

            match event {
                ViewInput::CursorEntered { .. } => return false,
                ViewInput::CursorLeft { device_id } => {
                    if view.contains_cursors.remove(&device_id) {
                        self.funnel.filter(
                            &mut self.hub,
                            &mut self.world,
                            &Input::ViewInput { input: event },
                        );
                    }
                    return false;
                }
                ViewInput::CursorMoved { device_id, x, y } => {
                    let px = x / view.pixel_per_point;
                    let py = y / view.pixel_per_point;

                    let gx = px - view.rect.min.x;
                    let gy = py - view.rect.min.y;

                    if view.rect.contains(egui::pos2(px, py)) {
                        if view.contains_cursors.insert(device_id) {
                            self.funnel.filter(
                                &mut self.hub,
                                &mut self.world,
                                &Input::ViewInput {
                                    input: ViewInput::CursorEntered { device_id },
                                },
                            );
                        }

                        self.funnel.filter(
                            &mut self.hub,
                            &mut self.world,
                            &Input::ViewInput {
                                input: ViewInput::CursorMoved {
                                    device_id,
                                    x: gx,
                                    y: gy,
                                },
                            },
                        );
                    } else {
                        if view.contains_cursors.remove(&device_id) {
                            self.funnel.filter(
                                &mut self.hub,
                                &mut self.world,
                                &Input::ViewInput {
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
                    self.funnel.filter(
                        &mut self.hub,
                        &mut self.world,
                        &Input::ViewInput { input: event },
                    );

                    return true;
                }

                ViewInput::KeyboardInput { ref event, .. }
                    if view.focused && event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
                {
                    view.focused = false;
                }

                ViewInput::KeyboardInput { .. } if view.focused => {
                    self.funnel.filter(
                        &mut self.hub,
                        &mut self.world,
                        &Input::ViewInput { input: event },
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
            view.render
                .work_graph_mut()
                .map(|graph| graph.add_hook::<T>(pin, hook))
        } else {
            None
        }
    }

    pub fn has_work_graph_hook(&self, view: ViewId, hook: HookId) -> bool {
        self.views
            .get(&view)
            .and_then(|view| view.render.work_graph())
            .is_some_and(|graph| graph.has_hook(hook))
    }

    pub fn remove_work_graph_hook(&mut self, view: ViewId, hook: HookId) {
        if let Some(view) = self.views.get_mut(&view) {
            if let Some(graph) = view.render.work_graph_mut() {
                graph.remove_hook(hook);
            }
        }
    }

    pub fn hub(&self) -> &PluginsHub {
        &self.hub
    }
}

impl Drop for Simulation {
    fn drop(&mut self) {
        // Task cancellation and erased asset destruction require the plugin guard.
        drop(self.runtime.take());
        self.assets.invalidate_all();
    }
}

fn new_runtime() -> Runtime {
    RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create simulation asset runtime")
}

pub struct SimulationWidget {
    view: Option<ViewId>,
    views: Vec<ViewId>,
}

impl SimulationWidget {
    pub fn new() -> Self {
        SimulationWidget {
            view: None,
            views: Vec::new(),
        }
    }

    fn select_view(&mut self, simulation: &mut Simulation, selected: Option<ViewId>) {
        let selected =
            selected.filter(|id| self.views.contains(id) && simulation.views.contains_key(id));
        for id in &self.views {
            if selected != self.view || Some(*id) != selected {
                if let Some(view) = simulation.views.get_mut(id) {
                    view.clear_input();
                }
            }
        }
        self.view = selected;
    }

    pub fn render(
        &mut self,
        simulation: &mut Simulation,
        queue: &mut mev::Queue,
        renders: &RenderManager,
        textures: &mut UserTextures<'_>,
    ) -> Result<(), mev::SurfaceError> {
        let Some(view) = self.view else {
            return Ok(());
        };
        simulation.render_view(view, queue, renders, textures)
    }

    pub fn handle_event(
        &mut self,
        simulation: &mut Simulation,
        filters: &FilterManager,
        window: WindowId,
        event: &WindowEvent,
    ) -> bool {
        let Some(view) = self.view else {
            return false;
        };
        simulation.handle_view_event(view, filters, window, event)
    }

    pub fn show(
        &mut self,
        simulation: &mut Simulation,
        window: WindowId,
        textures: &mut UserTextures,
        ui: &mut Ui,
    ) {
        ui.horizontal(|ui| {
            ui.label("Speed");
            let mut speed = simulation.rate().rate() as f32;
            if ui
                .add(
                    egui::DragValue::new(&mut speed)
                        .range(0.0..=16.0)
                        .speed(0.05)
                        .suffix("×"),
                )
                .on_hover_text("Simulation time scale; 0 pauses updates")
                .changed()
            {
                simulation.rate_mut().set_rate(speed);
            }
            if ui.button("Pause").clicked() {
                simulation.rate_mut().pause();
            }
            if ui.button("1×").on_hover_text("Normal speed").clicked() {
                simulation.rate_mut().set_rate(1.0);
            }
            if ui
                .button("Reset")
                .on_hover_text("Reinitialize active plugins and reset simulation time")
                .clicked()
            {
                simulation.reset(textures);
            }
            ui.label(format!("Time: {}", simulation.rate().last_stamp()));
        });
        let mut selected = self.view;
        self.views.retain(|id| simulation.views.contains_key(id));
        if self.views.is_empty() {
            let view = simulation.new_view();
            self.views.push(view);
            selected = Some(view);
        }
        ui.horizontal_top(|ui| {
            let selector =
                Selector::<_, SimulationView>::new("Simulation view", |_, view| view.name.as_str())
                    .pick_first();
            selector.show(
                &mut selected,
                simulation
                    .views
                    .iter()
                    .filter(|(id, _)| self.views.contains(id)),
                ui,
            );

            if ui
                .button(egui_phosphor::regular::PLUS)
                .on_hover_text("Create new view")
                .clicked()
            {
                let view = simulation.new_view();
                self.views.push(view);
                selected = Some(view);
            }
        });

        self.select_view(simulation, selected);
        let Some(view) = self.view.and_then(|id| simulation.views.get_mut(&id)) else {
            return;
        };

        view.window = Some(window);

        view.renderer = select_renderer(&simulation.world, view.renderer);
        egui::ComboBox::from_id_salt("view-renderer")
            .selected_text(
                view.renderer
                    .map_or_else(|| "No renderer".to_owned(), |id| format!("Renderer {id}")),
            )
            .show_ui(ui, |ui| {
                for renderer in renderer_entities(&simulation.world) {
                    ui.selectable_value(
                        &mut view.renderer,
                        Some(renderer),
                        format!("Renderer {renderer}"),
                    );
                }
            });
        let response = view.render.show(textures, ui);
        view.focused = response.has_focus();
        view.rect = response.rect;
        view.pixel_per_point = ui.ctx().pixels_per_point();
    }
}

impl Tool for SimulationWidget {
    fn title(&self) -> &str {
        "Simulation"
    }

    fn ui(&mut self, ui: &mut egui::Ui, window: WindowId, context: &mut ToolContext<'_>) {
        self.show(context.simulation, window, &mut context.textures, ui);
        if let Err(error) = self.render(
            context.simulation,
            context.queue,
            context.renders,
            &mut context.textures,
        ) {
            ui.colored_label(egui::Color32::RED, error.to_string());
        }
    }

    fn handle_event(
        &mut self,
        window: WindowId,
        event: &WindowEvent,
        context: &mut ToolContext<'_>,
    ) -> bool {
        self.handle_event(context.simulation, context.filters, window, event)
    }

    fn save(&self) -> Option<serde_json::Value> {
        Some(serde_json::Value::Null)
    }

    fn close(&mut self, simulation: &mut Simulation, textures: &mut UserTextures<'_>) {
        for id in self.views.drain(..) {
            if let Some(mut view) = simulation.views.remove(&id) {
                view.render.clear(textures);
            }
        }
        self.view = None;
    }
}

pub struct SimulationTemplate;

impl ToolTemplate for SimulationTemplate {
    fn glyph(&self) -> &str {
        egui_phosphor::regular::PLAY
    }

    fn description(&self) -> &str {
        "View the shared simulation and control its speed or reset it."
    }

    fn key(&self) -> &str {
        "simulation"
    }

    fn title(&self) -> &str {
        "Simulation"
    }

    fn create(
        &self,
        _state: Option<serde_json::Value>,
    ) -> Result<Box<dyn Tool>, serde_json::Error> {
        Ok(Box::new(SimulationWidget::new()))
    }
}

fn new_hub() -> PluginsHub {
    let mut hub = PluginsHub::new();
    arcana::assets::register_builtin_importers(&mut hub);
    arcana::scene::register_components(&mut hub);
    hub.add_component(ComponentInfo::without_default::<Renderer>(arcana::name!(
        Renderer
    )));
    hub
}

fn renderer_entities(world: &World) -> Vec<EntityId> {
    let mut entities = world
        .view::<(Entities, &Renderer)>()
        .iter()
        .map(|(entity, _)| entity.id())
        .collect::<Vec<_>>();
    entities.sort_unstable();
    entities
}

fn select_renderer(world: &World, selected: Option<EntityId>) -> Option<EntityId> {
    let entities = renderer_entities(world);
    selected
        .filter(|id| entities.contains(id))
        .or_else(|| entities.first().copied())
}

fn init_world(world: &mut World) {
    init_flows(world);
    world.insert_resource(Tick {
        now: TimeStamp::start(),
        span: TimeSpan::ZERO,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana::{
        assets::{
            Asset, AssetBuilder, AssetError, AssetId, AssetLoadError, AssetRef, AssetVersion,
        },
        ecs::{
            resources::{Res, ResMut},
            system::{IntoSystem, System},
        },
    };
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::Poll,
    };

    #[derive(Clone, arcana::HasStid, arcana::Component)]
    #[stid = "arcana-ed::test::Bytes"]
    struct Bytes(Vec<u8>);

    impl AssetVersion for Bytes {
        fn version() -> u64 {
            1
        }
    }

    impl Asset for Bytes {
        type Loaded = Vec<u8>;
        async fn load(data: Box<[u8]>, _: &Assets) -> Result<Vec<u8>, AssetError> {
            if &*data == b"bad!" {
                return Err(AssetError::new(std::io::Error::other("invalid bytes")));
            }
            Ok(data.to_vec())
        }
        fn build(data: Vec<u8>, _: &mut AssetBuilder) -> Result<Self, AssetError> {
            Ok(Self(data))
        }
    }

    struct TaskDrop(Arc<AtomicBool>);
    impl Drop for TaskDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn pending_task(simulation: &Simulation) -> Arc<AtomicBool> {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = TaskDrop(dropped.clone());
        simulation.runtime.as_ref().unwrap().spawn(async move {
            let _guard = guard;
            futures::future::pending::<()>().await;
        });
        dropped
    }

    #[test]
    fn assets_resource_and_task_cancellation_follow_reset_reload_and_drop() {
        let mut simulation = Simulation::new(EntityIds::new());
        let importer = arcana::graphics::ImageImporter::id();
        assert!(simulation.hub.importers.contains_key(&importer));
        assert!(simulation.world.get_resource::<Assets>().is_some());
        simulation
            .world
            .insert_resource(String::from("discard on plugin update"));
        let first_task = pending_task(&simulation);
        simulation.update_plugins(&Plugins::default());
        assert!(simulation.hub.importers.contains_key(&importer));
        assert!(simulation.world.get_resource::<String>().is_none());
        assert!(first_task.load(Ordering::SeqCst));
        let reset_task = pending_task(&simulation);
        let mut ui = crate::ui::Ui::new();
        simulation.reset(&mut ui.textures());
        assert!(simulation.hub.importers.contains_key(&importer));
        assert!(reset_task.load(Ordering::SeqCst));
        assert!(simulation.world.get_resource::<Assets>().is_some());
        let reload_task = pending_task(&simulation);
        simulation.update_plugins(&Plugins::default());
        assert!(simulation.hub.importers.contains_key(&importer));
        assert!(reload_task.load(Ordering::SeqCst));
        assert!(simulation.world.get_resource::<Assets>().is_some());
        let final_task = pending_task(&simulation);
        drop(simulation);
        assert!(final_task.load(Ordering::SeqCst));
    }

    #[derive(Clone, arcana::HasStid)]
    #[stid = "arcana-ed::test::UploadedBytes"]
    struct UploadedBytes(mev::Buffer);
    impl AssetVersion for UploadedBytes {
        fn version() -> u64 {
            1
        }
    }
    impl Asset for UploadedBytes {
        type Loaded = Vec<u8>;
        async fn load(data: Box<[u8]>, _: &Assets) -> Result<Vec<u8>, AssetError> {
            Ok(data.to_vec())
        }
        fn build(data: Vec<u8>, builder: &mut AssetBuilder) -> Result<Self, AssetError> {
            let staging = builder.device().new_buffer_init(mev::BufferInitDesc {
                name: "asset bridge test upload",
                data: &data,
                usage: mev::BufferUsage::TRANSFER_SRC | mev::BufferUsage::HOST_WRITE,
            });
            let output = builder.device().new_buffer(mev::BufferDesc {
                name: "asset bridge test GPU buffer",
                size: data.len(),
                usage: mev::BufferUsage::TRANSFER_DST | mev::BufferUsage::TRANSFER_SRC,
            });
            builder
                .encoder()
                .copy()
                .copy_buffer_to_buffer(&staging, &output);
            Ok(Self(output))
        }
    }

    #[derive(Default)]
    struct Requested {
        bytes: Option<Bytes>,
        uploaded: Option<UploadedBytes>,
        pending: bool,
        error: bool,
    }
    const REQUEST_ID: AssetId = arcana::id::static_id!(0xabc123 as AssetId);

    fn request_assets(assets: Res<Assets>, mut requested: ResMut<Requested>) {
        match assets.get::<Bytes>(REQUEST_ID) {
            Poll::Pending => requested.pending = true,
            Poll::Ready(Err(_)) => requested.error = true,
            Poll::Ready(Ok(bytes)) => requested.bytes = Some(bytes),
        }
        if let Poll::Ready(Ok(uploaded)) = assets.get::<UploadedBytes>(REQUEST_ID) {
            requested.uploaded = Some(uploaded);
        }
    }

    #[test]
    #[ignore = "Requires native GPU: project registry, ECS asset requests, upload and paused pump"]
    fn simulation_project_assets_load_build_reimport_and_reset() {
        let dir = std::env::temp_dir().join(format!(
            "arcana-simulation-assets-{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let registry = AssetRegistry::new(dir.join("registry")).unwrap();
        let source = dir.join("source.bin");
        let mut simulation = Simulation::new(EntityIds::new());
        simulation.connect_assets(registry.clone());
        simulation.update_plugins(&Plugins::default());
        let retained = simulation.assets.clone();
        simulation.rate_mut().pause();
        simulation.world.insert_resource(Requested::default());
        let mut system = request_assets.into_system();
        arcana::unfold::register_unfold::<AssetRef<Bytes>>(&mut simulation.world);
        let entity = simulation
            .world
            .spawn((AssetRef::<Bytes>::new(REQUEST_ID),))
            .id();
        let mut unfold = arcana::unfold::unfold_system.into_system();
        let instance = mev::Instance::load().unwrap();
        let (_, mut queues) = instance
            .new_device(mev::DeviceDesc {
                idx: 0,
                queues: &[0],
                features: mev::Features::BUFFER_MAP | mev::Features::BUFFER_MAP_AT_CREATION,
            })
            .unwrap();
        let mut gpu = Gpu::new(queues.pop().unwrap()).unwrap();
        let mut context = AssetBuildContext::new();
        let mut request = |simulation: &mut Simulation| {
            let _entered = simulation.runtime.as_ref().unwrap().enter();
            system.run_alone(&mut simulation.world);
            unfold.run_alone(&mut simulation.world);
        };
        request(&mut simulation);
        assert!(simulation.world.expect_resource::<Requested>().pending);
        assert!(
            simulation
                .world
                .get::<Option<&Bytes>>(entity)
                .unwrap()
                .is_none()
        );
        assert!(
            simulation
                .world
                .get::<Option<&AssetLoadError<Bytes>>>(entity)
                .unwrap()
                .is_none()
        );
        simulation.pump_assets(&mut context, &mut gpu).unwrap();
        context.flush(&mut gpu).unwrap();
        request(&mut simulation);
        assert!(simulation.world.expect_resource::<Requested>().error);
        assert_eq!(
            simulation
                .world
                .get::<&AssetLoadError<Bytes>>(entity)
                .unwrap()
                .id,
            REQUEST_ID
        );
        let unrelated = pending_task(&simulation);

        for (version, bytes) in [(1, b"bad!".as_slice()), (2, b"one!"), (3, b"two!")] {
            std::fs::write(&source, bytes).unwrap();
            registry
                .add_asset_from_file(REQUEST_ID, version, &source)
                .unwrap();
            simulation.sync_assets();
            simulation
                .world
                .insert(entity, AssetRef::<Bytes>::new(REQUEST_ID))
                .unwrap();
            assert!(
                simulation
                    .world
                    .get::<Option<&Bytes>>(entity)
                    .unwrap()
                    .is_none()
            );
            assert!(
                simulation
                    .world
                    .get::<Option<&AssetLoadError<Bytes>>>(entity)
                    .unwrap()
                    .is_none()
            );
            simulation.world.insert_resource(Requested::default());
            request(&mut simulation);
            simulation.pump_assets(&mut context, &mut gpu).unwrap();
            context.flush(&mut gpu).unwrap();
            request(&mut simulation);
            let requested = simulation.world.expect_resource::<Requested>();
            if version == 1 {
                assert!(requested.error);
                drop(requested);
                assert_eq!(
                    simulation
                        .world
                        .get::<&AssetLoadError<Bytes>>(entity)
                        .unwrap()
                        .id,
                    REQUEST_ID
                );
                continue;
            }
            assert_eq!(requested.bytes.as_ref().unwrap().0, bytes);
            let uploaded = requested.uploaded.as_ref().unwrap().0.clone();
            drop(requested);
            assert_eq!(simulation.world.get::<&Bytes>(entity).unwrap().0, bytes);
            assert_eq!(
                simulation.world.get::<&AssetRef<Bytes>>(entity).unwrap().id,
                REQUEST_ID
            );
            assert!(
                simulation
                    .world
                    .get::<Option<&AssetLoadError<Bytes>>>(entity)
                    .unwrap()
                    .is_none()
            );
            let mut buffer = gpu.queue.device().new_buffer(mev::BufferDesc {
                name: "asset bridge test readback",
                size: bytes.len(),
                usage: mev::BufferUsage::TRANSFER_DST | mev::BufferUsage::HOST_READ,
            });
            let mut encoder = gpu.queue.new_command_encoder();
            encoder.barrier(mev::PipelineStages::TRANSFER, mev::PipelineStages::TRANSFER);
            encoder.copy().copy_buffer_to_buffer(&uploaded, &buffer);
            gpu.queue.submit_checkpoint([encoder.finish()]).unwrap();
            gpu.queue.wait_idle().unwrap();
            let mut actual = [0; 4];
            buffer.read(0, &mut actual).unwrap();
            assert_eq!(actual.as_slice(), bytes);
            assert!(matches!(
                retained.get::<Bytes>(REQUEST_ID),
                Poll::Ready(Ok(_))
            ));
        }
        let missing_id = arcana::id::static_id!(0xabc124 as AssetId);
        simulation
            .world
            .get::<&mut AssetRef<Bytes>>(entity)
            .unwrap()
            .id = missing_id;
        request(&mut simulation);
        assert!(
            simulation
                .world
                .get::<Option<&Bytes>>(entity)
                .unwrap()
                .is_none()
        );
        simulation.pump_assets(&mut context, &mut gpu).unwrap();
        request(&mut simulation);
        assert_eq!(
            simulation
                .world
                .get::<&AssetLoadError<Bytes>>(entity)
                .unwrap()
                .id,
            missing_id
        );
        simulation
            .world
            .insert(entity, AssetRef::<Bytes>::new(REQUEST_ID))
            .unwrap();
        assert!(
            simulation
                .world
                .get::<Option<&AssetLoadError<Bytes>>>(entity)
                .unwrap()
                .is_none()
        );
        request(&mut simulation);
        assert_eq!(simulation.world.get::<&Bytes>(entity).unwrap().0, b"two!");
        simulation.world.drop::<AssetRef<Bytes>>(entity).unwrap();
        assert!(
            simulation
                .world
                .get::<Option<&Bytes>>(entity)
                .unwrap()
                .is_none()
        );
        assert!(
            simulation
                .world
                .get::<Option<&AssetLoadError<Bytes>>>(entity)
                .unwrap()
                .is_none()
        );
        assert!(!unrelated.load(Ordering::SeqCst));
        let mut ui = crate::ui::Ui::new();
        simulation.reset(&mut ui.textures());
        assert!(unrelated.load(Ordering::SeqCst));
        assert!(!simulation.world.is_alive(entity));
        arcana::unfold::register_unfold::<AssetRef<Bytes>>(&mut simulation.world);
        let reset_entity = simulation
            .world
            .spawn((AssetRef::<Bytes>::new(REQUEST_ID),))
            .id();
        simulation.world.insert_resource(Requested::default());
        request(&mut simulation);
        simulation.pump_assets(&mut context, &mut gpu).unwrap();
        context.flush(&mut gpu).unwrap();
        request(&mut simulation);
        assert_eq!(
            simulation
                .world
                .expect_resource::<Requested>()
                .bytes
                .as_ref()
                .unwrap()
                .0,
            b"two!"
        );
        assert_eq!(
            simulation.world.get::<&Bytes>(reset_entity).unwrap().0,
            b"two!"
        );
        simulation.update_plugins(&Plugins::default());
        assert!(!simulation.world.is_alive(reset_entity));
        assert!(simulation.world.get_resource::<Assets>().is_some());
        arcana::unfold::register_unfold::<AssetRef<Bytes>>(&mut simulation.world);
        let reload_entity = simulation
            .world
            .spawn((AssetRef::<Bytes>::new(REQUEST_ID),))
            .id();
        simulation.world.insert_resource(Requested::default());
        request(&mut simulation);
        simulation.pump_assets(&mut context, &mut gpu).unwrap();
        request(&mut simulation);
        assert!(
            simulation
                .world
                .expect_resource::<Requested>()
                .uploaded
                .is_some()
        );
        assert_eq!(
            simulation.world.get::<&Bytes>(reload_entity).unwrap().0,
            b"two!"
        );
        context.flush(&mut gpu).unwrap();
        gpu.queue.submit_checkpoint(std::iter::empty()).unwrap();
        gpu.queue.wait_idle().unwrap();
        drop(gpu);
        drop(simulation);
        drop(registry);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reset_discards_runtime_state_and_keeps_views_and_speed() {
        let mut simulation = Simulation::new(EntityIds::new());
        simulation.update_plugins(&Plugins::default());
        let graph = arcana::id::static_id!(1 as arcana::render::RenderGraphId);
        let entity = simulation.world.spawn((Renderer { graph },)).id();
        simulation
            .world
            .insert_resource(String::from("runtime state"));
        simulation.set_default_renderer(Some(entity));
        let view = simulation.new_view();
        simulation.views.get_mut(&view).unwrap().focused = true;
        simulation.rate_mut().set_rate(2.0);
        simulation.tick(
            &SystemManager::new(),
            Tick {
                now: TimeStamp::start(),
                span: TimeSpan::SECOND,
            },
        );
        assert!(simulation.rate().last_stamp() > TimeStamp::start());

        let mut ui = crate::ui::Ui::new();
        simulation.reset(&mut ui.textures());

        assert!(!simulation.world.is_alive(entity));
        assert!(simulation.world.get_resource::<String>().is_none());
        assert_eq!(simulation.rate().last_stamp(), TimeStamp::start());
        assert_eq!(simulation.rate().rate(), 2.0);
        assert_eq!(
            simulation.world.copy_resource::<Tick>().span,
            TimeSpan::ZERO
        );
        assert_eq!(simulation.views.len(), 1);
        assert_eq!(simulation.views[&view].renderer, None);
        assert!(!simulation.views[&view].focused);
        assert!(simulation.plugins.is_some());
        assert_eq!(simulation.schedule_modification, 0);
        assert_eq!(simulation.funnel_modification, 0);
        assert!(
            simulation
                .hub
                .components
                .values()
                .any(|component| component.name == arcana::name!(Renderer))
        );
    }

    #[test]
    fn paused_simulation_does_not_execute_flows_and_resumes_at_selected_speed() {
        let mut simulation = Simulation::new(EntityIds::new());
        simulation
            .world
            .spawn_flow(|world: arcana::ecs::flow::FlowWorld| async move {
                world.insert_resource(String::from("flow ran"));
            });
        let systems = SystemManager::new();
        let step = Tick {
            now: TimeStamp::start(),
            span: TimeSpan::SECOND,
        };
        simulation.rate_mut().pause();
        simulation.tick(&systems, step);
        assert_eq!(simulation.rate().last_stamp(), TimeStamp::start());
        assert!(simulation.world.get_resource::<String>().is_none());
        simulation.rate_mut().set_rate(0.5);
        simulation.tick(&systems, step);
        assert_eq!(
            simulation.rate().last_stamp(),
            TimeStamp::start() + TimeSpan::SECOND / 2
        );
        assert_eq!(&*simulation.world.expect_resource::<String>(), "flow ran");
        simulation
            .world
            .spawn_flow(|world: arcana::ecs::flow::FlowWorld| async move {
                world.insert_resource(String::from("pending flow ran"));
            });
        simulation.rate_mut().pause();
        let mut ui = crate::ui::Ui::new();
        simulation.reset(&mut ui.textures());
        assert_eq!(simulation.rate().rate(), 0.0);
        simulation.rate_mut().set_rate(1.0);
        simulation.tick(&systems, step);
        assert!(simulation.world.get_resource::<String>().is_none());
    }

    #[test]
    fn new_views_inherit_the_selected_renderer_and_removed_selection_falls_back() {
        let mut simulation = Simulation::new(EntityIds::new());
        let graph = arcana::id::static_id!(1 as arcana::render::RenderGraphId);
        let first = simulation.world.spawn((Renderer { graph },)).id();
        let second = simulation.world.spawn((Renderer { graph },)).id();
        simulation.set_default_renderer(Some(second));
        let view = simulation.new_view();
        assert_eq!(simulation.views[&view].renderer, Some(second));
        assert_eq!(
            select_renderer(&simulation.world, Some(second)),
            Some(second)
        );

        simulation
            .world
            .drop::<Renderer>(second)
            .expect("Entity exists");
        assert_eq!(
            select_renderer(&simulation.world, Some(second)),
            Some(first)
        );
        simulation.world.despawn(first).expect("Entity exists");
        assert_eq!(select_renderer(&simulation.world, Some(second)), None);
        let new_view = simulation.new_view();
        assert_eq!(simulation.views[&new_view].renderer, None);
    }

    #[test]
    fn selected_view_alone_receives_pointer_input_and_switching_clears_focus() {
        let mut simulation = Simulation::new(EntityIds::new());
        let first = simulation.new_view();
        let second = simulation.new_view();
        let window = WindowId::from(1);
        let device = winit::event::DeviceId::dummy();
        let mut widget = SimulationWidget::new();
        widget.views.extend([first, second]);
        widget.select_view(&mut simulation, Some(first));
        for view in simulation.views.values_mut() {
            view.window = Some(window);
            view.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
            view.focused = true;
        }
        let event = WindowEvent::CursorMoved {
            device_id: device,
            position: winit::dpi::PhysicalPosition::new(10.0, 10.0),
        };
        let filters = FilterManager::new();
        widget.handle_event(&mut simulation, &filters, window, &event);
        assert!(
            simulation.views[&first]
                .contains_cursors
                .contains(&DeviceId::from(device))
        );
        assert!(simulation.views[&second].contains_cursors.is_empty());

        widget.select_view(&mut simulation, Some(second));
        for view in simulation.views.values() {
            assert!(!view.focused);
            assert!(view.window.is_none());
            assert!(view.contains_cursors.is_empty());
        }
        simulation.views.remove(&second);
        widget.select_view(&mut simulation, Some(second));
        assert!(widget.view.is_none());
        assert!(!widget.handle_event(&mut simulation, &filters, window, &event));
    }

    #[test]
    fn clearing_textures_releases_every_view_texture_id() {
        let mut simulation = Simulation::new(EntityIds::new());
        simulation.new_view();
        simulation.new_view();
        let mut ui = crate::ui::Ui::new();
        for view in simulation.views.values_mut() {
            let context = egui::Context::default();
            let mut output = context.run_ui(egui::RawInput::default(), |egui_ui| {
                view.render.show(&mut ui.textures(), egui_ui);
            });
            output.textures_delta.clear();
        }
        simulation.clear_textures(&mut ui.textures());
        assert!(
            simulation
                .views
                .values()
                .all(|view| view.render.texture_id().is_none())
        );
    }

    #[test]
    fn simulation_tools_own_separate_views_and_close_releases_only_owned_textures() {
        let mut simulation = Simulation::new(EntityIds::new());
        let mut first = SimulationWidget::new();
        let mut second = SimulationWidget::new();
        let mut ui = crate::ui::Ui::new();
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |egui_ui| {
            first.show(
                &mut simulation,
                WindowId::from(1),
                &mut ui.textures(),
                egui_ui,
            );
            second.show(
                &mut simulation,
                WindowId::from(2),
                &mut ui.textures(),
                egui_ui,
            );
        });
        output.textures_delta.clear();
        let first_view = first.view.unwrap();
        let second_view = second.view.unwrap();
        assert_ne!(first_view, second_view);
        assert_eq!(simulation.views.len(), 2);
        assert_eq!(
            simulation.views[&first_view].window,
            Some(WindowId::from(1))
        );
        assert!(simulation.views[&first_view].render.texture_id().is_some());
        assert!(simulation.views[&second_view].render.texture_id().is_some());

        first.select_view(&mut simulation, Some(second_view));
        assert!(first.view.is_none());
        assert_eq!(
            simulation.views[&second_view].window,
            Some(WindowId::from(2))
        );
        first.close(&mut simulation, &mut ui.textures());
        assert!(!simulation.views.contains_key(&first_view));
        assert!(simulation.views[&second_view].render.texture_id().is_some());

        simulation.reset(&mut ui.textures());
        assert_eq!(second.view, Some(second_view));
        assert!(simulation.views[&second_view].render.texture_id().is_none());
        second.close(&mut simulation, &mut ui.textures());
        assert!(simulation.views.is_empty());
    }
}
