use core::fmt;
use std::{hash::Hash, path::PathBuf};

use arboard::Clipboard;
use arcana::{
    error::{Error, UnifyError, error},
    gametime::{Clock, ClockStep, FrequencyNumExt, FrequencyTicker},
    input::ViewInput,
    mev,
};
use egui::{TopBottomPanel, WidgetText};
use egui_dock::{DockArea, DockState, Tree};
use egui_probe::Probe;
use tracing_subscriber::layer::SubscriberExt as _;
use winit::{
    dpi,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

use crate::{
    assets::AssetStore,
    error::ModalError,
    filters::Filters,
    ide::{Ide, IdeType},
    init_mev,
    instance::Instance,
    plugins::{PluginsManager, PluginsWidget},
    project::Project,
    render::Rendering,
    sample::ImageSample,
    subprocess::{filter_subprocesses, kill_subprocesses},
    systems::{SystemsManager, SystemsWidget},
    toaster::Toaster,
    tool::Toolbox,
    ui::{Ui, UiViewport, UserTextures},
};

#[derive(Clone, Default, egui_probe::EguiProbe, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    ide: Option<IdeType>,
}

pub enum UserEvent {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum TabKind {
    Plugins,
    Systems,
}

impl fmt::Display for TabKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabKind::Plugins => f.write_str("Plugins"),
            TabKind::Systems => f.write_str("Systems"),
        }
    }
}

