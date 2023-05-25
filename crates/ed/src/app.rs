use std::path::Path;

use bob::{
    blink_alloc::BlinkAlloc,
    edict::World,
    egui::{EguiRender, EguiResource},
    events::{Event, EventLoop},
    init_nix, nix,
    render::{render, RenderGraph, RenderResources},
    winit::{
        event::WindowEvent,
        window::{Window, WindowBuilder, WindowId},
    },
};
use egui::{Context, Ui};
use hashbrown::HashMap;

use crate::project::{Project, ProjectInstance};

enum LastStatus {
    None,
    Error(String),
    Info(String),
}

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    /// Loaded project.
    project: Option<Project>,

    // Running project instance.
    instance: Option<ProjectInstance>,

    /// Windows opened in the editor.
    windows: Vec<Window>,

    /// Views opened in the editor.
    views: HashMap<WindowId, Vec<View>>,

    /// Contains various resources.
    world: World,

    graph: RenderGraph,
    resources: RenderResources,
    device: nix::Device,
    queue: nix::Queue,

    blink: BlinkAlloc,

    last_status: LastStatus,
}

/// Editor view correspond to the open views.
/// View belong to a window.
/// Each view knows what to render.
struct View {
    render: Box<dyn FnMut(&World, &mut Ui)>,
}

fn no_project_view(cx: &Context, last_status: &mut LastStatus) -> Option<Project> {
    let r = egui::Window::new("No projects")
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .pivot(egui::Align2::CENTER_CENTER)
        .resizable(false)
        .collapsible(false)
        .show(cx, |ui| {
            if ui.button("New project").clicked() {
                let picked = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .save_file();

                if let Some(folder) = picked {
                    if let Some(name) = folder.file_name().and_then(|name| name.to_str()) {
                        match Project::new(name.to_owned(), &folder) {
                            Ok(project) => {
                                *last_status =
                                    LastStatus::Info(format!("Opened project: {}", project.name()));
                                return Some(project);
                            }
                            Err(err) => {
                                *last_status =
                                    LastStatus::Error(format!("Failed to create project: {}", err));
                            }
                        }
                    } else {
                        *last_status = LastStatus::Error(format!(
                            "Failed to create project: invalid folder name"
                        ));
                    }
                }
            }

            if ui.button("Open project").clicked() {
                let picked = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .pick_folder();

                if let Some(folder) = picked {
                    match Project::open(&folder) {
                        Ok(project) => {
                            *last_status =
                                LastStatus::Info(format!("Opened project: {}", project.name()));
                            return Some(project);
                        }
                        Err(err) => {
                            *last_status =
                                LastStatus::Error(format!("Failed to open project: {}", err));
                        }
                    }
                }
            }

            None
        })
        .unwrap();

    r.inner.flatten()
}

fn status_panel(cx: &Context, last_status: &LastStatus) {
    egui::TopBottomPanel::bottom("status").show(cx, |ui| match last_status {
        LastStatus::None => ui.label("..."),
        LastStatus::Error(err) => ui.colored_label(egui::Color32::RED, err),
        LastStatus::Info(info) => ui.label(info),
    });
}

impl App {
    pub fn new(events: &EventLoop) -> miette::Result<Self> {
        let window = WindowBuilder::new()
            .with_title("Ed")
            .build(events)
            .map_err(|err| miette::miette!("Failed to Ed window: {}", err))?;

        let (device, queue) = init_nix();
        let mut world = World::new();

        let mut egui = EguiResource::new();
        egui.add_window(&window, events);

        world.insert_resource(egui);

        let mut graph = RenderGraph::new();

        let target =
            EguiRender::build(&mut graph, window.id(), nix::ClearColor(0.2, 0.2, 0.2, 1.0));
        graph.present(target, window.id());

        Ok(App {
            project: None,
            instance: None,
            windows: vec![window],
            views: HashMap::new(),
            world,
            graph,
            resources: RenderResources::default(),
            device,
            queue,
            blink: BlinkAlloc::new(),
            last_status: LastStatus::None,
        })
    }

