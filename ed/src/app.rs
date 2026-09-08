use std::path::PathBuf;

use arboard::Clipboard;
use arcana::{
    error::{Error, UnifyError, error},
    gametime::{Clock, ClockStep, FrequencyNumExt, FrequencyTicker},
    input::ViewInput,
    mev,
};
use egui::{Panel, WidgetText};
use egui_dock::{DockArea, DockState, Tree};
use egui_probe::Probe;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use winit::{
    dpi,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

use crate::{
    assets::AssetStore,
    error::ModalErrors,
    filters::FilterManager,
    ide::{Ide, IdeType},
    init_mev,
    plugins::PluginManager,
    project::{Project, ProjectData},
    render::RenderManager,
    simulation::Simulation,
    subprocess::{filter_subprocesses, kill_subprocesses},
    systems::SystemManager,
    toaster::Toaster,
    tool::{ToolCommand, ToolContext, ToolId, ToolState, Toolbox, Tools, View},
    ui::{Ui, UiViewport},
};

#[derive(Clone, Default, egui_probe::EguiProbe, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    ide: Option<IdeType>,
}

pub enum UserEvent {}

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

    assets: AssetStore,
    main: Simulation,

    cvt: mev::kernels::Cvt,
    clipboard: Clipboard,

    clock: Clock,
    limiter: FrequencyTicker,

    cfg: AppConfig,

    // Currently configured IDE to use.
    ide: Option<Box<dyn Ide>>,

    plugins: PluginManager,
    systems: SystemManager,
    filters: FilterManager,
    renders: RenderManager,

    /// App views correspond to windows.
    views: Vec<AppView>,
    toolbox: Toolbox,
    tools: Tools,
    tool_commands: Vec<ToolCommand>,
    suspended_views: Option<Vec<AppViewState<View>>>,

    preferences: Preferences,
    errors: ModalErrors,
    toaster: Toaster,
}