impl TabKind {
    fn build(&self) -> Tab {
        match self {
            TabKind::Plugins => Tab::Plugins(PluginsWidget::new()),
            TabKind::Systems => Tab::Systems(SystemsWidget::new()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins(PluginsWidget),
    Systems(SystemsWidget),
}

impl Tab {
    fn kind(&self) -> TabKind {
        match self {
            Tab::Plugins(_) => TabKind::Plugins,
            Tab::Systems(_) => TabKind::Systems,
        }
    }
}

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    should_quit: bool,

    /// Graphics queue.
    /// It should be placed higher in field order
    /// to ensure that all processing is stopped and resources are released
    /// before views are dropped where surfaces require acquired images to be freed.
    queue: mev::Queue,

    // Project main state.
    project: Project,

    ui: Ui,

    assets: AssetStore,
    main: Instance,

    image_sample: ImageSample,
    clipboard: Clipboard,

    clock: Clock,
    limiter: FrequencyTicker,

    cfg: AppConfig,

    // Currently configured IDE to use.
    ide: Option<Box<dyn Ide>>,
    toolbox: Toolbox,

    plugins: PluginsManager,
    systems: SystemsManager,

    /// App views correspond to windows.
    views: Vec<AppView>,

    preferences: Preferences,
    modal_error: ModalError,
    toaster: Toaster,
}

struct AppView {
    window: Window,
    surface: Option<mev::Surface>,
    dock_state: DockState<Tab>,
    viewport: UiViewport,
}

impl Drop for AppView {
    fn drop(&mut self) {
        if let Some(surface) = self.surface.take() {
            drop(surface);
        }
    }
}

impl App {
    pub fn new(project: Project) -> Result<Self, Error> {
        let (device, queue) = init_mev();

        let plugins = PluginsManager::new();
        // let console = Console::new(event_collector);
        let systems = SystemsManager::new();
        let filters = Filters::new();
        let rendering = Rendering::new();
        let image_sample = ImageSample::new(&device).unwrap();
        let main = Instance::new();

        let clock = Clock::new();

        let clipboard = Clipboard::new().unwrap();

        let views = Vec::new();

        let limiter = clock.ticker(120.hz());

        let cfg: AppConfig = match load_app_cfg() {
            Ok(cfg) => cfg,
            Err(error) => {
                tracing::warn!("Failed to load app cfg: {error:?}");
                AppConfig::default()
            }
        };

        let ide = match cfg.ide {
            None => None,
            Some(ide) => Some(ide.get()),
        };

        let assets = AssetStore::new(&project)?;

        let toolbox = Toolbox::new();
        let toaster = Toaster::new();

        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                // .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .finish()
                .with(toaster.tracing_layer()),
        )
        .expect("Global subscriber is set only once");

        Ok(App {
            should_quit: false,

            project,

            views,

            ui: Ui::new(),

            queue,

            assets,
            main,

            image_sample,
            clipboard,

            clock,
            limiter,
            cfg: cfg.clone(),

            ide,
            toolbox,

            plugins,
            systems,

            preferences: Preferences::new(cfg),
            modal_error: ModalError::new(),
            toaster,
        })
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
        self.plugins.tick(&mut self.project, &mut self.toaster);

        if let Some(c) = self.plugins.take_updated() {
            self.toolbox.update_plugins(&mut self.project, &c);
            self.systems.update_plugins(&mut self.project, &c);
            self.main.update_plugins(&c);
            self.assets.update_plugins(&c);
        }

        self.toolbox.tick(&mut self.project, &mut self.main);
        self.toaster.tick();
        self.main.tick(&self.project, step);
    }

    /// Runs rendering.
    pub fn handle_event(&mut self, window_id: WindowId, event: &WindowEvent) {
        if self.main.handle_event(&self.project, window_id, event) {
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
                        TopBottomPanel::top("Menu").show(cx, |ui| {
                            ui.horizontal(|ui| {
                                ui.menu_button("File", |ui| {
                                    if ui.button("Preferences").clicked() {
                                        self.preferences.open(cx.viewport_id());
                                        ui.close();
                                    }

                                    if ui.button("Exit").clicked() {
                                        self.should_quit = true;
                                        ui.close();
                                    }
                                });
                                ui.menu_button("View", |ui| {
                                    let tab_kinds = [TabKind::Plugins, TabKind::Systems];
                                    for tab in tab_kinds {
                                        if ui.button(tab.to_string()).clicked() {
                                            focus_or_add_tab(
                                                view.dock_state.main_surface_mut(),
                                                tab,
                                            );
                                            ui.close();
                                        }
                                    }

                                    for (plugin, name) in self.toolbox.enumerate() {
                                        if ui.button(format!("{name} @ {plugin}")).clicked() {
                                            todo!();
                                            // let id = self.toolbox.add(plugin, name);

                                            // view.tab_tree.tiles.insert_pane(Tab::Tool { id });
                                            // ui.close();
                                        }
                                    }
                                });
                            });
                        });

                        egui::containers::CentralPanel::default().show(cx, |ui| {
                            let mut model = AppModel {
                                // window: &view.window,
                                project: &mut self.project,
                                main: &mut self.main,
                                sample: &self.image_sample,
                                device: &device,
                                textures,
                                ide: self.ide.as_deref(),
                                toolbox: &mut self.toolbox,
                                plugins: &mut self.plugins,
                                systems: &mut self.systems,
                                modal_error: &mut self.modal_error,
                                toaster: &mut self.toaster,
                            };

                            let dock_area = DockArea::new(&mut view.dock_state);
                            dock_area.show_inside(ui, &mut model);
                        });

                        if let Err(error) = self.preferences.show(cx, &mut self.cfg) {
                            self.modal_error.push_error(
                                cx.viewport_id(),
                                "Preferences Error",
                                error,
                            );
                        }
                        self.toaster.show(cx);
                        self.modal_error.show(cx);
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
                        Err(error) => {
                            tracing::error!("Failed to create surface: {error}");
                            return;
                        }
                    },
                };

                let frame = match surface.next_frame() {
                    Ok(frame) => frame,
                    Err(error) => {
                        tracing::error!("Failed to acquire frame: {error}");
                        view.surface = None;
                        return;
                    }
                };

                self.ui.render(&mut view.viewport, frame, &mut self.queue);

                break;
            }
        }

        self.main
            .render(&mut self.queue, &self.project, &mut self.ui.textures())
            .unwrap();
    }

    fn save_state(&self) {
        let state = AppStateRef {
            views: self
                .views
                .iter()
                .map(|view| {
                    let scale_factor = view.window.scale_factor();
                    AppViewStateRef {
                        pos: view
                            .window
                            .inner_position()
                            .unwrap_or_default()
                            .to_logical(scale_factor),
                        size: view.window.inner_size().to_logical(scale_factor),
                        tab_tree: view.dock_state.main_surface(),
                        maximized: view.window.is_maximized(),
                    }
                })
                .collect(),
        };

        if let Err(error) = save_app_state(&state, &self.project.name()) {
            tracing::error!("Failed to save app state: {error:?}");
        }
    }

    fn load_state(&mut self, events: &ActiveEventLoop) {
        let state = load_app_state(&self.project.name());

        match state {
            Err(error) => {
                tracing::warn!("Failed to load app state: {error:?}");
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
                        .map_err(|error| {
                            Error::msg(format!("Failed to create Ed window: {error:?}"))
                        })
                        .unwrap();

                    if view.maximized {
                        window.set_maximized(true);
                    }

                    let size = window.inner_size();

                    let viewport = self.ui.new_viewport(
                        egui::vec2(size.width as f32, size.height as f32),
                        window.scale_factor() as f32,
                    );

                    let mut dock_state = DockState::new(Vec::new());
                    *dock_state.main_surface_mut() = view.tab_tree;

                    let view = AppView {
                        window,
                        surface: None,
                        dock_state,
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
                .map_err(|error| Error::msg(format!("Failed to create Ed window: {error:?}")))
                .unwrap();

            let size = window.inner_size();

            let viewport = self.ui.new_viewport(
                egui::vec2(size.width as f32, size.height as f32),
                window.scale_factor() as f32,
            );

            self.views.push(AppView {
                window,
                surface: None,
                dock_state: DockState::new(Vec::new()),
                viewport,
            });
        }
    }
}

fn focus_or_add_tab(tree: &mut Tree<Tab>, kind: TabKind) {
    if let Some((node_index, tab_index)) = tree.find_tab_from(|t| t.kind() == kind) {
        tree.set_focused_node(node_index);
        tree.set_active_tab(node_index, tab_index);
    } else {
        let _ = tree.push_to_focused_leaf(kind.build());
    }
}

#[derive(serde::Serialize)]
struct AppViewStateRef<'a> {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    tab_tree: &'a Tree<Tab>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppViewState {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    tab_tree: Tree<Tab>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppState {
    views: Vec<AppViewState>,
}

#[derive(serde::Serialize)]
struct AppStateRef<'a> {
    views: Vec<AppViewStateRef<'a>>,
}

struct AppModel<'a> {
    // window: &'a Window,
    project: &'a mut Project,
    main: &'a mut Instance,
    sample: &'a ImageSample,
    device: &'a mev::Device,
    textures: UserTextures<'a>,
    ide: Option<&'a dyn Ide>,
    toolbox: &'a mut Toolbox,
    plugins: &'a mut PluginsManager,
    systems: &'a mut SystemsManager,
    modal_error: &'a mut ModalError,
    toaster: &'a mut Toaster,
}

