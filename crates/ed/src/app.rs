use std::{borrow::Cow, hash::Hash, path::PathBuf};

use arboard::Clipboard;
use arcana::{
    gametime::{Clock, ClockStep, FrequencyNumExt, FrequencyTicker},
    input::ViewInput,
    mev,
    project::Project,
    Ident,
};
use egui::{Id, TopBottomPanel, WidgetText};
use egui_dock::{DockArea, DockState, Tree};
use miette::IntoDiagnostic;
use winit::{
    dpi,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

use crate::tool::{self, Toolbox};

use super::{
    assets::AssetRepository,
    filters::Filters,
    ide::{Ide, IdeType},
    init_mev,
    instance::Instance,
    plugins::Plugins,
    project::ProjectData,
    render::Rendering,
    sample::ImageSample,
    subprocess::{filter_subprocesses, kill_subprocesses},
    systems::Systems,
    tool::{Tool, ToolId},
    ui::{Ui, UiViewport, UserTextures},
};

#[derive(Clone, Default, egui_probe::EguiProbe, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    ide: Option<IdeType>,
}

pub enum UserEvent {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    Systems,

    Tool { id: ToolId },
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
    data: ProjectData,

    ui: Ui,

    assets: AssetRepository,
    main: Instance,

    image_sample: ImageSample,
    clipboard: Clipboard,

    clock: Clock,
    limiter: FrequencyTicker,

    cfg: AppConfig,

    // Currently configured IDE to use.
    ide: Option<Box<dyn Ide>>,
    toolbox: Toolbox,

    show_preferences: bool,

    plugins: Plugins,
    systems: Systems,

    /// App views correspond to windows.
    views: Vec<AppView>,
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
    pub fn new(project: Project, data: ProjectData) -> Self {
        let (device, queue) = init_mev();

        let plugins = Plugins::new();
        // let console = Console::new(event_collector);
        let systems = Systems::new();
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
            Err(err) => {
                tracing::warn!("Failed to load app cfg: {err:?}");
                AppConfig::default()
            }
        };

        let ide = match cfg.ide {
            None => None,
            Some(ide) => Some(ide.get()),
        };

        let assets = AssetRepository::new(&project.root_path().join("Assets"));

        let toolbox = Toolbox::new();
        App {
            should_quit: false,

            project,
            data,

            views,

            ui: Ui::new(),

            queue,

            assets,
            main,

            image_sample,
            clipboard,

            clock,
            limiter,
            cfg,

            ide,
            toolbox,
            show_preferences: false,

            plugins,
            systems,
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
        self.plugins.tick(&mut self.project, &mut self.data);

        if let Some(c) = self.plugins.take_updated() {
            self.toolbox
                .update_container(&mut self.project, &mut self.data, &c);
            self.systems.update_container(&mut self.data, &c);
            self.main.update_container(&c);
        }

        self.toolbox
            .tick(&mut self.project, &mut self.data, &mut self.main);

        self.main.tick(&self.data, step);
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
                                        focus_or_add_tab(
                                            view.dock_state.main_surface_mut(),
                                            Tab::Plugins,
                                        );
                                        ui.close_menu();
                                    }

                                    if ui.button("Systems").clicked() {
                                        focus_or_add_tab(
                                            view.dock_state.main_surface_mut(),
                                            Tab::Systems,
                                        );
                                        ui.close_menu();
                                    }

                                    for (plugin, name) in self.toolbox.enumerate() {
                                        if ui.button(format!("{name} @ {plugin}")).clicked() {
                                            todo!();
                                            // let id = self.toolbox.add(plugin, name);

                                            // view.tab_tree.tiles.insert_pane(Tab::Tool { id });
                                            // ui.close_menu();
                                        }
                                    }
                                });
                            });
                        });

                        egui::containers::CentralPanel::default().show(cx, |ui| {
                            let mut model = AppModel {
                                window: &view.window,
                                project: &mut self.project,
                                data: &mut self.data,
                                assets: &mut self.assets,
                                main: &mut self.main,
                                sample: &self.image_sample,
                                device: &device,
                                textures,
                                ide: self.ide.as_deref(),
                                toolbox: &mut self.toolbox,
                                plugins: &mut self.plugins,
                                systems: &mut self.systems,
                            };

                            let dock_area = DockArea::new(&mut view.dock_state);
                            dock_area.show_inside(ui, &mut model);
                        });

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
            .render(&mut self.queue, &self.data, &mut self.ui.textures())
            .unwrap();
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
                        tab_tree: Cow::Borrowed(view.dock_state.main_surface()),
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

                    let mut dock_state = DockState::new(Vec::new());
                    *dock_state.main_surface_mut() = view.tab_tree.into_owned();

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
                dock_state: DockState::new(Vec::new()),
                viewport,
            });
        }
    }
}

