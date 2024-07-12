#![forbid(unsafe_op_in_unsafe_fn)]
#![feature(float_next_up_down)]

use std::{hash::Hash, io::ErrorKind, path::Path};

use arcana::{
    mev,
    project::{Profile, Project},
};
use data::ProjectData;
use winit::event_loop::EventLoop;

#[cfg(windows)]
use winit::platform::windows::EventLoopBuilderExtWindows;

use crate::app::UserEvent;

/// Result::ok, but logs Err case.
macro_rules! ok_log_err {
    ($res:expr) => {
        match { $res } {
            Ok(ok) => Some(ok),
            Err(err) => {
                tracing::error!("{err:?}");
                None
            }
        }
    };
}

/// Unwraps Result::Ok and returns if it is Err case.
/// Returns with provided expression if one specified.
macro_rules! try_log_err {
    ($res:expr $(; $ret:expr)?) => {
        match {$res} {
            Ok(ok) => ok,
            Err(err) => {
                tracing::error!("{err:?}");
                return $($ret)?;
            }
        }
    };
}

mod app;
// mod console;
mod data;
mod filters;
mod ide;
mod render;
// mod memory;
mod code;
mod container;
mod error;
mod instance;
mod model;
// mod monitor;
mod inspector;
mod plugins;
mod sample;
mod subprocess;
mod systems;
mod tool;
mod ui;

/// Runs the editor application
pub fn run(path: &Path) {
    if let Err(err) = _run(path) {
        eprintln!("Error: {}", err);
    }
}

fn _run(path: &Path) -> miette::Result<()> {
    // Marks the running instance of Arcana library.
    // This flag is checked in plugins to ensure they are linked to this arcana.
    arcana::plugin::set_running_arcana_instance();

    // `path` is `<project-dir>/crates/ed`
    let mut path = path.to_owned();
    assert!(path.file_name().unwrap() == "ed");
    assert!(path.pop());
    assert!(path.file_name().unwrap() == "crates");
    assert!(path.pop());

    // `path` is `<project-dir>`
    let (project, data) = load_project(&path)?;

    let event_collector = egui_tracing::EventCollector::default();

    use tracing_subscriber::layer::SubscriberExt as _;

    if let Err(err) = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            // .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish()
            .with(tracing_error::ErrorLayer::default())
            .with(event_collector.clone()),
    ) {
        panic!("Failed to install tracing subscriber: {}", err);
    }

    let mut builder = EventLoop::<UserEvent>::with_user_event();

    #[cfg(windows)]
    builder.with_any_thread(true);

    let events = builder.build().expect("Failed to create event loop");
    let mut app = app::App::new(event_collector, project, data);

    events.run_app(&mut app).unwrap();

    Ok(())
}

// fn move_element<T>(slice: &mut [T], from_index: usize, to_index: usize) {
//     if from_index == to_index {
//         return;
//     }
//     if from_index < to_index {
//         let sub = &mut slice[from_index..=to_index];
//         sub.rotate_left(1);
//     } else {
//         let sub = &mut slice[to_index..=from_index];
//         sub.rotate_right(1);
//     }
// }

fn load_project(path: &Path) -> miette::Result<(Project, ProjectData)> {
    let project = Project::open(path)?;

    let path = project.root_path().join("Arcana.bin");

    let data = match std::fs::File::open(path) {
        Err(err) if err.kind() == ErrorKind::NotFound => ProjectData::default(),
        Ok(file) => match serde_json::from_reader(file) {
            Ok(data) => data,
            Err(err) => {
                miette::bail!("Failed to deserialize project data: {}", err);
            }
        },
        Err(err) => {
            miette::bail!("Failed to open Arcana.bin to load project data: {}", err);
        }
    };

    Ok((project, data))
}

fn toggle_ui(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact(&response);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    response
}

fn get_profile() -> Profile {
    let s = std::env::var("ARCANA_PROFILE").expect("ARCANA_PROFILE environment variable unset");
    match &*s {
        "release" => Profile::Release,
        "debug" => Profile::Debug,
        _ => panic!("Invalid profile: {}", s),
    }
}

fn init_mev() -> (mev::Device, mev::Queue) {
    let instance = mev::Instance::load().expect("Failed to init graphics");

    let (device, mut queues) = instance
        .create(mev::DeviceDesc {
            idx: 0,
            queues: &[0],
            features: mev::Features::SURFACE,
        })
        .unwrap();
    let queue = queues.pop().unwrap();
    (device, queue)
}

fn hue_hash<T>(value: &T) -> egui::Color32
where
    T: Hash + ?Sized,
{
    let [r, g, b] = ::arcana::hash::hue_hash(value);
    egui::Color32::from_rgb(r, g, b)
}
