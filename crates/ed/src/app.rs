use std::{path::PathBuf, sync::Arc};

use arcana::{
    blink_alloc::BlinkAlloc,
    edict::world::WorldLocal,
    game::Quit,
    init_mev, mev,
    project::Project,
    render::{render, RenderGraph, RenderResources},
    Clock, Entities, With, World, WorldBuilder,
};
use egui_dock::{DockState, TabViewer, Tree};
use egui_tracing::EventCollector;
use hashbrown::HashMap;
use parking_lot::Mutex;

use arcana_egui::Egui;

use winit::{
    dpi,
    event::WindowEvent,
    window::{Window, WindowBuilder, WindowId},
};

use arcana_egui::{Context, EguiRender, EguiResource, TopBottomPanel, Ui, WidgetText};

use super::{
    console::Console, filters::Filters, games::Games, plugins::Plugins, systems::Systems, Tab,
};

pub enum UserEvent {}

pub type Event<'a> = winit::event::Event<'a, UserEvent>;
pub type EventLoop = winit::event_loop::EventLoop<UserEvent>;
pub type EventLoopWindowTarget = winit::event_loop::EventLoopWindowTarget<UserEvent>;

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    /// Tabs opened in the editor.
    dock_states: HashMap<WindowId, DockState<Tab>>,

    // App state is stored in World.
    world: WorldLocal,

    graph: RenderGraph,
    resources: RenderResources,
    device: mev::Device,
    queue: Arc<Mutex<mev::Queue>>,

    blink: BlinkAlloc,
    clock: Clock,
}