fn focus_or_add_tab(tree: &mut Tree<Tab>, tab: Tab) {
    if let Some((node_index, tab_index)) = tree.find_tab(&tab) {
        tree.set_focused_node(node_index);
        tree.set_active_tab(node_index, tab_index);
    } else {
        let _ = tree.push_to_focused_leaf(tab);
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppViewState<'a> {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    tab_tree: Cow<'a, Tree<Tab>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppState<'a> {
    views: Vec<AppViewState<'a>>,
}

struct AppModel<'a> {
    window: &'a Window,
    project: &'a mut Project,
    data: &'a mut ProjectData,
    assets: &'a mut AssetRepository,
    main: &'a mut Instance,
    sample: &'a ImageSample,
    device: &'a mev::Device,
    textures: UserTextures<'a>,
    ide: Option<&'a dyn Ide>,
    toolbox: &'a mut Toolbox,
    plugins: &'a mut Plugins,
    systems: &'a mut Systems,
}

impl egui_dock::widgets::TabViewer for AppModel<'_> {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match *tab {
            // Tab::Assets => self.assets.show(ui, self.main),
            Tab::Plugins => self.plugins.show(self.project, self.data, ui),
            // Tab::Console => self.console.show(ui),
            Tab::Systems => self.systems.show(self.project, self.data, self.ide, ui),
            // Tab::Filters => self.filters.show(self.project, self.data, self.ide, ui),
            // Tab::Codes => self.code.show(self.project, self.data, ui),
            // Tab::Rendering => self.rendering.show(
            //     self.project,
            //     self.data,
            //     self.sample,
            //     self.device,
            //     self.main,
            //     &mut self.textures,
            //     self.ide,
            //     ui,
            // ),
            // Tab::Main => self.main.show(self.window.id(), &mut self.textures, ui),
            // Tab::Inspector => {} //Inspector::show(self.world, ui),
            Tab::Tool { id } => {
                self.toolbox.show(
                    id,
                    self.project,
                    self.data,
                    self.ide.as_deref(),
                    self.main,
                    ui,
                );
            }
        }
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match *tab {
            // Tab::Assets => "Assets".into(),
            Tab::Plugins => "Plugins".into(),
            // Tab::Console => "Console".into(),
            Tab::Systems => "Systems".into(),
            // Tab::Filters => "Filters".into(),
            // Tab::Codes => "Codes".into(),
            // Tab::Rendering => "Rendering".into(),
            // Tab::Main => "Main".into(),
            // Tab::Inspector => "Inspector".into(),
            Tab::Tool { id } => self.toolbox.title(id).into(),
        }
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            // Tab::Assets => [false, false],
            // Tab::Console => [false, false],
            Tab::Systems => [false, false],
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

fn load_app_state(name: &str) -> miette::Result<AppState<'static>> {
    let path = app_state_path(true, name)
        .ok_or_else(|| miette::miette!("Failed to get app state path"))?;

    let mut file = std::fs::File::open(path).into_diagnostic()?;

    let state = serde_json::from_reader(&mut file).into_diagnostic()?;

    Ok(state)
}

fn save_app_state(state: &AppState, name: &str) -> miette::Result<()> {
    let path = app_state_path(true, name)
        .ok_or_else(|| miette::miette!("Failed to get app state path"))?;
    let mut file = std::fs::File::create(path).into_diagnostic()?;
    serde_json::to_writer_pretty(&mut file, state).into_diagnostic()?;
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

    let state = serde_json::from_reader(&mut file).into_diagnostic()?;

    Ok(state)
}

fn save_app_cfg(config: &AppConfig) -> miette::Result<()> {
    let path = app_cfg_path(true).ok_or_else(|| miette::miette!("Failed to get app state path"))?;
    let mut file = std::fs::File::create(path).into_diagnostic()?;
    serde_json::to_writer_pretty(&mut file, config).into_diagnostic()?;
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
