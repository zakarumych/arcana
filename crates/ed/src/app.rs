use std::{borrow::Cow, hash::Hash, path::PathBuf};

use arboard::Clipboard;
use arcana::{
    blink_alloc::BlinkAlloc, gametime::FrequencyNumExt, input::ViewInput, mev, project::Project,
    Clock, ClockStep, FrequencyTicker,
};
use egui::{Id, TopBottomPanel, WidgetText};
use egui_dock::{DockState, NodeIndex, TabIndex, TabViewer, Tree};
use egui_tracing::EventCollector;
use miette::IntoDiagnostic;
use winit::{
    dpi,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

use crate::{
    code::CodeTool,
    container::Container,
    data::ProjectData,
    filters::Filters,
    ide::{Ide, IdeType},
    init_mev,
    instance::Instance,
    plugins::Plugins,
    render::Rendering,
    sample::ImageSample,
    subprocess::{filter_subprocesses, kill_subprocesses},
    systems::Systems,
    ui::{Ui, UiViewport, UserTextures},
};

#[derive(Clone, Default, egui_probe::EguiProbe, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    ide: Option<IdeType>,
}

pub enum UserEvent {}

/// Editor tab.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    // Console,
    Systems,
    Filters,
    Rendering,
    // Main,
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
    // console: Console,
    code: CodeTool,
    systems: Systems,
    filters: Filters,
    rendering: Rendering,
    main: Instance,

    image_sample: ImageSample,
    clipboard: Clipboard,
    should_quit: bool,

    clock: Clock,
    limiter: FrequencyTicker,
    cfg: AppConfig,
    show_preferences: bool,

    ide: Option<Box<dyn Ide>>,
}

struct AppView {
    window: Window,
    surface: Option<mev::Surface>,
    dock_state: DockState<Tab>,
    viewport: UiViewport,
}

impl App {
    pub fn new(_event_collector: EventCollector, project: Project, data: ProjectData) -> Self {
        let (device, queue) = init_mev();

        let plugins = Plugins::new();
        // let console = Console::new(event_collector);
        let systems = Systems::new();
        let filters = Filters::new();
        let rendering = Rendering::new();
        let image_sample = ImageSample::new(&device).unwrap();
        let code = CodeTool::new();
        let main = Instance::new();

        let clock = Clock::new();

        let clipboard = Clipboard::new().unwrap();

        let views = Vec::new();

        let limiter = clock.ticker(120.hz());

        let cfg: AppConfig = match load_app_cfg() {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!("Failed to load app cfg: {err:?}");
                AppConfig::default()
            }
        };

        let ide = match cfg.ide {
            None => None,
            Some(ide) => Some(ide.get()),
        };