impl App {
    pub fn on_event<'a>(&mut self, event: Event<'a>, events: &EventLoopWindowTarget) {
        match event {
            Event::WindowEvent { window_id, event } => {
                let world = self.world.local();

                let Some(event) = Games::handle_event(world, window_id, event) else {
                    return;
                };

                for (w, egui) in world.view_mut::<(&Window, &mut Egui)>() {
                    if w.id() == window_id {
                        // egui.handle_event(event);
                    }
                }

                match event {
                    WindowEvent::CloseRequested => {
                        let mut drop_windows = Vec::new();
                        for (e, w) in world.view_mut::<(Entities, &Window)>() {
                            if w.id() == window_id {
                                drop_windows.push(e.id());
                            }
                        }
                        for e in drop_windows {
                            let _ = world.despawn(e);
                        }
                    }
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                self.tick(events);
            }
            Event::RedrawEventsCleared => {
                self.render();
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, events: &EventLoopWindowTarget) {
        // Quit if last window was closed.
        if self.world.view_mut::<With<Window>>().into_iter().count() == 0 {
            self.world.insert_resource(Quit);
            return;
        }

        let step = self.clock.step();

        Games::tick(&mut self.world, step);
        Plugins::tick(&mut self.world);

        for (window, egui) in self.world.view::<(&Window, &mut Egui)>() {
            let dock_state = self
                .dock_states
                .entry(window.id())
                .or_insert_with(|| DockState::new(vec![]));

            egui.run(|cx| {
                let mut menu = Menu {
                    events,
                    device: &self.device,
                    queue: &self.queue,
                };
                menu.show(dock_state.main_surface_mut(), &self.world, cx);
                egui_dock::DockArea::new(dock_state).show(cx, &mut AppModel { world: &self.world })
            });
        }

        let mut subprocesses = super::SUBPROCESSES.lock();
        subprocesses.retain_mut(|child| match child.try_wait() {
            Ok(Some(_)) => false,
            Err(_) => false,
            _ => true,
        });
    }

    pub fn render(&mut self) {
        if self.world.view_mut::<With<Window>>().into_iter().count() == 0 {
            return;
        }

        Games::render(&mut self.world);

        render(
            &mut self.graph,
            &self.device,
            &mut self.queue.lock(),
            &self.blink,
            None,
            &mut self.world,
            &mut self.resources,
        );
    }

    pub fn should_quit(&self) -> bool {
        if !self.world.get_resource::<Quit>().is_some() {
            return false;
        }
        let subprocesses = std::mem::take(&mut *super::SUBPROCESSES.lock());
        for mut subprocess in subprocesses {
            subprocess.kill();
            subprocess.wait();
        }
        true
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppWindowState {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    dock_state: DockState<Tab>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct AppState {
    windows: Vec<AppWindowState>,
}

#[repr(transparent)]
struct AppModel<'a> {
    world: &'a WorldLocal,
}

impl TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match *tab {
            Tab::Plugins => Plugins::show(self.world, ui),
            Tab::Console => Console::show(self.world, ui),
            Tab::Systems => Systems::show(self.world, ui),
            Tab::Filters => Filters::show(self.world, ui),
            Tab::Game => Games::show(self.world, ui),
            // Tab::Memory => Memory::show(&mut self.world, ui),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab {
            Tab::Plugins => "Plugins".into(),
            Tab::Console => "Console".into(),
            Tab::Systems => "Systems".into(),
            Tab::Filters => "Filters".into(),
            Tab::Game => "Game".into(),
            // Tab::Memory => "Memory".into(),
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let state = AppState {
            windows: self
                .world
                .view_mut::<&Window>()
                .iter()
                .map(|window| {
                    let scale_factor = window.scale_factor();
                    AppWindowState {
                        pos: window
                            .inner_position()
                            .unwrap_or_default()
                            .to_logical(scale_factor),
                        size: window.inner_size().to_logical(scale_factor),
                        dock_state: self
                            .dock_states
                            .remove(&window.id())
                            .unwrap_or_else(|| DockState::new(vec![])),
                    }
                })
                .collect(),
        };
        let _ = save_app_state(&state);

        let subprocesses = std::mem::take(&mut *super::SUBPROCESSES.lock());
        for mut child in subprocesses {
            let _ = child.kill();
        }
    }
}

impl App {
    pub fn new(events: &EventLoop, event_collector: EventCollector, project: Project) -> Self {
        let (device, queue) = init_mev();

        let mut builder = WorldBuilder::new();
        builder.register_external::<Window>();
        builder.register_component::<Egui>();
        builder.register_external::<mev::Surface>();

        let mut world = builder.build_local();
        world.insert_resource(project);
        world.insert_resource(Plugins::new());
        world.insert_resource(Console::new(event_collector));
        world.insert_resource(EguiResource::new());

        let mut graph = RenderGraph::new();

        let state = load_app_state().unwrap_or_default();

        let mut dock_states = HashMap::new();

        if state.windows.is_empty() {
            let builder = WindowBuilder::new().with_title("Ed");
            let window = builder
                .build(events)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
                .unwrap();

            dock_states.insert(window.id(), DockState::new(vec![]));

            let egui: Egui = Egui::new(&world);
            let id = world.spawn_external((window, egui)).id();

            let target = EguiRender::build(id, mev::ClearColor(0.2, 0.2, 0.2, 1.0), &mut graph);
            graph.present_to(target, id);
        }

        for w in state.windows {
            let builder = WindowBuilder::new()
                .with_title("Ed")
                .with_position(w.pos)
                .with_inner_size(w.size);

            let window = builder
                .build(&events)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
                .unwrap();

            dock_states.insert(window.id(), w.dock_state);

            let egui: Egui = Egui::new(&world);
            let id = world.spawn_external((window, egui)).id();

            let target = EguiRender::build(id, mev::ClearColor(0.2, 0.2, 0.2, 1.0), &mut graph);
            graph.present_to(target, id);
        }

        App {
            dock_states,
            world,
            graph,
            resources: RenderResources::default(),
            device,
            queue: Arc::new(Mutex::new(queue)),
            blink: BlinkAlloc::new(),
            clock: Clock::new(),
        }
    }
}

pub struct Menu<'a> {
    events: &'a EventLoopWindowTarget,
    device: &'a mev::Device,
    queue: &'a Arc<Mutex<mev::Queue>>,
}

impl Menu<'_> {
    fn show(&mut self, tabs: &mut Tree<Tab>, world: &WorldLocal, cx: &Context) {
        TopBottomPanel::top("Menu").show(cx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Exit").clicked() {
                        world.insert_resource_defer(Quit);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Run", |ui| {
                    if ui.button("Launch new game").clicked() {
                        let id = Games::launch(self.events, world, self.device, self.queue, true);
                        if id.is_some() {
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Game").clicked() {
                        tabs.push_to_first_leaf(Games::tab());
                        ui.close_menu();
                    }
                    if ui.button("Plugins").clicked() {
                        tabs.push_to_first_leaf(Plugins::tab());
                        ui.close_menu();
                    }
                    if ui.button("Console").clicked() {
                        tabs.push_to_first_leaf(Console::tab());
                        ui.close_menu();
                    }
                    if ui.button("Systems").clicked() {
                        tabs.push_to_first_leaf(Systems::tab());
                        ui.close_menu();
                    }
                    if ui.button("Filters").clicked() {
                        tabs.push_to_first_leaf(Filters::tab());
                        ui.close_menu();
                    }
                });
            });
        });
    }
}

fn app_state_path() -> Option<PathBuf> {
    let mut path = match dirs::config_dir() {
        None => {
            let mut path = std::env::current_exe().ok()?;
            path.pop();
            path
        }
        Some(path) => path,
    };
    path.push("ed_state.json");
    Some(path)
}

fn load_app_state() -> Option<AppState> {
    let mut file = std::fs::File::open(app_state_path()?).ok()?;

    serde_json::from_reader(&mut file).ok()
}

fn save_app_state(state: &AppState) -> Option<()> {
    let mut file = std::fs::File::create(app_state_path()?).ok()?;
    serde_json::to_writer(&mut file, state).ok()
}