    pub fn open_project(&mut self, path: &Path) -> miette::Result<()> {
        let project = Project::open(path)?;
        self.project = Some(project);
        Ok(())
    }

    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::WindowEvent { window_id, event } => {
                let local = self.world.local();
                let mut egui = local.expect_resource_mut::<EguiResource>();
                egui.handle_event(window_id, &event);

                match event {
                    WindowEvent::CloseRequested => {
                        self.views.remove(&window_id);
                        if let Some(idx) = self.windows.iter().position(|w| w.id() == window_id) {
                            self.windows.swap_remove(idx);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        if let Some(instance) = &mut self.instance {
            instance.tick();
        }

        if self.windows.is_empty() {
            return;
        }

        let world = self.world.local();
        let mut egui = world.expect_resource_mut::<EguiResource>();

        match &mut self.project {
            None => {
                let window = &self.windows[0];
                if let Some(Some(project)) = egui.run(window, |cx| {
                    status_panel(cx, &self.last_status);
                    no_project_view(cx, &mut self.last_status)
                }) {
                    self.project = Some(project);
                }
            }
            Some(project) => {
                let mut quit = false;
                for window in &self.windows {
                    let mut dummy = Vec::new();
                    let views = self.views.get_mut(&window.id()).unwrap_or(&mut dummy);
                    egui.run(window, |cx| {
                        status_panel(cx, &self.last_status);

                        egui::TopBottomPanel::top("top").show(cx, |ui| {
                            ui.horizontal(|ui| {
                                ui.menu_button("File", |ui| {
                                    if ui.button("Quit").clicked() {
                                        quit = true;
                                        ui.close_menu();
                                    }
                                });

                                ui.menu_button("Edit", |ui| {
                                    ui.menu_button("Add plugin library", |ui| {
                                        if ui.button("Path").clicked() {
                                            let picked = rfd::FileDialog::new()
                                                .set_directory(std::env::current_dir().unwrap())
                                                .pick_folder();

                                            if let Some(folder) = picked {
                                                match project.add_library_path(&folder) {
                                                    Ok(()) => {
                                                        self.last_status =
                                                            LastStatus::Info(format!(
                                                                "Added library: {}",
                                                                folder.display()
                                                            ));
                                                    }
                                                    Err(err) => {
                                                        self.last_status =
                                                            LastStatus::Error(format!(
                                                                "Failed to add library: {}",
                                                                err
                                                            ));
                                                    }
                                                }
                                            }
                                            ui.close_menu();
                                        }
                                    });
                                });
                            });
                        });

                        egui::SidePanel::left("plugins").show(cx, |ui| {
                            let mut build_clicked = false;
                            let mut launch_clicked = false;

                            if let Some(library) = &mut self.instance {
                                ui.horizontal(|ui| {
                                    build_clicked = ui.button("Rebuild").clicked();
                                    launch_clicked = ui.button("Launch").clicked();
                                });
                                ui.separator();
                                ui.heading("Plugins");
                                for (lib, plugins) in library.plugins_enabled_mut() {
                                    ui.separator();
                                    ui.heading(lib);
                                    for (plugin, enabled) in plugins {
                                        ui.checkbox(enabled, plugin);
                                    }
                                }
                            } else {
                                build_clicked = ui.button("Build").clicked();
                            };

                            if build_clicked {
                                match project.build(&mut self.instance) {
                                    Err(err) => {
                                        self.last_status = LastStatus::Error(format!(
                                            "Failed to build project: {}",
                                            err
                                        ));
                                    }
                                    Ok(()) => {
                                        self.last_status =
                                            LastStatus::Info("Project build succeeded".into());
                                    }
                                }
                            }

                            if launch_clicked {
                                self.instance.as_mut().unwrap().launch();
                            }
                        });

                        egui::CentralPanel::default().show(cx, |ui| {
                            for view in views {
                                (view.render)(&world, ui);
                            }
                        });
                    });
                }
            }
        }
    }

    pub fn render(&mut self) {
        if self.windows.is_empty() {
            return;
        }

        render(
            &mut self.graph,
            &self.device,
            &mut self.queue,
            &self.blink,
            None,
            self.windows.iter(),
            &self.world,
            &mut self.resources,
        );
    }

    pub fn should_quit(&self) -> bool {
        self.windows.is_empty()
    }
}
