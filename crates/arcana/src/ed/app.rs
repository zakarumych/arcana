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

use super::{console::Console, game::Games, plugins::Plugins, AppTab};

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

#[repr(transparent)]
struct AppModel {
    world: World,
}

/// Editor view correspond to the open views.
/// Tab belong to a window.
/// Each view knows what to render.
struct Tab(Box<dyn AppTab>);

impl TabViewer for AppModel {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        tab.0.show(&mut self.world, ui);
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.0.title().into()
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

        let window = WindowBuilder::new()
            .with_title("Ed")
            .build(events)
            .map_err(|err| miette::miette!("Failed to Ed window: {err}"))?;

        let (device, queue) = init_mev();
        let mut world = World::new();
        world.insert_resource(project);
        world.insert_resource(Plugins::new());
        world.insert_resource(Games::new());
        world.insert_resource(Console::new(event_collector));

        let mut egui = EguiResource::new();
        egui.add_window(&window, events);

        world.insert_resource(egui);

        let mut graph = RenderGraph::new();

        let target =
            EguiRender::build(&mut graph, window.id(), mev::ClearColor(0.2, 0.2, 0.2, 1.0));
        graph.present(target, window.id());

        Ok(App {
            windows: vec![window],
            tabs: HashMap::new(),
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
                let world = self.model.world.local();
                let mut egui = world.expect_resource_mut::<EguiResource>();
                egui.handle_event(window_id, &event);

                match event {
                    WindowEvent::CloseRequested => {
                        self.tabs.remove(&window_id);
                        let Some(idx) = self.windows.iter().position(|w| w.id() == window_id) else {
                            return Some(Event::WindowEvent { window_id, event });
                        };
                        self.windows.swap_remove(idx);
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

        if self.model.world.get_resource::<Quit>().is_some() {
            self.windows.clear();
        }
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
        self.windows.is_empty()
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
                        tabs.push_to_first_leaf(Tab(Plugins::tab()));
                        ui.close_menu();
                    }
                    if ui.button("Console").clicked() {
                        tabs.push_to_first_leaf(Tab(Console::tab()));
                        ui.close_menu();
                    }
                });
            });
        });
    }
}
