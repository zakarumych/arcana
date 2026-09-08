//! Running instance of the project.

use arcana::{
    Name,
    ecs::{
        entity::EntityId,
        flow::{Flows, init_flows, wake_flows},
        query::Entities,
        world::World,
    },
    format_name,
    gametime::{ClockRate, ClockStep, FrequencyNumExt, FrequencyTicker, TimeSpan, TimeStamp},
    hash::{HashMap, HashSet},
    id::{ShufIdGen, make_id},
    input::{DeviceId, Input, KeyCode, PhysicalKey, ViewInput},
    mev,
    plugin::{ComponentDesc, PluginsHub},
    render::Renderer,
    work_graph::{CommandStream, HookId, PinId, Target},
};
use egui::Ui;
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    entity_ids::EntityIds,
    filters::{FilterManager, Funnel},
    render::RenderManager,
    render_view::RenderView,
    systems::SystemManager,
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
    limiter: FrequencyTicker,

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

    /// Keeps plugin code loaded until every plugin-owned value has been dropped.
    plugins: Option<Plugins>,
}

impl Simulation {
    pub fn new(ids: EntityIds) -> Self {
        let mut world = ids.new_world();
        let hub = new_hub();

        let rate = ClockRate::new();
        let fix = FrequencyTicker::new(20.hz(), rate.now());
        let limiter = FrequencyTicker::new(120.hz(), TimeStamp::start());

        let flows = Flows::new();

        let schedule = Schedule::new();
        let funnel = Funnel::new();

        init_world(&mut world);

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
            plugins: None,
        }
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        tracing::info!("Updating plugins container");

        if self.plugins.is_some() {
            self.world = self.ids.new_world();
            init_world(&mut self.world);
            self.hub = new_hub();
            self.flows = Flows::new();
            self.schedule = Schedule::new();
            self.schedule_modification = 0;
            self.funnel = Funnel::new();
            self.funnel_modification = 0;

            self.rate.reset();
            self.fix = FrequencyTicker::new(20.hz(), self.rate.now());
            self.limiter = FrequencyTicker::new(120.hz(), TimeStamp::start());

            for view in self.views.values_mut() {
                view.render.reset();
                view.renderer = None;
                view.clear_input();
            }
        }

        self.plugins = Some(plugins.clone());
        for (_, p) in plugins.iter() {
            p.init(&mut self.world, &mut self.hub);
        }
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
                name: format_name!("New view {id}"),
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

    pub fn tick(&mut self, systems: &SystemManager, step: ClockStep) {
        systems.update_schedule(&mut self.schedule_modification, &mut self.schedule);

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

pub struct SimulationWidget {
    view: Option<ViewId>,
}

impl SimulationWidget {
    pub fn new() -> Self {
        SimulationWidget { view: None }
    }

    fn select_view(&mut self, simulation: &mut Simulation, selected: Option<ViewId>) {
        let selected = selected.filter(|id| simulation.views.contains_key(id));
        for (&id, view) in &mut simulation.views {
            if selected != self.view || Some(id) != selected {
                view.clear_input();
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
        let mut selected = self.view;
        if simulation.views.is_empty() {
            selected = Some(simulation.new_view());
        }
        ui.horizontal_top(|ui| {
            let selector =
                Selector::<_, SimulationView>::new("Simulation view", |_, view| view.name.as_str())
                    .pick_first();
            selector.show(&mut selected, simulation.views.iter(), ui);

            if ui
                .button(egui_phosphor::regular::PLUS)
                .on_hover_text("Create new view")
                .clicked()
            {
                selected = Some(simulation.new_view());
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

fn new_hub() -> PluginsHub {
    let mut hub = PluginsHub::new();
    hub.add_component(ComponentDesc::without_default::<Renderer>(arcana::name!(
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
    world.insert_resource(ClockStep {
        now: TimeStamp::start(),
        step: TimeSpan::ZERO,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
