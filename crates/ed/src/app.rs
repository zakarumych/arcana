use std::{path::PathBuf, sync::Arc};

use arcana::{
    blink_alloc::BlinkAlloc,
    edict::world::WorldLocal,
    events::ViewportEvent,
    gametime::TimeStamp,
    make_id, mev,
    project::Project,
    render::{render, RenderGraph, RenderResources},
    viewport::Viewport,
    ClockStep, Entities, IdGen, With, WorldBuilder,
};
use arcana_egui::{Egui, EguiRender, TopBottomPanel, Ui, WidgetText};
use egui::vec2;
use egui_dock::{DockState, TabViewer};
use egui_tracing::EventCollector;
use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{
    dpi,
    event::WindowEvent,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    console::Console,
    data::ProjectData,
    filters::Filters,
    games::{Games, GamesTab},
    plugins::Plugins,
    systems::Systems,
    workgraph::WorkGraph,
};

pub enum UserEvent {}

pub type Event = winit::event::Event<UserEvent>;
pub type EventLoop = winit::event_loop::EventLoop<UserEvent>;
pub type ActiveEventLoop = winit::event_loop::ActiveEventLoop;

/// Editor tab.
#[derive(serde::Serialize, serde::Deserialize)]
enum TabKind {
    Plugins,
    Console,
    Systems,
    Filters,
    WorkGraph,
    Game {
        #[serde(skip)]
        tab: GamesTab,
    },
    // Memory,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Tab {
    kind: TabKind,
    id: egui::Id,
}

make_id!(TabId);

impl Tab {
    pub fn plugins(idgen: &mut IdGen) -> Self {
        Tab {
            kind: TabKind::Plugins,
            id: egui::Id::new(idgen.next::<TabId>()),
        }
    }

    pub fn console(idgen: &mut IdGen) -> Self {
        Tab {
            kind: TabKind::Console,
            id: egui::Id::new(idgen.next::<TabId>()),
        }
    }

    pub fn systems(idgen: &mut IdGen) -> Self {
        Tab {
            kind: TabKind::Systems,
            id: egui::Id::new(idgen.next::<TabId>()),
        }
    }

    pub fn filters(idgen: &mut IdGen) -> Self {
        Tab {
            kind: TabKind::Filters,
            id: egui::Id::new(idgen.next::<TabId>()),
        }
    }

    pub fn workgraph(idgen: &mut IdGen) -> Self {
        Tab {
            kind: TabKind::WorkGraph,
            id: egui::Id::new(idgen.next::<TabId>()),
        }
    }

    pub fn game(idgen: &mut IdGen, tab: GamesTab) -> Self {
        Tab {
            kind: TabKind::Game { tab },
            id: egui::Id::new(idgen.next::<TabId>()),
        }
    }
}

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    /// Tabs opened in the editor.
    dock_states: HashMap<WindowId, DockState<Tab>>,

    // App state is stored in World.
    world: WorldLocal,

    graph: RenderGraph,
    resources: RenderResources,

    blink: BlinkAlloc,

    device: mev::Device,
    queue: Arc<Mutex<mev::Queue>>,

    tab_idgen: IdGen,
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

        let builder = WorldBuilder::new();

        let mut world = builder.build();
        world.insert_resource(project);
        world.insert_resource(Plugins::new());
        world.insert_resource(Console::new(event_collector));
        world.insert_resource(Games::new());
        world.insert_resource(Systems::new());
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
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
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
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
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
            world: world.into(),
            graph,
            resources: RenderResources::default(),
            blink: BlinkAlloc::new(),
            device,
            queue,

            tab_idgen: state.tab_idgen,
        }
    }

    pub fn on_event<'a>(&mut self, window_id: WindowId, event: WindowEvent) {
        let world = self.world.local();

        if Games::handle_event(world, window_id, &event) {
            return;
        };

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
                        world.insert_resource(Quit);
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
            self.world.insert_resource(Quit);
            return;
        }

        // Update plugins state.
        Plugins::tick(&mut self.world);

        // Simulate games.
        Games::tick(&mut self.world, step);

        for (viewport, egui) in self.world.view::<(&Viewport, &mut Egui)>() {
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
                                self.world.insert_resource_defer(Quit);
                                ui.close_menu();
                            }
                        });
                        ui.menu_button("Run", |ui| {
                            if ui.button("New game").clicked() {
                                tabs.push_to_first_leaf(Tab::game(
                                    &mut self.tab_idgen,
                                    GamesTab::new(&self.world),
                                ));
                            }
                        });
                        ui.menu_button("View", |ui| {
                            if ui.button("Game").clicked() {
                                tabs.push_to_first_leaf(Tab::game(
                                    &mut self.tab_idgen,
                                    GamesTab::default(),
                                ));
                                ui.close_menu();
                            }
                            if ui.button("Plugins").clicked() {
                                tabs.push_to_first_leaf(Tab::plugins(&mut self.tab_idgen));
                                ui.close_menu();
                            }
                            if ui.button("Console").clicked() {
                                tabs.push_to_first_leaf(Tab::console(&mut self.tab_idgen));
                                ui.close_menu();
                            }
                            if ui.button("Systems").clicked() {
                                tabs.push_to_first_leaf(Tab::systems(&mut self.tab_idgen));
                                ui.close_menu();
                            }
                            if ui.button("Filters").clicked() {
                                tabs.push_to_first_leaf(Tab::filters(&mut self.tab_idgen));
                                ui.close_menu();
                            }
                            if ui.button("WorkGraph").clicked() {
                                tabs.push_to_first_leaf(Tab::workgraph(&mut self.tab_idgen));
                                ui.close_menu();
                            }
                        });
                    });
                });
                egui_dock::DockArea::new(dock_state).show(
                    cx,
                    &mut AppModel {
                        world: &self.world,
                        window,
                    },
                )
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
        Games::render(&mut self.world, now);

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
        if self.world.get_resource::<Quit>().is_none() {
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

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        tab.id
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match tab.kind {
            TabKind::Plugins => Plugins::show(self.world, ui),
            TabKind::Console => Console::show(self.world, ui),
            TabKind::Systems => Systems::show(self.world, ui),
            TabKind::Filters => Filters::show(self.world, ui),
            TabKind::WorkGraph => WorkGraph::show(self.world, ui),
            TabKind::Game { ref mut tab } => tab.show(ui, self.world, self.window),
            // TabKind::Memory => Memory::show(&mut self.world, ui),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab.kind {
            TabKind::Plugins => "Plugins".into(),
            TabKind::Console => "Console".into(),
            TabKind::Systems => "Systems".into(),
            TabKind::Filters => "Filters".into(),
            TabKind::WorkGraph => "Work Graph".into(),
            TabKind::Game { .. } => "Game".into(),
            // TabKind::Memory => "Memory".into(),
        }
    }

    fn on_close(&mut self, tab: &mut Tab) -> bool {
        match &mut tab.kind {
            TabKind::Game { tab } => {
                tab.on_close(self.world);
            }
            _ => {}
        }
        true
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab.kind {
            TabKind::Game { .. } => [false, false],
            TabKind::Systems => [false, false],
            TabKind::Console => [false, false],
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
