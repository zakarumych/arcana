use std::{
    env::args_os,
    path::PathBuf,
    process::{Child, ExitCode, Termination},
};

use arcana_launcher::{validate_engine_path, Dependency, Ident, Profile, Project, Start};
use egui_file::FileDialog;
use hashbrown::HashMap;

enum Exit {
    Success,
    CliError(miette::Error),
    EFrameError(eframe::Error),
}

impl Termination for Exit {
    fn report(self) -> ExitCode {
        match self {
            Exit::Success => ExitCode::SUCCESS,
            Exit::CliError(err) => Termination::report(Err::<(), _>(err)),
            Exit::EFrameError(err) => Termination::report(Err::<(), _>(err)),
        }
    }
}

fn main() -> Exit {
    use tracing_subscriber::layer::SubscriberExt as _;

    if let Err(err) = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            // .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish()
            .with(tracing_error::ErrorLayer::default()),
    ) {
        panic!("Failed to install tracing subscriber: {}", err);
    }

    let args = args_os();
    if args.len() > 1 {
        match arcana_launcher_cli::run_cli() {
            Ok(()) => return Exit::Success,
            Err(err) => return Exit::CliError(err),
        }
    }

    let native_options = eframe::NativeOptions::default();
    let err = eframe::run_native(
        "My egui App",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    );

    match err {
        Ok(()) => Exit::Success,
        Err(err) => Exit::EFrameError(err),
    }
}

