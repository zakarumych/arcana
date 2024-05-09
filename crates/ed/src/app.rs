use std::{path::PathBuf, sync::Arc};

use arcana::{
    blink_alloc::BlinkAlloc,
    edict::world::WorldLocal,
    events::ViewportEvent,
    gametime::TimeStamp,
    mev,
    project::Project,
    render::{render, RenderGraph, RenderResources},
    viewport::Viewport,
    ClockStep, Entities, IdGen, With, World,
};
use arcana_egui::{Egui, EguiRender, TopBottomPanel, Ui, WidgetText};
use egui::{vec2, Id};
use egui_dock::{DockState, NodeIndex, TabIndex, TabViewer, Tree};
use egui_tracing::EventCollector;
use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{
    dpi,
    event::WindowEvent,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    console::Console, data::ProjectData, filters::Filters, init_mev, instance::Main,
    plugins::Plugins, render::Rendering, systems::Systems,
};

pub enum UserEvent {}

pub type EventLoop = winit::event_loop::EventLoop<UserEvent>;

/// Editor tab.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    Console,
    Systems,
    Filters,
    Rendering,
    Main,
}

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    /// Tabs opened in the editor.
    dock_states: HashMap<WindowId, DockState<Tab>>,

    // App state is stored in World.
    world: World,

    graph: RenderGraph,
    resources: RenderResources,

    blink: BlinkAlloc,

    device: mev::Device,
    queue: Arc<Mutex<mev::Queue>>,

    tab_idgen: IdGen,

    should_quit: bool,
}

impl Drop for App {
    fn drop(&mut self) {
        let state = AppState {
            windows: self
                .world
                .view_mut::<&Viewport>()
                .iter()
                .map(|viewport| {
                    let window = viewport.get_window();
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
                        maximized: window.is_maximized(),
                    }
                })
                .collect(),

            tab_idgen: self.tab_idgen.clone(),
        };
        let _ = save_app_state(&state);

        let subprocesses = std::mem::take(&mut *super::SUBPROCESSES.lock());
        for mut child in subprocesses {
            let _ = child.kill();
        }
    }
}

impl App {
    pub fn new(
        events: &EventLoop,
        event_collector: EventCollector,
        project: Project,
        data: ProjectData,
    ) -> Self {
        let (device, queue) = init_mev();
        let queue = Arc::new(Mutex::new(queue));

        let builder = World::builder();

        let mut world = builder.build();
        world.insert_resource(project);
        world.insert_resource(Plugins::new());
        world.insert_resource(Console::new(event_collector));
        world.insert_resource(Systems::new());
        world.insert_resource(Filters::new());
        world.insert_resource(Rendering::new());
        world.insert_resource(Main::new());
        world.insert_resource(device.clone());
        world.insert_resource(queue.clone());
        world.insert_resource(data);

        let mut graph = RenderGraph::new();

        let state = load_app_state().unwrap_or_default();

        let mut dock_states = HashMap::new();

        if state.windows.is_empty() {
            let builder = Window::default_attributes().with_title("Ed");
            let window = events
                .create_window(builder)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err:?}"))
                .unwrap();

            let size = window.inner_size();
            let scale_factor = window.scale_factor();

            dock_states.insert(window.id(), DockState::new(vec![]));

            let egui = Egui::new(
                vec2(size.width as f32, size.height as f32),
                scale_factor as f32,
            );
            let id = world.spawn((Viewport::new_window(window), egui)).id();

            let target = EguiRender::build(Some(id), &mut graph);
            graph.present_to(target, id);
        }

        for w in state.windows {
            let builder = WindowAttributes::new()
                .with_title("Ed")
                .with_position(w.pos)
                .with_inner_size(w.size);

            let window: Window = events
                .create_window(builder)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err:?}"))
                .unwrap();

            if w.maximized {
                window.set_maximized(true);
            }

            let size = window.inner_size();
            let scale_factor = window.scale_factor();

            dock_states.insert(window.id(), w.dock_state);

            let egui = Egui::new(
                vec2(size.width as f32, size.height as f32),
                scale_factor as f32,
            );
            let id = world.spawn((Viewport::new_window(window), egui)).id();

