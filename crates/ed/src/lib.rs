use std::{hash::Hash, path::Path};

use arcana::{error::Error, mev};
use arcana_project::Profile;
use winit::event_loop::EventLoop;

#[cfg(windows)]
use winit::platform::windows::EventLoopBuilderExtWindows;

use crate::project::Project;

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
mod assets;
mod blobs;
// mod code;
mod container;
mod error;
mod filters;
mod ide;
mod inspector;
mod instance;
mod model;
mod plugins;
mod project;
mod render;
mod sample;
mod subprocess;
mod systems;
mod task;
mod tool;
mod ui;
mod viewport;

/// Runs the editor application
pub fn run(project_path: impl AsRef<Path>) {
    if let Err(err) = _run(project_path.as_ref()) {
        eprintln!("Error: {}", err);
    }
}

fn _run(project_path: &Path) -> Result<(), Error> {
    // Marks the running instance of Arcana library.
    // This flag is checked in plugins to ensure they are linked to this arcana.
    arcana::plugin::set_running_arcana_instance();

    let project = Project::load(project_path)?;

    // let event_collector = egui_tracing::EventCollector::default();

    use tracing_subscriber::layer::SubscriberExt as _;

    if let Err(err) = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            // .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish()
            .with(tracing_error::ErrorLayer::default()),
        // .with(event_collector.clone()),
    ) {
        panic!("Failed to install tracing subscriber: {}", err);
    }

    // basis_universal::transcoder_init();

    let mut builder = EventLoop::<app::UserEvent>::with_user_event();

    #[cfg(windows)]
    builder.with_any_thread(true);

    let events = builder
        .build()
        .expect("Event loop should be created successfully");
    let mut app = app::App::new(project);

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
        ui.painter().rect(
            rect,
            radius,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Middle,
        );
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    response
}

fn get_profile() -> Profile {
    let s =
        std::env::var("ARCANA_PROFILE").expect("ARCANA_PROFILE environment variable should be set");
    match &*s {
        "release" => Profile::Release,
        "debug" => Profile::Debug,
        _ => panic!("Invalid profile: {}", s),
    }
}

fn init_mev() -> (mev::Device, mev::Queue) {
    let instance = mev::Instance::load().expect("Graphics initialization should succeed");

    let (device, mut queues) = instance
        .new_device(mev::DeviceDesc {
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