impl eframe::App for App {
    fn update(&mut self, cx: &egui::Context, _frame: &mut eframe::Frame) {
        match self.child {
            AppChild::None => {}
            AppChild::EditorBuilding(ref mut child, _) => match child.try_wait() {
                Err(err) => {
                    self.dialog = Some(AppDialog::Error(ErrorDialog {
                        title: "Failed to check if build finished".to_owned(),
                        message: err.to_string(),
                    }));
                    self.child = AppChild::None;
                }
                Ok(Some(status)) => {
                    if status.success() {
                        match self.child {
                            AppChild::EditorBuilding(_, ref path) => {
                                let project = self.recent.get(path).unwrap().as_ref().unwrap();
                                self.child = AppChild::None;

                                match project.run_editor_non_blocking(self.profile) {
                                    Err(err) => {
                                        self.dialog = Some(AppDialog::Error(ErrorDialog {
                                            title: "Failed to run Arcana Ed".to_owned(),
                                            message: err.to_string(),
                                        }));
                                    }
                                    Ok(child) => {
                                        self.child = AppChild::EditorRunning(child);
                                        return;
                                    }
                                }
                            }
                            _ => unreachable!(),
                        }
                    } else {
                        self.dialog = Some(AppDialog::Error(ErrorDialog {
                            title: "Failed to build Arcana Ed".to_owned(),
                            message: format!("{}", status),
                        }));
                        self.child = AppChild::None;
                    }
                }
                Ok(None) => {}
            },
            AppChild::EditorRunning(ref mut child) => {
                match child.try_wait() {
                    Err(err) => {
                        self.dialog = Some(AppDialog::Error(ErrorDialog {
                            title: "Failed to check if Arcana Ed closed".to_owned(),
                            message: err.to_string(),
                        }));
                        self.child = AppChild::None;
                    }
                    Ok(Some(status)) => {
                        if !status.success() {
                            self.dialog = Some(AppDialog::Error(ErrorDialog {
                                title: "Arcana Ed exited with error".to_owned(),
                                message: format!("{}", status),
                            }));
                        }
                        self.child = AppChild::None;
                    }
                    Ok(None) => {
                        egui::CentralPanel::default().show(cx, |ui| {
                            // ui.horizontal_centered(|ui| {
                            ui.vertical_centered_justified(|ui| {
                                egui::Frame::window(ui.style()).show(ui, |ui| {
                                    ui.label("Arcana Ed is running");
                                });
                            });
                            // });
                        });

                        // Editor is still running.
                        return;
                    }
                }
            }
        }

        let mut run_editor = None;

        egui::TopBottomPanel::top("Menu").show(cx, |ui| {
            if self.dialog.is_some() || self.child.is_some() {
                ui.disable();
            }

            ui.menu_button("File", |ui| {
                let r = ui.button("New Project");
                if r.clicked() {
                    let engine = self.start.list_engine_versions().first().cloned();
                    self.dialog = Some(AppDialog::NewProject(NewProject::new(engine)));
                    ui.close_menu();
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("Create new project");
                    });
                }

                let r = ui.button("Open Project");
                if r.clicked() {
                    let mut dialog = FileDialog::open_file(None)
                        .title("Open project")
                        .show_new_folder(false);
                    dialog.open();
                    self.dialog = Some(AppDialog::OpenProject(dialog));

                    ui.close_menu();
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("Create new project");
                    });
                }

                let r = ui.button("Add Engine path");
                if r.clicked() {
                    let mut dialog: FileDialog = FileDialog::select_folder(None)
                        .title("Add engine")
                        .show_new_folder(false);
                    dialog.open();
                    self.dialog = Some(AppDialog::AddEngine(dialog));

                    ui.close_menu();
                } else {
                    r.on_hover_ui(|ui| {
                        ui.label("Add new engine to Arcana Launcher");
                    });
                }
            });
        });

        egui::TopBottomPanel::top("Controls").show(cx, |ui| {
            if self.dialog.is_some() || self.child.is_some() {
                ui.disable();
            }

            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.profile == Profile::Debug, "Debug")
                    .clicked()
                {
                    self.profile = Profile::Debug;
                }
                if ui
                    .selectable_label(self.profile == Profile::Release, "Release")
                    .clicked()
                {
                    self.profile = Profile::Release;
                }
            });
        });

        let mut remove_recent = None;

        egui::CentralPanel::default().show(cx, |ui| {
            if self.dialog.is_some() {
                ui.disable();
            }

            let recent = self.start.recent();

            if recent.len() == 0 {
                ui.vertical_centered(|ui| {
                    ui.allocate_space(egui::vec2(0.0, ui.available_height() * 0.5));
                    ui.label("No recent projects");

                    let r = ui.button("New Project");
                    if r.clicked() {
                        let engine = self.start.list_engine_versions().first().cloned();
                        self.dialog = Some(AppDialog::NewProject(NewProject::new(engine)));
                        ui.close_menu();
                    } else {
                        r.on_hover_ui(|ui| {
                            ui.label("Create new project");
                        });
                    }
                });
            } else {
                ui.vertical(|ui| {
                    for path in recent {
                        let result = match self.recent.entry(path.to_owned()) {
                            hashbrown::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                            hashbrown::hash_map::Entry::Vacant(entry) => {
                                entry.insert(Project::open(&path))
                            }
                        };

                        match result {
                            Err(err) => {
                                egui::Frame::group(ui.style())
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_RED))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.add_enabled(
                                                false,
                                                egui::Button::new(
                                                    egui::RichText::from(
                                                        egui_phosphor::regular::FOLDER_NOTCH_OPEN,
                                                    )
                                                    .size(30.0),
                                                ),
                                            );

                                            ui.vertical(|ui| {
                                                ui.label(format!("cannot open project. {err:?}"));
                                                ui.label(path.display().to_string());
                                            });
                                            let r = ui.button(egui_phosphor::regular::X);

                                            if r.clicked() {
                                                remove_recent = Some(path.to_owned());
                                            } else {
                                                r.on_hover_ui(|ui| {
                                                    ui.label("Remove from thisl list");
                                                });
                                            }
                                        });
                                    });
                            }
                            Ok(project) => {
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let r = ui.add(egui::Button::new(
                                            egui::RichText::from(
                                                egui_phosphor::regular::FOLDER_NOTCH_OPEN,
                                            )
                                            .size(30.0),
                                        ));

                                        ui.vertical(|ui| {
                                            ui.label(project.name().as_str());
                                            ui.label(project.manifest_path().display().to_string());
                                        });

                                        if r.clicked() {
                                            run_editor = Some(path.to_owned());
                                        } else {
                                            r.on_hover_ui(|ui| {
                                                ui.label("Open this project");
                                            });
                                        }
                                        let r = ui.button(egui_phosphor::regular::X);

                                        if r.clicked() {
                                            remove_recent = Some(path.to_owned());
                                        } else {
                                            r.on_hover_ui(|ui| {
                                                ui.label("Remove from thisl list");
                                            });
                                        }
                                    });
                                });
                            }
                        }
                    }
                });
            }
        });

        if let Some(path) = remove_recent {
            self.start.remove_recent(&path);
        }

        match self.child {
            AppChild::None => match self.dialog {
                None => {}
                Some(AppDialog::Error(ref error)) => {
                    if error.show(cx) {
                        self.dialog = None;
                        cx.request_repaint();
                    }
                }
                Some(AppDialog::OpenProject(ref mut file_dialog)) => {
                    match file_dialog.show(cx).state() {
                        egui_file::State::Open => {}
                        egui_file::State::Closed | egui_file::State::Cancelled => {
                            self.dialog = None;
                        }
                        egui_file::State::Selected => match file_dialog.path() {
                            None => {
                                self.dialog = None;
                            }
                            Some(path) => {
                                let result = Project::open(path);
                                let result =
                                    self.recent.entry(path.to_owned()).insert(result).into_mut();

                                match result {
                                    Err(err) => {
                                        self.dialog = Some(AppDialog::Error(ErrorDialog {
                                            title: "Failed to open project".to_owned(),
                                            message: err.to_string(),
                                        }));
                                    }
                                    Ok(project) => {
                                        let path = project.manifest_path().to_owned();
                                        self.start.add_recent(path.clone());

                                        run_editor = Some(path.to_owned());
                                        self.dialog = None;
                                    }
                                }
                            }
                        },
                    }
                }
                Some(AppDialog::AddEngine(ref mut file_dialog)) => {
                    match file_dialog.show(cx).state() {
                        egui_file::State::Open => {}
                        egui_file::State::Closed | egui_file::State::Cancelled => {
                            self.dialog = None;
                        }
                        egui_file::State::Selected => match file_dialog.path() {
                            None => {
                                self.dialog = None;
                            }
                            Some(path) => match Dependency::from_path(path) {
                                Some(engine) => {
                                    self.start.add_engine(engine);
                                    self.dialog = None;
                                }
                                None => {
                                    self.dialog = Some(AppDialog::Error(ErrorDialog {
                                        title: "Failed to add engine".to_owned(),
                                        message: "Invalid engine path".to_owned(),
                                    }));
                                }
                            },
                        },
                    }
                }
                Some(AppDialog::NewProject(ref mut new_project)) => {
                    match new_project.show(&mut self.start, cx) {
                        None => {}
                        Some(None) => {
                            self.dialog = None;
                            cx.request_repaint();
                        }
                        Some(Some(project)) => {
                            let path = project.manifest_path().to_owned();
                            self.recent.insert(path.clone(), Ok(project));
                            self.start.add_recent(path.clone());

                            self.dialog = None;
                            cx.request_repaint();
                            run_editor = Some(path);
                        }
                    }
                }
            },
            AppChild::EditorBuilding(_, _) => {
                egui::Window::new("Preparing project")
                    .resizable(false)
                    .collapsible(false)
                    .show(cx, |ui| {
                        ui.label("Preparing project...");
                        ui.spinner();
                    });
            }
            AppChild::EditorRunning(_) => {
                unreachable!()
            }
        }

        if cx.requested_repaint_last_frame() {
            cx.request_repaint();
        }

        match run_editor {
            None => {}
            Some(path) => {
                let project = self.recent.get(&path).unwrap().as_ref().unwrap();

                match project.build_editor_non_blocking(self.profile) {
                    Err(err) => {
                        self.dialog = Some(AppDialog::Error(ErrorDialog {
                            title: "Failed to run project".to_owned(),
                            message: err.to_string(),
                        }));
                    }
                    Ok(child) => {
                        self.child = AppChild::EditorBuilding(child, path);
                    }
                };
            }
        }
    }
}

