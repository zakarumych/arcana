use std::sync::Arc;

use arcana_project::Project;
use blink_alloc::BlinkAlloc;
use edict::World;
use egui::{Context, Ui};
use egui_dock::{TabViewer, Tree};
use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{Window, WindowBuilder, WindowId},
};

use crate::{
    egui::{EguiRender, EguiResource},
    events::{Event, EventLoop},
    game::Quit,
    init_mev,
    render::{render, RenderGraph, RenderResources},
};

use super::{console::Console, game::Games, plugins::Plugins, Tab};

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    /// Windows opened in the editor.
    windows: Vec<Window>,

    /// Tabs opened in the editor.
    tabs: HashMap<WindowId, Tree<Tab>>,

    /// Model of the App.
    model: AppModel,

    graph: RenderGraph,
    resources: RenderResources,
    device: mev::Device,
    queue: Arc<Mutex<mev::Queue>>,

    blink: BlinkAlloc,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct AppWindowState {
    pos: winit::dpi::LogicalPosition<f64>,
    size: winit::dpi::LogicalSize<f64>,
    tree: Tree<Tab>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct AppState {
    windows: Vec<AppWindowState>,
}

#[repr(transparent)]
struct AppModel {
    world: World,
}

impl TabViewer for AppModel {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match tab {
            Tab::Plugins => {
                Plugins::show(&mut self.world, ui);
            }
            Tab::Console => {
                Console::show(&mut self.world, ui);
            }
        }
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        match tab {
            Tab::Plugins => "Plugins".into(),
            Tab::Console => "Console".into(),
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
                        tree: self.tabs.remove(&window.id()).unwrap_or_default(),
                    }
                })
                .collect(),
        };
        let _ = save_app_state(&state);
    }
}

impl App {
    pub fn new(events: &EventLoop, project: Project) -> miette::Result<Self> {
        let event_collector = egui_tracing::EventCollector::default();

        use tracing_subscriber::layer::SubscriberExt as _;

        if let Err(err) = tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .finish()
                .with(tracing_error::ErrorLayer::default())
                .with(event_collector.clone()),
        ) {
            panic!("Failed to install tracing subscriber: {}", err);
        }

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
        let mut tabs = HashMap::new();
        for w in state.windows {
            let window = WindowBuilder::new()
                .with_title("Ed")
                .with_position(w.pos)
                .with_inner_size(w.size)
                .build(events)
                .map_err(|err| miette::miette!("Failed to Ed window: {err}"))?;
            egui.add_window(&window, events);
            let target =
                EguiRender::build(&mut graph, window.id(), mev::ClearColor(0.2, 0.2, 0.2, 1.0));
            graph.present(target, window.id());
            tabs.insert(window.id(), w.tree);
            windows.push(window);
        }

        if windows.is_empty() {
            let window = WindowBuilder::new()
                .with_title("Ed")
                .build(events)
                .map_err(|err| miette::miette!("Failed to Ed window: {err}"))?;
            egui.add_window(&window, events);
            let target =
                EguiRender::build(&mut graph, window.id(), mev::ClearColor(0.2, 0.2, 0.2, 1.0));
            graph.present(target, window.id());
            windows.push(window);
        }

        world.insert_resource(egui);

        Ok(App {
            windows,
            tabs,
            model: AppModel { world },
            graph,
            resources: RenderResources::default(),
            device,
            queue: Arc::new(Mutex::new(queue)),
            blink: BlinkAlloc::new(),
        })
    }

    pub fn on_event(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent { window_id, event } => {
                let mut world = self.model.world.local();
                world
                    .expect_resource_mut::<EguiResource>()
                    .handle_event(window_id, &event);

                match event {
                    WindowEvent::CloseRequested => {
                        let Some(idx) = self.windows.iter().position(|w| w.id() == window_id) else {
                            return Some(Event::WindowEvent { window_id, event });
                        };
                        if self.windows.len() == 1 {
                            world.insert_resource(Quit);
                        } else {
                            self.tabs.remove(&window_id);
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

    pub fn tick(&mut self, events: &EventLoop) {
        if self.windows.is_empty() {
            return;
        }

        Plugins::tick(&mut self.model.world);

        let mut egui = self
            .model
            .world
            .remove_resource::<EguiResource>()
            .expect("EguiResource must be present");

        for window in &self.windows {
            let tabs = self
                .tabs
                .entry(window.id())
                .or_insert_with(|| Tree::new(vec![]));
            egui.run(window, |cx| {
                Menu.show(tabs, &mut self.model.world, cx);
                egui_dock::DockArea::new(tabs).show(cx, &mut self.model)
            });
        }

        self.model.world.insert_resource(egui);
    }

    pub fn render(&mut self) {
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
    }

    pub fn should_quit(&self) -> bool {
        self.model.world.get_resource::<Quit>().is_some()
    }
}

pub struct Menu;

impl Menu {
    fn show(&mut self, tabs: &mut Tree<Tab>, world: &mut World, cx: &Context) {
        egui::panel::TopBottomPanel::top("Menu").show(cx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Exit").clicked() {
                        world.insert_resource(Quit);
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
