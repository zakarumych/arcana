use std::sync::Arc;

use arcana_project::Project;

use egui_dock::{DockState, TabViewer, Tree};
use egui_tracing::EventCollector;
use gametime::Clock;
use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::{
    app::Application,
    blink_alloc::BlinkAlloc,
    edict::World,
    egui::{Context, EguiRender, EguiResource, TopBottomPanel, Ui, WidgetText},
    events::{Event, EventLoop},
    game::Quit,
    init_mev, mev,
    render::{render, RenderGraph, RenderResources},
    winit::{
        dpi,
        event::WindowEvent,
        window::{Window, WindowBuilder, WindowId},
    },
};

use super::{console::Console, game::Games, memory::Memory, plugins::Plugins, Tab};

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    /// Windows opened in the editor.
    windows: Vec<Window>,

    /// Tabs opened in the editor.
    dock_states: HashMap<WindowId, DockState<Tab>>,

    /// Model of the App.
    model: AppModel,

    graph: RenderGraph,
    resources: RenderResources,
    device: mev::Device,
    queue: Arc<Mutex<mev::Queue>>,

    blink: BlinkAlloc,
    clock: Clock,
}

impl Application for App {
    fn on_event(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent { window_id, event } => {
                let mut world = self.model.world.local();

                let event = world
                    .expect_resource_mut::<Games>()
                    .handle_event(window_id, event)?;

                world
                    .expect_resource_mut::<EguiResource>()
                    .handle_event(window_id, &event);

                match event {
                    WindowEvent::CloseRequested => {
                        let Some(idx) = self.windows.iter().position(|w| w.id() == window_id)
                        else {
                            return Some(Event::WindowEvent { window_id, event });
                        };
                        if self.windows.len() == 1 {
                            world.insert_resource(Quit);
                        } else {
                            self.dock_states.remove(&window_id);
                            self.windows.swap_remove(idx);
                        }
                        None
                    }
                    _ => Some(Event::WindowEvent { window_id, event }),
                }
            }
            _ => Some(event),
        }
    }

    fn tick(&mut self, events: &EventLoop) {
        if self.windows.is_empty() {
            return;
        }

        let step = self.clock.step();

        {
            let world = self.model.world.local();
            let mut games = world.get_resource_mut::<Games>().unwrap();
            games.tick(step);
        }

        Plugins::tick(&mut self.model.world);

        let mut egui = self
            .model
            .world
            .remove_resource::<EguiResource>()
            .expect("EguiResource must be present");

        for window in &self.windows {
            let dock_state = self
                .dock_states
                .entry(window.id())
                .or_insert_with(|| DockState::new(vec![]));
            egui.run(window, |cx| {
                let mut menu = Menu {
                    events,
                    device: &self.device,
                    queue: &self.queue,
                };
                menu.show(dock_state.main_surface_mut(), &mut self.model.world, cx);
                egui_dock::DockArea::new(dock_state).show(cx, &mut self.model)
            });
        }

        self.model.world.insert_resource(egui);

        let mut subprocesses = super::SUBPROCESSES.lock();
        subprocesses.retain_mut(|child| match child.try_wait() {
            Ok(Some(_)) => false,
            Err(_) => false,
            _ => true,
        });
    }

    fn render(&mut self) {
        if self.windows.is_empty() {
            return;
        }

        render(
            &mut self.graph,
            &self.device,
            &mut self.queue.lock(),
            &self.blink,
            None,
            self.windows.iter(),
            &self.model.world,
            &mut self.resources,
        );

        {
            let world = self.model.world.local();
            let mut games = world.get_resource_mut::<Games>().unwrap();
            games.show();
        }
    }

    fn should_quit(&self) -> bool {
        self.model.world.get_resource::<Quit>().is_some()
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
struct AppModel {
    world: World,
}

impl Drop for AppModel {
    fn drop(&mut self) {
        // Dirty hack to avoid crash on exit.
        self.world.remove_resource::<Games>();
    }
}

impl TabViewer for AppModel {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match tab {
            Tab::Plugins => Plugins::show(&mut self.world, ui),
            Tab::Console => Console::show(&mut self.world, ui),
            // Tab::Memory => Memory::show(&mut self.world, ui),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab {
            Tab::Plugins => "Plugins".into(),
            Tab::Console => "Console".into(),
            // Tab::Memory => "Memory".into(),
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let state = AppState {
            windows: self
                .windows
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
        let mut world = World::new();
        world.insert_resource(project);
        world.insert_resource(Plugins::new());
        world.insert_resource(Games::new());
        world.insert_resource(Console::new(event_collector));

        let mut egui = EguiResource::new();
        let mut graph = RenderGraph::new();

        let state = load_app_state().unwrap_or_default();

        let mut windows = Vec::new();
        let mut dock_states = HashMap::new();
        for w in state.windows {
            let window = WindowBuilder::new()
                .with_title("Ed")
                .with_position(w.pos)
                .with_inner_size(w.size)
                .build(events)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
                .unwrap();
            egui.add_window(&window, events);
            let target =
                EguiRender::build(&mut graph, window.id(), mev::ClearColor(0.2, 0.2, 0.2, 1.0));
            graph.present(target, window.id());
            dock_states.insert(window.id(), w.dock_state);
            windows.push(window);
        }

        if windows.is_empty() {
            let window = WindowBuilder::new()
                .with_title("Ed")
                .build(events)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
                .unwrap();
            egui.add_window(&window, events);
            let target =
                EguiRender::build(&mut graph, window.id(), mev::ClearColor(0.2, 0.2, 0.2, 1.0));
            graph.present(target, window.id());
            windows.push(window);
        }

        world.insert_resource(egui);

        App {
            windows,
            dock_states,
            model: AppModel { world },
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
    events: &'a EventLoop,
    device: &'a mev::Device,
    queue: &'a Arc<Mutex<mev::Queue>>,
}

impl Menu<'_> {
    fn show(&mut self, tabs: &mut Tree<Tab>, world: &mut World, cx: &Context) {
        TopBottomPanel::top("Menu").show(cx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Exit").clicked() {
                        world.insert_resource(Quit);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Run", |ui| {
                    if ui.button("Launch new game").clicked() {
                        Games::launch(world, self.events, self.device, self.queue);
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Plugins").clicked() {
                        tabs.push_to_first_leaf(Plugins::tab());
                        ui.close_menu();
                    }
                    if ui.button("Console").clicked() {
                        tabs.push_to_first_leaf(Console::tab());
                        ui.close_menu();
                    }
                    // if ui.button("Memory").clicked() {
                    //     tabs.push_to_first_leaf(Memory::tab());
                    //     ui.close_menu();
                    // }
                });
            });
        });
    }
}

fn load_app_state() -> Option<AppState> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    path.push("app_state.json");
    let mut file = std::fs::File::open(path).ok()?;

    serde_json::from_reader(&mut file).ok()
}

fn save_app_state(state: &AppState) -> Option<()> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    path.push("app_state.json");
    let mut file = std::fs::File::create(path).ok()?;
    serde_json::to_writer(&mut file, state).ok()
}