struct ErrorDialog {
    title: String,
    message: String,
}

impl ErrorDialog {
    fn show(&self, cx: &egui::Context) -> bool {
        let title = egui::WidgetText::from(&*self.title);
        let message = egui::WidgetText::from(&*self.message);

        let mut close = false;
        egui::Window::new(title)
            .resizable(false)
            .collapsible(false)
            .show(cx, |ui| {
                ui.label(message);
                if ui.button("Ok").clicked() {
                    close = true;
                }
            });
        close
    }
}

enum AppDialog {
    NewProject(NewProject),
    OpenProject(FileDialog),
    AddEngine(FileDialog),
    Error(ErrorDialog),
}

enum AppChild {
    None,
    EditorBuilding(Child, PathBuf),
    EditorRunning(Child),
}

impl AppChild {
    fn is_some(&self) -> bool {
        !matches!(self, AppChild::None)
    }
}

impl Drop for AppChild {
    fn drop(&mut self) {
        match self {
            AppChild::None => {}
            AppChild::EditorBuilding(child, _) => {
                let _ = child.kill();
            }
            AppChild::EditorRunning(child) => {
                let _ = child.kill();
            }
        }
    }
}

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    start: Start,
    profile: Profile,
    recent: HashMap<PathBuf, Result<Project, miette::Report>>,

    /// Open dialog.
    dialog: Option<AppDialog>,

    /// Running child app.
    /// When this is `Some`, laucher is not interactive,
    /// window is hidden.
    /// But event-loop is still running.
    ///
    /// When child app finishes, launcher is shown again.
    child: AppChild,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        App {
            start: Start::new(),
            profile: Profile::Debug,
            recent: HashMap::new(),

            dialog: None,
            child: AppChild::None,
        }
    }
}