        App {
            project,
            data,
            container: None,

            views,

            ui: Ui::new(),

            blink: BlinkAlloc::new(),
            queue,

            // console,
            plugins,
            code,
            systems,
            filters,
            rendering,
            main,

            image_sample,
            clipboard,

            should_quit: false,

            clock,
            limiter,
            cfg,
            show_preferences: false,

            ide,
        }
    }

    pub fn try_tick(&mut self, events: &ActiveEventLoop) {
        let step = self.clock.step();

        let ticks = self.limiter.ticks(step.step);

        for clock in ticks {
            self.tick(clock);
        }

        filter_subprocesses();

        let until = self.clock.stamp_instant(self.limiter.next_tick().unwrap());
        events.set_control_flow(ControlFlow::WaitUntil(until));
    }

    pub fn tick(&mut self, step: ClockStep) {
        let update = self
            .plugins
            .tick(&mut self.project, &self.data, self.container.is_none());

        if let Some(c) = update {
            self.systems.update_plugins(&mut self.data, &c);
            self.filters.update_plugins(&mut self.data, &c);
            self.code.update_plugins(&mut self.data, &c);
            self.rendering.update_plugins(&mut self.data, &c);
            self.main.update_plugins(&c);

            self.container = Some(c);
        }

        self.main.tick(&self.data, &self.systems, step);
    }

    /// Runs rendering.
    pub fn handle_event(&mut self, window_id: WindowId, event: &WindowEvent) {
        if self.main.handle_event(&self.data, window_id, event) {
            return;
        }

        for view in &mut self.views {
            if view.window.id() == window_id {
                let Ok(event) = ViewInput::try_from(event) else {
                    return;
                };

                self.ui
                    .handle_event(&mut view.viewport, &mut self.clipboard, &event);
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
                                    if ui.button("Preferences").clicked() {
                                        self.show_preferences = true;
                                        ui.close_menu();
                                    }

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
                                    // if ui.button("Console").clicked() {
                                    //     focus_or_add_tab(tabs, Tab::Console);
                                    //     ui.close_menu();
                                    // }
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
                                    // if ui.button("Main").clicked() {
                                    //     focus_or_add_tab(tabs, Tab::Main);
                                    //     ui.close_menu();
                                    // }
                                });
                            });
                        });

                        let mut model = AppModel {
                            window: &view.window,
                            linked: self.container.as_ref(),
                            project: &mut self.project,
                            data: &mut self.data,
                            plugins: &mut self.plugins,
                            // console: &mut self.console,
                            systems: &mut self.systems,
                            filters: &mut self.filters,
                            code: &mut self.code,
                            rendering: &mut self.rendering,
                            main: &mut self.main,
                            sample: &self.image_sample,
                            device: &device,
                            textures,
                            ide: self.ide.as_deref(),
                        };

                        egui_dock::DockArea::new(&mut view.dock_state).show(cx, &mut model);

                        if self.show_preferences {
                            egui::Window::new("Preferences")
                                .collapsible(false)
                                .title_bar(true)
                                .resizable(false)
                                .open(&mut self.show_preferences)
                                .show(cx, |ui| {
                                    egui_probe::Probe::new(&mut self.cfg).show(ui);

                                    if let Err(err) = save_app_cfg(&self.cfg) {
                                        tracing::error!("Failed to save app cfg: {err:?}");
                                    }

                                    match self.cfg.ide {
                                        None => self.ide = None,
                                        Some(ide) => self.ide = Some(ide.get()),
                                    }
                                });
                        }
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

                break;
            }
        }

        self.main
            .render(&mut self.queue, &self.data, &mut self.ui.textures());
    }

    fn save_state(&self) {
        let state = AppState {
            views: self
                .views
                .iter()
                .map(|view| {
                    let scale_factor = view.window.scale_factor();
                    AppViewState {
                        pos: view
                            .window
                            .inner_position()
                            .unwrap_or_default()
                            .to_logical(scale_factor),
                        size: view.window.inner_size().to_logical(scale_factor),
                        dock_state: Cow::Borrowed(&view.dock_state),
                        maximized: view.window.is_maximized(),
                    }
                })
                .collect(),
        };

        if let Err(err) = save_app_state(&state, &self.project.name()) {
            tracing::error!("Failed to save app state: {err:?}");
        }
    }

    fn load_state(&mut self, events: &ActiveEventLoop) {
        let state = load_app_state(&self.project.name());

        match state {
            Err(err) => {
                tracing::warn!("Failed to load app state: {err:?}");
            }
            Ok(state) => {
                self.views.clear();

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
                        dock_state: view.dock_state.into_owned(),
                        viewport,
                    };

                    self.views.push(view);
                }
            }
        }

        if self.views.is_empty() {
            tracing::info!("Start from clean app state");

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
struct AppViewState<'a> {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    dock_state: Cow<'a, DockState<Tab>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppState<'a> {
    views: Vec<AppViewState<'a>>,
}

struct AppModel<'a> {
    window: &'a Window,
    linked: Option<&'a Container>,
    project: &'a mut Project,
    data: &'a mut ProjectData,
    plugins: &'a mut Plugins,
    // console: &'a mut Console,
    systems: &'a mut Systems,
    filters: &'a mut Filters,
    code: &'a mut CodeTool,
    rendering: &'a mut Rendering,
    main: &'a mut Instance,
    sample: &'a ImageSample,
    device: &'a mev::Device,
    textures: UserTextures<'a>,
    ide: Option<&'a dyn Ide>,
}

