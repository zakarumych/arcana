use std::{path::PathBuf, sync::Arc};

use arcana::{
    blink_alloc::BlinkAlloc,
    edict::world::WorldLocal,
    events::ViewportEvent,
    game::Quit,
    init_mev, mev,
    project::Project,
    render::{render, RenderGraph, RenderResources},
    viewport::Viewport,
    ClockStep, Entities, With, WorldBuilder,
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
    window::{Window, WindowBuilder, WindowId},
};

use crate::{data::ProjectData, games::GamesTab};

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

    blink: BlinkAlloc,

    device: mev::Device,
    queue: Arc<Mutex<mev::Queue>>,
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

        let mut world = builder.build_local();
        world.insert_resource(project);
        world.insert_resource(Plugins::new());
        world.insert_resource(Console::new(event_collector));
        world.insert_resource(Games::new());
        world.insert_resource(device.clone());
        world.insert_resource(queue.clone());
        world.insert_resource(data);

        let mut graph = RenderGraph::new();

        let state = load_app_state().unwrap_or_default();

        let mut dock_states = HashMap::new();

        if state.windows.is_empty() {
            let builder = WindowBuilder::new().with_title("Ed");
            let window = builder
                .build(events)
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
            let builder = WindowBuilder::new()
                .with_title("Ed")
                .with_position(w.pos)
                .with_inner_size(w.size)
                .with_maximized(w.maximized);

            let window = builder
                .build(&events)
                .map_err(|err| miette::miette!("Failed to create Ed window: {err}"))
                .unwrap();

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
        }
    }

    pub fn on_event<'a>(&mut self, window_id: WindowId, event: WindowEvent<'a>) {
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

        // Try to launch games requested on last tick.
        Games::launch_games(&mut self.world);

        // Simulate games.
        Games::tick(&mut self.world, step);

        for (viewport, egui) in self.world.view::<(&Viewport, &mut Egui)>() {
            let window = viewport.get_window();
            let dock_state = self
                .dock_states
                .entry(window.id())
                .or_insert_with(|| DockState::new(vec![]));

            egui.run(step.now, |cx| {
                let (scroll_delta, hover_pos) =
                    cx.input(|i| (i.scroll_delta.y, i.pointer.hover_pos()));

                if scroll_delta != 0.0 {
                    dbg!(scroll_delta);
                }

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
                                tabs.push_to_first_leaf(Tab::Game {
                                    tab: GamesTab::new(&self.world),
                                });
                            }
                        });
                        ui.menu_button("View", |ui| {
                            if ui.button("Game").clicked() {
                                tabs.push_to_first_leaf(Tab::Game {
                                    tab: GamesTab::default(),
                                });
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

    pub fn render(&mut self) {
        Games::render(&mut self.world);

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
}

struct AppModel<'a> {
    world: &'a WorldLocal,
    window: &'a Window,
}

impl TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match *tab {
            Tab::Plugins => Plugins::show(self.world, ui),
            Tab::Console => Console::show(self.world, ui),
            Tab::Systems => Systems::show(self.world, ui),
            Tab::Filters => Filters::show(self.world, ui),
            Tab::Game { ref mut tab } => tab.show(ui, self.world, self.window),
            // Tab::Memory => Memory::show(&mut self.world, ui),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab {
            Tab::Plugins => "Plugins".into(),
            Tab::Console => "Console".into(),
            Tab::Systems => "Systems".into(),
            Tab::Filters => "Filters".into(),
            Tab::Game { .. } => "Game".into(),
            // Tab::Memory => "Memory".into(),
        }
    }

    fn on_close(&mut self, tab: &mut Tab) -> bool {
        match tab {
            Tab::Game { tab } => {
                tab.on_close(self.world);
            }
            _ => {}
        }
        true
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            Tab::Game { .. } => [false, false],
            Tab::Systems => [false, false],
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