struct AppView {
    window: Window,
    surface: Option<mev::Surface>,
    dock_state: DockState<View>,
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
        let toaster = Toaster::new();

        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::filter::EnvFilter::from_default_env())
            .with(toaster.layer())
            .init();

        let cfg: AppConfig = match load_app_cfg() {
            Ok(cfg) => cfg,
            Err(error) => {
                tracing::warn!("Failed to load app cfg: {error:?}");
                AppConfig::default()
            }
        };

        let data = ProjectData::load(&project)?;

        let (device, queue) = init_mev();

        let mut plugins = PluginManager::new();
        plugins.load(&project, &data);

        let mut systems = SystemManager::new();
        systems.load(&project, &data);

        let mut filters = FilterManager::new();
        filters.load(&project, &data);

        let mut renders = RenderManager::new();
        renders.load(&project, &data);

        let cvt = mev::kernels::Cvt::new(&device);
        let main = Simulation::new();

        let clock = Clock::new();

        let clipboard = Clipboard::new().unwrap();

        let views = Vec::new();

        let limiter = clock.ticker(120.hz());

        let ide = match cfg.ide {
            None => None,
            Some(ide) => Some(ide.get()),
        };

        let assets = AssetStore::new(&project)?;

        Ok(App {
            should_quit: false,

            project,
            data,

            views,
            toolbox: Toolbox::new(),
            tools: Tools::default(),
            tool_commands: Vec::new(),
            suspended_views: None,

            ui: Ui::new(),

            queue,

            assets,
            main,

            cvt,
            clipboard,

            clock,
            limiter,
            cfg: cfg.clone(),

            ide,

            plugins,
            systems,
            filters,
            renders,

            preferences: Preferences::new(cfg),
            errors: ModalErrors::new(),
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
        self.plugins.tick(&mut self.toaster, &self.project);

        if let Some(c) = self.plugins.take_updated() {
            self.systems.update_plugins(&c);
            self.main.update_plugins(&c);
            self.assets.update_plugins(&c);
        }

        self.tools.update(
            step,
            &mut ToolContext {
                simulation: &mut self.main,
                plugins: &mut self.plugins,
                systems: &mut self.systems,
                filters: &mut self.filters,
                renders: &mut self.renders,

                queue: &mut self.queue,
                cvt: &mut self.cvt,
                textures: self.ui.textures(),

                errors: &mut self.errors,
                toaster: &mut self.toaster,

                ide: self.ide.as_deref(),
                toolbox: &self.toolbox,
                commands: &mut self.tool_commands,

                project: &self.project,
                data: &self.data,
            },
        );
        self.drain_tool_commands(None);
        self.toaster.tick();
        self.main.tick(&self.systems, step);
    }

    /// Runs rendering.
    pub fn handle_event(&mut self, window_id: WindowId, event: &WindowEvent) {
        if self.main.handle_event(&self.filters, window_id, event) {
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
                self.ui.run(
                    &mut view.viewport,
                    &mut self.clipboard,
                    &view.window,
                    self.clock.now(),
                    |ui, textures| {
                        Panel::top("Menu").show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.menu_button("File", |ui| {
                                    if ui.button("Save").clicked() {
                                        self.plugins.save(&mut self.project, &mut self.data);
                                        self.systems.save(&mut self.project, &mut self.data);
                                        self.filters.save(&mut self.project, &mut self.data);
                                        self.renders.save(&mut self.project, &mut self.data);

                                        self.data.save_in_ui(&self.project, ui, &mut self.errors);
                                        ui.close();
                                    }

                                    if ui.button("Preferences").clicked() {
                                        self.preferences.open(ui.viewport_id());
                                        ui.close();
                                    }

                                    if ui.button("Exit").clicked() {
                                        self.should_quit = true;
                                        ui.close();
                                    }
                                });
                                ui.menu_button("New tool", |ui| {
                                    for template in self.toolbox.templates() {
                                        if ui.button(template.title()).clicked() {
                                            if let Some(tool) =
                                                self.toolbox.materialize(template.key(), None)
                                            {
                                                self.tool_commands.push(ToolCommand::Create {
                                                    tool,
                                                    template: Some(template.key().to_owned()),
                                                    open: true,
                                                });
                                            }
                                            ui.close();
                                        }
                                    }
                                });
                                egui::ComboBox::from_id_salt("alive-tools")
                                    .selected_text("Tools")
                                    .show_ui(ui, |ui| {
                                        for (id, entry) in self.tools.iter() {
                                            if ui
                                                .selectable_label(
                                                    false,
                                                    format!("{} #{}", entry.tool.title(), id),
                                                )
                                                .clicked()
                                            {
                                                self.tool_commands.push(ToolCommand::Open(id));
                                                ui.close();
                                            }
                                        }
                                    });
                            });
                        });

                        egui::containers::CentralPanel::default().show(ui, |ui| {
                            let mut model = AppModel {
                                tools: &mut self.tools,
                                window: window_id,
                                context: ToolContext {
                                    simulation: &mut self.main,
                                    plugins: &mut self.plugins,
                                    systems: &mut self.systems,
                                    filters: &mut self.filters,
                                    renders: &mut self.renders,

                                    queue: &mut self.queue,
                                    cvt: &mut self.cvt,
                                    textures: textures,

                                    errors: &mut self.errors,
                                    toaster: &mut self.toaster,

                                    ide: self.ide.as_deref(),
                                    toolbox: &self.toolbox,
                                    commands: &mut self.tool_commands,

                                    project: &self.project,
                                    data: &self.data,
                                },
                            };
                            let dock_area = DockArea::new(&mut view.dock_state);
                            dock_area.show_inside(ui, &mut model);
                        });

                        if let Err(error) = self.preferences.show(ui, &mut self.cfg) {
                            self.errors
                                .push_error(ui.viewport_id(), "Preferences Error", error);
                        }
                        self.toaster.show(ui);
                        self.errors.show(ui);
                    },
                );

                view.window.request_redraw();

                break;
            }
        }
        self.drain_tool_commands(Some(window_id));
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
            .render(&mut self.queue, &self.renders, &mut self.ui.textures())
            .unwrap();
    }

    fn save_state(&self) {
        let (ids, tools) = self.tools.snapshot();
        let mut views: Vec<_> = self
            .views
            .iter()
            .map(|view| {
                let scale = view.window.scale_factor();
                AppViewState {
                    pos: view
                        .window
                        .inner_position()
                        .unwrap_or_default()
                        .to_logical(scale),
                    size: view.window.inner_size().to_logical(scale),
                    dock_state: view
                        .dock_state
                        .filter_map_tabs(|view| ids.get(&view.tool()).copied()),
                    maximized: view.window.is_maximized(),
                }
            })
            .collect();
        if let Some(suspended) = &self.suspended_views {
            views.extend(suspended.iter().map(|view| {
                AppViewState {
                    pos: view.pos,
                    size: view.size,
                    maximized: view.maximized,
                    dock_state: view
                        .dock_state
                        .filter_map_tabs(|view| ids.get(&view.tool()).copied()),
                }
            }));
        }
        for view in &mut views {
            normalize_layout(&mut view.dock_state);
        }
        let state = AppState { tools, views };
        if let Err(error) = save_app_state(&state, &self.project.name()) {
            tracing::error!("Failed to save app state: {error:?}");
        }
    }

    fn drain_tool_commands(&mut self, preferred: Option<WindowId>) {
        for command in std::mem::take(&mut self.tool_commands) {
            match command {
                ToolCommand::Create {
                    tool,
                    template,
                    open,
                } => {
                    let id = self.tools.insert(tool, template);
                    if open {
                        self.open_tool(id, preferred);
                    }
                }
                ToolCommand::Open(id) => self.open_tool(id, preferred),
                ToolCommand::Detach(id) => self.close_tool_view(id, true),
                ToolCommand::Close(id) => self.close_tool_view(id, false),
            }
        }
    }

    fn open_tool(&mut self, id: ToolId, preferred: Option<WindowId>) {
        if self.suspended_views.is_some() {
            self.tool_commands.push(ToolCommand::Open(id));
            return;
        }
        for window in &mut self.views {
            if let Some(path) = window.dock_state.find_tab_from(|view| view.tool() == id) {
                window
                    .dock_state
                    .set_focused_node_and_surface(path.node_path());
                let _ = window.dock_state.set_active_tab(path);
                window.window.focus_window();
                window.window.request_redraw();
                return;
            }
        }
        let index = self
            .views
            .iter()
            .position(|v| Some(v.window.id()) == preferred)
            .unwrap_or(0);
        if let Some(window) = self.views.get_mut(index) {
            if let Some(view) = self.tools.attach(id) {
                let _ = window
                    .dock_state
                    .main_surface_mut()
                    .push_to_focused_leaf(view);
                window.window.request_redraw();
            }
        }
    }

    fn close_tool_view(&mut self, id: ToolId, detach: bool) {
        for window in &mut self.views {
            if let Some(path) = window.dock_state.find_tab_from(|view| view.tool() == id) {
                window.dock_state.remove_tab(path);
            }
        }
        if let Some(windows) = &mut self.suspended_views {
            for window in windows {
                if let Some(path) = window.dock_state.find_tab_from(|view| view.tool() == id) {
                    window.dock_state.remove_tab(path);
                }
            }
        }
        if detach {
            self.tools.detach(id);
        } else {
            self.tools.remove(id);
        }
    }

    fn load_state(&mut self, events: &ActiveEventLoop) {
        if !self.views.is_empty() {
            return;
        }
        let state = match self.suspended_views.take() {
            Some(views) => Ok(views),
            None => load_app_state(&self.project.name()).map(|state| {
                let restored = self.tools.restore(&self.toolbox, state.tools);
                state
                    .views
                    .into_iter()
                    .map(|view| AppViewState {
                        pos: view.pos,
                        size: view.size,
                        maximized: view.maximized,
                        dock_state: view.dock_state.filter_map_tabs(|index| {
                            self.tools.attach(*restored.get(*index)?.as_ref()?)
                        }),
                    })
                    .collect()
            }),
        };

        match state {
            Err(error) => {
                tracing::warn!("Failed to load app state: {error:?}");
            }
            Ok(views) => {
                for view in views {
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

                    let dock_state = view.dock_state;

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

#[derive(serde::Serialize, serde::Deserialize)]
struct AppViewState<T = usize> {
    pos: dpi::LogicalPosition<f64>,
    size: dpi::LogicalSize<f64>,
    maximized: bool,
    dock_state: DockState<T>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppState {
    views: Vec<AppViewState>,
    tools: Vec<ToolState>,
}

struct AppModel<'a> {
    tools: &'a mut Tools,
    window: WindowId,
    context: ToolContext<'a>,
}

impl egui_dock::widgets::TabViewer for AppModel<'_> {
    type Tab = View;

    fn id(&mut self, view: &mut View) -> egui::Id {
        egui::Id::new(("tool", view.tool()))
    }

    fn title(&mut self, view: &mut View) -> WidgetText {
        self.tools.title(view.tool()).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, view: &mut View) {
        if let Some(entry) = self.tools.get_mut(view.tool()) {
            ui.push_id(("tool", view.tool()), |ui| {
                entry.tool.ui(ui, self.window, &mut self.context)
            });
        }
    }

    fn on_close(&mut self, view: &mut View) -> egui_dock::tab_viewer::OnCloseResponse {
        self.context.commands.push(ToolCommand::Close(view.tool()));
        egui_dock::tab_viewer::OnCloseResponse::Close
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, view: &mut View, _path: egui_dock::NodePath) {
        if ui.button("Detach").clicked() {
            self.context.commands.push(ToolCommand::Detach(view.tool()));
            ui.close();
        }
    }
}

/// Layout rectangles are recomputed by egui and can be infinite before first draw.
fn normalize_layout<T>(dock: &mut DockState<T>) {
    for (_, node) in dock.iter_all_nodes_mut() {
        node.set_rect(egui::Rect::ZERO);
        if let Some(leaf) = node.get_leaf_mut() {
            leaf.viewport = egui::Rect::ZERO;
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
    decode_app_state(state).unify_error()
}

fn decode_app_state(value: serde_json::Value) -> Result<AppState, serde_json::Error> {
    if value.get("tools").is_some() {
        return serde_json::from_value(value);
    }
    #[derive(serde::Deserialize)]
    struct LegacyView {
        pos: dpi::LogicalPosition<f64>,
        size: dpi::LogicalSize<f64>,
        maximized: bool,
        tab_tree: Tree<serde_json::Value>,
    }
    #[derive(serde::Deserialize)]
    struct LegacyState {
        views: Vec<LegacyView>,
    }
    let legacy: LegacyState = serde_json::from_value(value)?;
    let mut tools = Vec::new();
    let views = legacy
        .views
        .into_iter()
        .map(|view| {
            let tree = view.tab_tree.filter_map_tabs(|tab| {
                let (key, state) = tab.as_object()?.iter().next()?;
                let template = match key.as_str() {
                    "Plugins" => "plugins",
                    "Systems" => "systems",
                    _ => return None,
                };
                let index = tools.len();
                tools.push(ToolState {
                    template: template.to_owned(),
                    state: state.clone(),
                });
                Some(index)
            });
            let mut dock_state = DockState::new(Vec::new());
            *dock_state.main_surface_mut() = tree;
            AppViewState {
                pos: view.pos,
                size: view.size,
                maximized: view.maximized,
                dock_state,
            }
        })
        .collect();
    Ok(AppState { views, tools })
}

fn save_app_state(state: &AppState, name: &str) -> Result<(), Error> {
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
        if self.suspended_views.is_some() {
            return;
        }
        self.save_state();
        self.suspended_views = Some(
            self.views
                .drain(..)
                .map(|mut view| {
                    let scale = view.window.scale_factor();
                    AppViewState {
                        pos: view
                            .window
                            .inner_position()
                            .unwrap_or_default()
                            .to_logical(scale),
                        size: view.window.inner_size().to_logical(scale),
                        maximized: view.window.is_maximized(),
                        dock_state: std::mem::replace(
                            &mut view.dock_state,
                            DockState::new(Vec::new()),
                        ),
                    }
                })
                .collect(),
        );
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
                    if let Some(index) = self
                        .views
                        .iter()
                        .position(|view| view.window.id() == window_id)
                    {
                        let window = self.views.remove(index);
                        for (_, view) in window.dock_state.iter_all_tabs() {
                            self.tools.remove(view.tool());
                        }
                    }
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

    fn show(&mut self, ui: &egui::Context, cfg: &mut AppConfig) -> Result<(), Error> {
        let mut result = Ok(());

        if self.open == Some(ui.viewport_id()) {
            egui::Modal::new(egui::Id::new("arcana-ed-preferences")).show(ui, |ui| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_layout_migrates_widget_snapshots() {
        let mut tree = Tree::new(vec![
            serde_json::json!({"Plugins": {}}),
            serde_json::json!({"Systems": {}}),
        ]);
        for node in tree.iter_mut() {
            node.set_rect(egui::Rect::ZERO);
            if let Some(leaf) = node.get_leaf_mut() {
                leaf.viewport = egui::Rect::ZERO;
            }
        }
        let value = serde_json::json!({"views": [{"pos": {"x": 10.0, "y": 20.0}, "size": {"width": 800.0, "height": 600.0}, "maximized": false, "tab_tree": tree}]});
        let state = decode_app_state(value).unwrap();
        assert_eq!(state.tools.len(), 2);
        assert_eq!(state.tools[0].template, "plugins");
        assert_eq!(state.tools[1].template, "systems");
        assert_eq!(
            state.views[0]
                .dock_state
                .iter_all_tabs()
                .map(|(_, tab)| *tab)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        let roundtrip = decode_app_state(serde_json::to_value(state).unwrap()).unwrap();
        assert_eq!(roundtrip.views[0].dock_state.iter_all_tabs().count(), 2);
    }
}