impl TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        Id::new(*tab)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match *tab {
            Tab::Plugins => self.plugins.show(self.linked, self.project, self.data, ui),
            // Tab::Console => self.console.show(ui),
            Tab::Systems => self.systems.show(self.project, self.data, self.ide, ui),
            Tab::Filters => self.filters.show(self.project, self.data, self.ide, ui),
            Tab::Codes => self.code.show(self.project, self.data, ui),
            Tab::Rendering => self.rendering.show(
                self.project,
                self.data,
                self.sample,
                self.device,
                self.main,
                &mut self.textures,
                self.ide,
                ui,
            ),
            // Tab::Main => self.main.show(self.window.id(), &mut self.textures, ui),
            Tab::Inspector => {} //Inspector::show(self.world, ui),
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match *tab {
            Tab::Plugins => "Plugins".into(),
            // Tab::Console => "Console".into(),
            Tab::Systems => "Systems".into(),
            Tab::Filters => "Filters".into(),
            Tab::Codes => "Codes".into(),
            Tab::Rendering => "Rendering".into(),
            // Tab::Main => "Main".into(),
            Tab::Inspector => "Inspector".into(),
        }
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            // Tab::Console => [false, false],
            Tab::Systems => [false, false],
            Tab::Codes => [false, false],
            Tab::Rendering => [false, false],
            _ => [true, true],
        }
    }
}

fn app_state_path(create: bool, name: &str) -> Option<PathBuf> {
    let mut path = match dirs::config_dir() {
        None => {
            let mut path = std::env::current_exe().ok()?;
            path.pop();
            path
        }
        Some(mut path) => {
            path.push("Arcana Engine");
            path.push(name);
            if create {
                std::fs::create_dir_all(&*path).ok()?;
            }
            path
        }
    };
    path.push("ed.bin");
    Some(path)
}

fn load_app_state(name: &str) -> miette::Result<AppState<'static>> {
    let path = app_state_path(true, name)
        .ok_or_else(|| miette::miette!("Failed to get app state path"))?;

    let mut file = std::fs::File::open(path).into_diagnostic()?;

    let state = bincode::deserialize_from(&mut file).into_diagnostic()?;

    Ok(state)
}

fn save_app_state(state: &AppState, name: &str) -> miette::Result<()> {
    let path = app_state_path(true, name)
        .ok_or_else(|| miette::miette!("Failed to get app state path"))?;
    let mut file = std::fs::File::create(path).into_diagnostic()?;
    bincode::serialize_into(&mut file, state).into_diagnostic()?;
    Ok(())
}

fn app_cfg_path(create: bool) -> Option<PathBuf> {
    let mut path = match dirs::config_dir() {
        None => {
            let mut path = std::env::current_exe().ok()?;
            path.pop();
            path
        }
        Some(mut path) => {
            path.push("Arcana Engine");
            path.push("Config");
            if create {
                std::fs::create_dir_all(&*path).ok()?;
            }
            path
        }
    };
    path.push("ed.bin");
    Some(path)
}

fn load_app_cfg() -> miette::Result<AppConfig> {
    let path = app_cfg_path(true).ok_or_else(|| miette::miette!("Failed to get app cfg path"))?;

    let mut file = std::fs::File::open(path).into_diagnostic()?;

    let state = bincode::deserialize_from(&mut file).into_diagnostic()?;

    Ok(state)
}

fn save_app_cfg(config: &AppConfig) -> miette::Result<()> {
    let path = app_cfg_path(true).ok_or_else(|| miette::miette!("Failed to get app state path"))?;
    let mut file = std::fs::File::create(path).into_diagnostic()?;
    bincode::serialize_into(&mut file, config).into_diagnostic()?;
    Ok(())
}

impl winit::application::ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        self.load_state(events);
    }

    fn suspended(&mut self, _events: &ActiveEventLoop) {
        self.save_state();
        self.views.clear();
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
                if self.views.len() == 1 {
                    self.should_quit = true;
                } else {
                    self.views.retain(|view| view.window.id() != window_id);
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

    fn exiting(&mut self, _events: &ActiveEventLoop) {
        self.save_state();
        kill_subprocesses();
    }
}