            let target = EguiRender::build(Some(id), &mut graph);
            graph.present_to(target, id);
        }

        App {
            dock_states,
            world,
            graph,
            resources: RenderResources::default(),
            blink: BlinkAlloc::new(),
            device,
            queue,

            tab_idgen: state.tab_idgen,
            should_quit: false,
        }
    }

    pub fn on_event<'a>(&mut self, window_id: WindowId, event: WindowEvent) {
        let world = self.world.local();

        for (v, egui) in world.view_mut::<(&Viewport, &mut Egui)>() {
            if v.get_window().id() == window_id {
                if let Ok(event) = ViewportEvent::try_from(&event) {
                    if egui.handle_event(&event) {
                        return;
                    }
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                let mut windows_count = 0;
                let mut window_entity = None;
                for (e, v) in world.view_mut::<(Entities, &Viewport)>() {
                    windows_count += 1;
                    if v.get_window().id() == window_id {
                        window_entity = Some(e.id());
                    }
                }
                if let Some(window_entity) = window_entity {
                    if windows_count < 2 {
                        self.should_quit = true;
                    } else {
                        let _ = world.despawn(window_entity);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, step: ClockStep) {
        // Quit if last window was closed.
        if self.world.view_mut::<With<Viewport>>().into_iter().count() == 0 {
            self.should_quit = true;
            return;
        }

        // Update plugins state.
        Plugins::tick(&mut self.world);

        // Simulate main isntance.
        Main::tick(&mut self.world, step);

        let world: &WorldLocal = self.world.local();

        for (viewport, egui) in world.view::<(&Viewport, &mut Egui)>() {
            let window = viewport.get_window();
            let dock_state = self
                .dock_states
                .entry(window.id())
                .or_insert_with(|| DockState::new(vec![]));

            egui.run(step.now, |cx| {
                let tabs = dock_state.main_surface_mut();
                TopBottomPanel::top("Menu").show(cx, |ui| {
                    ui.horizontal(|ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("Exit").clicked() {
                                self.should_quit = true;
                                ui.close_menu();
                            }
                        });
                        ui.menu_button("View", |ui| {
                            if ui.button("Plugins").clicked() {
                                focus_or_add_tab(tabs, Tab::Plugins);
                                ui.close_menu();
                            }
                            if ui.button("Console").clicked() {
                                focus_or_add_tab(tabs, Tab::Console);
                                ui.close_menu();
                            }
                            if ui.button("Systems").clicked() {
                                focus_or_add_tab(tabs, Tab::Systems);
                                ui.close_menu();
                            }
                            if ui.button("Filters").clicked() {
                                focus_or_add_tab(tabs, Tab::Filters);
                                ui.close_menu();
                            }
                            if ui.button("Rendering").clicked() {
                                focus_or_add_tab(tabs, Tab::Rendering);
                                ui.close_menu();
                            }
                            if ui.button("Main").clicked() {
                                focus_or_add_tab(tabs, Tab::Main);
                                ui.close_menu();
                            }
                        });
                    });
                });
                egui_dock::DockArea::new(dock_state).show(cx, &mut AppModel { world, window })
            });
        }

        // Run actions encoded by UI.
        self.world.run_deferred();

        let mut subprocesses = super::SUBPROCESSES.lock();
        subprocesses.retain_mut(|child| match child.try_wait() {
            Ok(Some(_)) => false,
            Err(_) => false,
            _ => true,
        });
    }

    pub fn render(&mut self, now: TimeStamp) {
        Main::render(&mut self.world);

        if self.world.view_mut::<With<Viewport>>().into_iter().count() == 0 {
            return;
        }

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
        if !self.should_quit {
            return false;
        }
        let subprocesses = std::mem::take(&mut *super::SUBPROCESSES.lock());
        for mut subprocess in subprocesses {
            if subprocess.kill().is_ok() {
                let _ = subprocess.wait();
            }
        }
        true
    }
}

fn find_tab(tabs: &Tree<Tab>, tab: Tab) -> Option<(NodeIndex, TabIndex)> {
    for (node_idx, node) in tabs.iter().enumerate() {
        if let Some(tab_idx) = node.iter_tabs().position(|t| *t == tab) {
            return Some((NodeIndex(node_idx), TabIndex(tab_idx)));
        }
    }
    None
}

fn focus_or_add_tab(tabs: &mut Tree<Tab>, tab: Tab) {
    if let Some((node_idx, tab_idx)) = find_tab(tabs, tab) {
        tabs.set_focused_node(node_idx);
        tabs.set_active_tab(node_idx, tab_idx);
    } else {
        tabs.push_to_first_leaf(tab);
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppWindowState {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    dock_state: DockState<Tab>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct AppState {
    windows: Vec<AppWindowState>,
    tab_idgen: IdGen,
}

struct AppModel<'a> {
    world: &'a WorldLocal,
    window: &'a Window,
}

impl TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        Id::new(*tab)
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match *tab {
            Tab::Plugins => Plugins::show(self.world, ui),
            Tab::Console => Console::show(self.world, ui),
            Tab::Systems => Systems::show(self.world, ui),
            Tab::Filters => Filters::show(self.world, ui),
            Tab::Rendering => Rendering::show(self.world, ui),
            Tab::Main => Main::show(self.world, ui, self.window.id()),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match *tab {
            Tab::Plugins => "Plugins".into(),
            Tab::Console => "Console".into(),
            Tab::Systems => "Systems".into(),
            Tab::Filters => "Filters".into(),
            Tab::Rendering => "Rendering".into(),
            Tab::Main => "Main".into(),
        }
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            Tab::Systems => [false, false],
            Tab::Console => [false, false],
            _ => [true, true],
        }
    }
}

fn app_state_path(create: bool) -> Option<PathBuf> {
    let mut path = match dirs::config_dir() {
        None => {
            let mut path = std::env::current_exe().ok()?;
            path.pop();
            path
        }
        Some(mut path) => {
            path.push("Arcana Engine");
            if create {
                std::fs::create_dir_all(&*path).ok()?;
            }
            path
        }
    };
    path.push("ed.bin");
    Some(path)
}

fn load_app_state() -> Option<AppState> {
    let mut file = std::fs::File::open(app_state_path(false)?).ok()?;

    bincode::deserialize_from(&mut file).ok()
}

fn save_app_state(state: &AppState) -> Option<()> {
    let mut file = std::fs::File::create(app_state_path(true)?).ok()?;
    bincode::serialize_into(&mut file, state).ok()
}