enum NewProjectDialog {
    Error(ErrorDialog),
    PickProjectPath(FileDialog),
    PickEnginePath(FileDialog),
}

/// This widget is used to configure and create new project.
struct NewProject {
    /// Name of new project.
    ///
    /// If bad `Ident` is provided, project may not be created.
    name: String,

    /// Path to new project.
    /// This path is absolute and normalized.
    path: Option<PathBuf>,
    path_string: String,

    /// If true, advanced options are shown.
    advanced: bool,

    /// Chosen engine version.
    ///
    /// If none, project may not be created.
    engine: Option<Dependency>,

    /// Current dialog.
    dialog: Option<NewProjectDialog>,
}

impl NewProject {
    /// Creates new `NewProject` widget.
    fn new(engine: Option<Dependency>) -> Self {
        NewProject {
            name: String::new(),
            path: None,
            path_string: String::new(),
            advanced: false,
            engine,
            dialog: None,
        }
    }

    fn can_create_project(&self) -> bool {
        Ident::from_str(&self.name).is_ok() && self.path.is_some() && self.engine.is_some()
    }

    fn show(&mut self, start: &mut Start, cx: &egui::Context) -> Option<Option<Project>> {
        let mut create_project = false;
        let mut close_dialog = false;

        egui::Window::new("New project")
            .auto_sized()
            .default_pos(egui::pos2(50.0, 50.0))
            .collapsible(false)
            .show(cx, |ui| {
                if self.dialog.is_some() {
                    ui.disable();
                }

                egui::Grid::new("new-project-settings")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        let cfg_name_layout = egui::Layout::right_to_left(egui::Align::Center);
                        ui.with_layout(cfg_name_layout, |ui| ui.label("Name"));
                        ui.text_edit_singleline(&mut self.name);
                        ui.end_row();

                        ui.with_layout(cfg_name_layout, |ui| ui.label("Path"));
                        ui.horizontal(|ui| {
                            let r = ui.text_edit_singleline(&mut self.path_string);
                            if r.changed() {
                                if self.path_string.is_empty() {
                                    self.path = None;
                                } else {
                                    self.path = Some(PathBuf::from(&self.path_string));
                                }
                            }
                            let r = ui.small_button(egui_phosphor::regular::DOTS_THREE);
                            if r.clicked() {
                                let mut dialog =
                                    FileDialog::select_folder(None).title("Select project path");
                                dialog.open();
                                self.dialog = Some(NewProjectDialog::PickProjectPath(dialog));
                            }
                        });
                        ui.end_row();

                        ui.checkbox(&mut self.advanced, "Advanced");
                        ui.end_row();

                        if self.advanced {
                            ui.with_layout(cfg_name_layout, |ui| ui.label("Engine"));
                            let mut cbox = egui::ComboBox::from_id_source("versions").width(300.0);

                            if let Some(v) = &self.engine {
                                cbox = cbox.selected_text(display_dependency(v));
                            }

                            cbox.show_ui(ui, |ui| {
                                for v in start.list_engine_versions() {
                                    ui.selectable_value(
                                        &mut self.engine,
                                        Some(v.clone()),
                                        display_dependency(v),
                                    );
                                }
                            });

                            let r = ui.small_button(egui_phosphor::regular::DOTS_THREE);
                            if r.clicked() {
                                let mut dialog = FileDialog::select_folder(None)
                                    .title("Select engine path")
                                    .show_new_folder(false);
                                dialog.open();
                                self.dialog = Some(NewProjectDialog::PickEnginePath(dialog));
                            }

                            ui.end_row();
                        }
                    });

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    let r = ui.add_enabled(self.can_create_project(), egui::Button::new("Create"));
                    create_project = r.clicked();

                    let r = ui.add(egui::Button::new("Cancel"));
                    close_dialog = r.clicked();
                });
            });

        match self.dialog {
            None => {}
            Some(NewProjectDialog::Error(ref error)) => {
                if error.show(cx) {
                    self.dialog = None;
                }
            }
            Some(NewProjectDialog::PickProjectPath(ref mut file_dialog)) => {
                match file_dialog.show(cx).state() {
                    egui_file::State::Open => {}
                    egui_file::State::Closed | egui_file::State::Cancelled => {
                        self.dialog = None;
                    }
                    egui_file::State::Selected => {
                        if let Some(path) = file_dialog.path() {
                            self.path = Some(path.to_owned());
                            self.path_string = path.display().to_string();
                        }
                        self.dialog = None;
                    }
                }
            }
            Some(NewProjectDialog::PickEnginePath(ref mut file_dialog)) => {
                match file_dialog.show(cx).state() {
                    egui_file::State::Open => {}
                    egui_file::State::Closed | egui_file::State::Cancelled => {
                        self.dialog = None;
                    }
                    egui_file::State::Selected => {
                        if let Some(path) = file_dialog.path() {
                            match validate_engine_path(path) {
                                Ok(dep) => {
                                    start.add_engine(dep.clone());
                                    self.dialog = None;
                                    self.engine = Some(dep);
                                }
                                Err(err) => {
                                    self.dialog = Some(NewProjectDialog::Error(ErrorDialog {
                                        title: format!(
                                            "Failed to add engine from path '{}'",
                                            path.display()
                                        ),
                                        message: err.to_string(),
                                    }));
                                }
                            }
                        } else {
                            self.dialog = None;
                        }
                    }
                }
            }
        }

        if close_dialog {
            return Some(None);
        }

        if create_project {
            let result = Project::new(
                Ident::from_str(&self.name).unwrap(),
                &self.path.as_ref().unwrap().join(&self.name),
                self.engine.clone().unwrap(),
                true,
            );

            match result {
                Ok(project) => {
                    return Some(Some(project));
                }
                Err(err) => {
                    self.dialog = Some(NewProjectDialog::Error(ErrorDialog {
                        title: "Failed to create project".to_owned(),
                        message: err.to_string(),
                    }));
                }
            }
        }

        None
    }
}

