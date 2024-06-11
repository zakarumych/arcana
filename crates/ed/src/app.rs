use std::{hash::Hash, path::PathBuf, time::Instant};

use arboard::Clipboard;
use arcana::{
    blink_alloc::BlinkAlloc,
    gametime::{FrequencyNumExt, TimeStamp},
    input::ViewportInput,
    mev,
    project::Project,
    Clock, ClockStep, FrequencyTicker,
};
use egui::{Id, TopBottomPanel, WidgetText};
use egui_dock::{DockState, NodeIndex, TabIndex, TabViewer, Tree};
use egui_tracing::EventCollector;
use winit::{
    dpi,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

use crate::{
    code::Codes,
    console::Console,
    container::Container,
    data::ProjectData,
    filters::Filters,
    init_mev,
    inspector::Inspector,
    instance::Main,
    plugins::Plugins,
    render::Rendering,
    sample::ImageSample,
    subprocess::{filter_subprocesses, kill_subprocesses},
    systems::Systems,
    ui::{Ui, UiViewport, UserTextures},
};

pub enum UserEvent {}

/// Editor tab.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    Console,
    Systems,
    Filters,
    Rendering,
    Main,
    Codes,
    Inspector,
    // Custom(ToolId),
}

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    project: Project,
    data: ProjectData,
    container: Option<Container>,

    views: Vec<AppView>,

    ui: Ui,

    blink: BlinkAlloc,

    queue: mev::Queue,

    plugins: Plugins,
    console: Console,
    codes: Codes,
    systems: Systems,
    filters: Filters,
    rendering: Rendering,
    main: Main,

    image_sample: ImageSample,
    clipboard: Clipboard,
    should_quit: bool,

    clock: Clock,
    limiter: FrequencyTicker,
}

struct AppView {
    window: Window,
    surface: Option<mev::Surface>,
    dock_state: DockState<Tab>,
    viewport: UiViewport,
}

impl Drop for App {
    fn drop(&mut self) {
        kill_subprocesses();
    }
}

impl App {
    pub fn new(event_collector: EventCollector, project: Project, data: ProjectData) -> Self {
        let (device, queue) = init_mev();

        let plugins = Plugins::new();
        let console = Console::new(event_collector);
        let systems = Systems::new();
        let filters = Filters::new();
        let rendering = Rendering::new();
        let image_sample = ImageSample::new(&device).unwrap();
        let codes = Codes::new();
        let main = Main::new();

        let clipboard = Clipboard::new().unwrap();

        let views = Vec::new();

        let clock = Clock::new();
        let limiter = clock.ticker(20.hz());

        App {
            project,
            data,
            container: None,

            views,

            ui: Ui::new(),

            blink: BlinkAlloc::new(),
            queue,

            console,
            plugins,
            codes,
            systems,
            filters,
            rendering,
            main,

            image_sample,
            clipboard,

            should_quit: false,

            clock,
            limiter,
        }
    }

    pub fn try_tick(&mut self, events: &ActiveEventLoop) {
        let mut last = self.clock.now();
        let step = self.clock.step();

        let ticks = self.limiter.ticks(step.step);

        for tick in ticks {
            self.tick(ClockStep {
                now: tick,
                step: tick - last,
            });
            last = tick;
        }

        filter_subprocesses();

        let until = self.clock.stamp_instant(self.limiter.next_tick().unwrap());
        events.set_control_flow(ControlFlow::WaitUntil(until));
    }

    pub fn tick(&mut self, clock: ClockStep) {
        self.plugins
            .tick(&mut self.container, &mut self.project, &self.data);
    }

    /// Runs rendering.
    pub fn handle_event(&mut self, window_id: WindowId, event: &WindowEvent) {
        for view in &mut self.views {
            if view.window.id() == window_id {
                let Ok(event) = ViewportInput::try_from(event) else {
                    return;
                };

                self.ui.handle_event(&mut view.viewport, &event);
                break;
            }
        }
    }