impl egui_dock::widgets::TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Plugins(widget) => {
                widget.show(self.plugins, self.project, self.modal_error, ui);
            }
            Tab::Systems(widget) => {
                widget.show(self.systems, self.project, self.modal_error, self.ide, ui)
            }
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        tab.kind().to_string().into()
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            // Tab::Assets => [false, false],
            // Tab::Console => [false, false],
            // Tab::Systems => [false, false],
            // Tab::Codes => [false, false],
            // Tab::Rendering => [false, false],
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

fn load_app_state(name: &str) -> Result<AppState, Error> {
    let path = app_state_path(true, name).ok_or_else(|| error!("Failed to get app state path"))?;

    let mut file = std::fs::File::open(path).unify_error()?;

    let state = serde_json::from_reader(&mut file).unify_error()?;

    Ok(state)
}

fn save_app_state(state: &AppStateRef, name: &str) -> Result<(), Error> {
    let path = app_state_path(true, name).ok_or_else(|| error!("Failed to get app state path"))?;
    let mut file = std::fs::File::create(path).unify_error()?;
    serde_json::to_writer_pretty(&mut file, state).unify_error()?;
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

fn load_app_cfg() -> Result<AppConfig, Error> {
    let path = app_cfg_path(true).ok_or_else(|| error!("Failed to get app cfg path"))?;

    let mut file = std::fs::File::open(path).unify_error()?;

    let state = serde_json::from_reader(&mut file).unify_error()?;

    Ok(state)
}

fn save_app_cfg(config: &AppConfig) -> Result<(), Error> {
    let path = app_cfg_path(true).ok_or_else(|| error!("Failed to get app state path"))?;
    let mut file = std::fs::File::create(path).unify_error()?;
    serde_json::to_writer_pretty(&mut file, config).unify_error()?;
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

struct Preferences {
    cfg: AppConfig,
    open: Option<egui::ViewportId>,
}

impl Preferences {
    fn new(cfg: AppConfig) -> Self {
        Preferences { cfg, open: None }
    }

    fn open(&mut self, viewport_id: egui::ViewportId) {
        self.open = Some(viewport_id);
    }

    fn show(&mut self, cx: &egui::Context, cfg: &mut AppConfig) -> Result<(), Error> {
        let mut result = Ok(());

        if self.open == Some(cx.viewport_id()) {
            egui::Modal::new(egui::Id::new("arcana-ed-preferences")).show(cx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Editor Preferences");

                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        Probe::new(&mut self.cfg).show(ui);
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Accept").clicked() {
                            if let Err(error) = save_app_cfg(&self.cfg) {
                                tracing::error!("Failed to save app config: {error:?}");
                                result = Err(error);
                            }
                            *cfg = self.cfg.clone();
                        }

                        if ui.button("OK").clicked() {
                            if let Err(error) = save_app_cfg(&self.cfg) {
                                tracing::error!("Failed to save app config: {error:?}");
                                result = Err(error);
                            }
                            *cfg = self.cfg.clone();
                            self.open = None;
                        }

                        if ui.button("Cancel").clicked() {
                            self.open = None;
                        }
                    });
                });
            });
        }

        result
    }
}