fn display_dependency(dep: &Dependency) -> String {
    match dep {
        Dependency::Crates(v) => {
            format!("{}{}", egui_phosphor::regular::PACKAGE, v)
        }
        Dependency::Git { git, branch: None } => {
            if let Some(suffix) = git.strip_prefix("https://github.com/") {
                format!("{}{}", egui_phosphor::regular::GITHUB_LOGO, suffix)
            } else if let Some(suffix) = git.strip_prefix("https://gitlab.com/") {
                format!("{}{}", egui_phosphor::regular::GITLAB_LOGO, suffix)
            } else {
                git.clone()
            }
        }
        Dependency::Git {
            git,
            branch: Some(branch),
        } => {
            if let Some(suffix) = git.strip_prefix("https://github.com/") {
                format!(
                    "{}{}{}{}",
                    egui_phosphor::regular::GITHUB_LOGO,
                    suffix,
                    egui_phosphor::regular::GIT_BRANCH,
                    branch
                )
            } else if let Some(suffix) = git.strip_prefix("https://gitlab.com/") {
                format!(
                    "{}{}{}{}",
                    egui_phosphor::regular::GITLAB_LOGO,
                    suffix,
                    egui_phosphor::regular::GIT_BRANCH,
                    branch
                )
            } else {
                format!("{}{}{}", git, egui_phosphor::regular::GIT_BRANCH, branch)
            }
        }
        Dependency::Path { path } => {
            format!("{}{}", egui_phosphor::regular::FILE_CODE, path)
        }
    }
}