    /// Update UI.
    pub fn update_ui(&mut self, window_id: WindowId) {
        for view in &mut self.views {
            if view.window.id() == window_id {
                let device = self.queue.device().clone();

                self.ui.run(
                    &mut view.viewport,
                    &mut self.clipboard,
                    &view.window,
                    self.clock.now(),
                    |cx, textures| {
                        let tabs = view.dock_state.main_surface_mut();
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
                                    if ui.button("Codes").clicked() {
                                        focus_or_add_tab(tabs, Tab::Codes);
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

                        let mut model = AppModel {
                            window: &view.window,
                            linked: self.container.as_ref(),
                            project: &mut self.project,
                            data: &mut self.data,
                            plugins: &mut self.plugins,
                            console: &mut self.console,
                            systems: &mut self.systems,
                            filters: &mut self.filters,
                            codes: &mut self.codes,
                            rendering: &mut self.rendering,
                            main: &mut self.main,
                            sample: &self.image_sample,
                            device: &device,
                            textures,
                        };

                        egui_dock::DockArea::new(&mut view.dock_state).show(cx, &mut model);
                    },
                );

                view.window.request_redraw();

                break;
            }
        }
    }

    /// Runs rendering.
    pub fn render(&mut self, window_id: WindowId) {
        for view in &mut self.views {
            if view.window.id() == window_id {
                // let mut render_view = |view: &mut AppView| {
                let surface = match &mut view.surface {
                    Some(surface) => surface,
                    slot => match self.queue.new_surface(&view.window, &view.window) {
                        Ok(surface) => slot.get_or_insert(surface),
                        Err(err) => {
                            tracing::error!("Failed to create surface: {err}");
                            return;
                        }
                    },
                };

                let frame = match surface.next_frame() {
                    Ok(frame) => frame,
                    Err(err) => {
                        tracing::error!("Failed to acquire frame: {err}");
                        view.surface = None;
                        return;
                    }
                };

                self.ui.render(&mut view.viewport, frame, &mut self.queue);

                if self.ui.has_requested_repaint_for(&view.viewport) {
                    view.window.request_redraw();
                }

                break;
            }
        }
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
struct AppViewState {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    dock_state: DockState<Tab>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppState {
    views: Vec<AppViewState>,
}

struct AppModel<'a> {
    window: &'a Window,
    linked: Option<&'a Container>,
    project: &'a mut Project,
    data: &'a mut ProjectData,
    plugins: &'a mut Plugins,
    console: &'a mut Console,
    systems: &'a mut Systems,
    filters: &'a mut Filters,
    codes: &'a mut Codes,
    rendering: &'a mut Rendering,
    main: &'a mut Main,
    sample: &'a ImageSample,
    device: &'a mev::Device,
    textures: UserTextures<'a>,
}

impl TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        Id::new(*tab)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match *tab {
            Tab::Plugins => self.plugins.show(self.linked, self.project, self.data, ui),
            Tab::Console => self.console.show(ui),
            Tab::Systems => self.systems.show(self.project, self.data, ui),
            Tab::Filters => self.filters.show(self.project, self.data, ui),
            Tab::Codes => self.codes.show(self.project, self.data, ui),
            Tab::Rendering => {}
            // self.rendering.show(
            //     self.project,
            //     self.sample,
            //     self.data,
            //     ui,
            //     self.device,
            //     self.textures,
            // ),
            Tab::Main => {}      //Main::show(self.world, ui, self.window.id()),
            Tab::Inspector => {} //Inspector::show(self.world, ui),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match *tab {
            Tab::Plugins => "Plugins".into(),
            Tab::Console => "Console".into(),
            Tab::Systems => "Systems".into(),
            Tab::Filters => "Filters".into(),
            Tab::Codes => "Codes".into(),
            Tab::Rendering => "Rendering".into(),
            Tab::Main => "Main".into(),
            Tab::Inspector => "Inspector".into(),
        }
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            Tab::Console => [false, false],
            Tab::Systems => [false, false],
            Tab::Codes => [false, false],
            Tab::Rendering => [false, false],
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

impl winit::application::ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        let state = load_app_state();

        match state {
            None => {
                let builder = Window::default_attributes().with_title("Ed");

                let window = events
                    .create_window(builder)
                    .map_err(|err| miette::miette!("Failed to create Ed window: {err:?}"))
                    .unwrap();

                let size = window.inner_size();

                let viewport = self.ui.new_viewport(
                    egui::vec2(size.width as f32, size.height as f32),
                    window.scale_factor() as f32,
                );

                self.views.push(AppView {
                    window,
                    surface: None,
                    dock_state: DockState::new(vec![]),
                    viewport,
                });
            }
            Some(state) => {
                for view in state.views {
                    let builder = Window::default_attributes()
                        .with_title("Ed")
                        .with_position(view.pos)
                        .with_inner_size(view.size);

                    let window: Window = events
                        .create_window(builder)
                        .map_err(|err| miette::miette!("Failed to create Ed window: {err:?}"))
                        .unwrap();

                    if view.maximized {
                        window.set_maximized(true);
                    }

                    let size = window.inner_size();

                    let viewport = self.ui.new_viewport(
                        egui::vec2(size.width as f32, size.height as f32),
                        window.scale_factor() as f32,
                    );

                    let view = AppView {
                        window,
                        surface: None,
                        dock_state: view.dock_state,
                        viewport,
                    };

                    self.views.push(view);
                }
            }
        };
    }

    fn suspended(&mut self, _events: &ActiveEventLoop) {
        let state = AppState {
            views: self
                .views
                .iter_mut()
                .map(|view| {
                    let scale_factor = view.window.scale_factor();
                    AppViewState {
                        pos: view
                            .window
                            .inner_position()
                            .unwrap_or_default()
                            .to_logical(scale_factor),
                        size: view.window.inner_size().to_logical(scale_factor),
                        dock_state: std::mem::replace(&mut view.dock_state, DockState::new(vec![])),
                        maximized: view.window.is_maximized(),
                    }
                })
                .collect(),
        };

        self.views.clear();
        let _ = save_app_state(&state);
    }

    fn new_events(&mut self, events: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.try_tick(events);
    }

    fn window_event(&mut self, events: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        self.handle_event(window_id, &event);

        if self.should_quit {
            events.exit();
        }

        match event {
            WindowEvent::CloseRequested => {
                self.views.retain(|view| view.window.id() != window_id);

                if self.views.is_empty() {
                    self.should_quit = true;
                }
            }
            WindowEvent::RedrawRequested => {
                self.update_ui(window_id);
                self.render(window_id);
            }
            _ => {}
        }

        self.try_tick(events);
    }
}
