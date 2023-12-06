use std::{
    io::{ErrorKind, Write},
    path::Path,
    process::Child,
};

use arcana::{gametime::FrequencyNumExt, plugin::GLOBAL_CHECK, project::Project};
use data::ProjectData;
use games::GamesTab;
use parking_lot::Mutex;
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};

use crate::app::UserEvent;

pub use arcana::*;

/// Result::ok, but logs Err case.
macro_rules! ok_log_err {
    ($res:expr) => {
        match { $res } {
            Ok(ok) => Some(ok),
            Err(err) => {
                tracing::error!("{err}");
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
                tracing::error!("{err}");
                return $($ret)?;
            }
        }
    };
}

mod app;
mod console;
mod data;
mod filters;
mod games;
mod ide;
// mod memory;
mod plugins;
mod systems;

/// Editor tab.
#[derive(serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    Console,
    Systems,
    Filters,
    Game {
        #[serde(skip)]
        tab: GamesTab,
    },
    // Memory,
}

/// Runs the editor application
pub fn run(path: &Path) {
    if let Err(err) = _run(path) {
        eprintln!("Error: {}", err);
    }
}

fn _run(path: &Path) -> miette::Result<()> {
    // Marks the running instance of Arcana library.
    // This flag is checked in plugins to ensure they are linked to this arcana.
    GLOBAL_CHECK.store(true, std::sync::atomic::Ordering::SeqCst);

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

    let mut clock = Clock::new();

    let mut limiter = clock.ticker(120.hz());

    let events = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let mut app = app::App::new(&events, event_collector, project, data);

    events.run(move |event, _events, flow| match event {
        Event::WindowEvent { window_id, event } => {
            app.on_event(window_id, event);
        }
        Event::MainEventsCleared => {
            let step = clock.step();

            app.tick(step);

            if app.should_quit() {
                *flow = ControlFlow::Exit;
                return;
            }

            limiter.ticks(step.step);
            let until = clock.stamp_instant(limiter.next_tick().unwrap());
            flow.set_wait_until(until);
        }
        Event::RedrawEventsCleared => {
            app.render();
        }
        _ => {}
    })
}

static SUBPROCESSES: Mutex<Vec<Child>> = Mutex::new(Vec::new());

fn move_element<T>(slice: &mut [T], from_index: usize, to_index: usize) {
    if from_index == to_index {
        return;
    }
    if from_index < to_index {
        let sub = &mut slice[from_index..=to_index];
        sub.rotate_left(1);
    } else {
        let sub = &mut slice[to_index..=from_index];
        sub.rotate_right(1);
    }
}

fn sync_project(project: &Project, data: &ProjectData) -> miette::Result<()> {
    project.sync()?;

    let path = project.root_path().join("Arcana.bin");

    let mut file = match std::fs::File::create(path) {
        Ok(file) => file,
        Err(err) => {
            miette::bail!("Failed to create Arcana.bin to store project data: {}", err);
        }
    };

    match bincode::serialize(data) {
        Ok(bytes) => match file.write_all(&bytes) {
            Ok(()) => Ok(()),
            Err(err) => {
                miette::bail!("Failed to write project data: {}", err);
            }
        },
        Err(err) => {
            miette::bail!("Failed to serialize project data: {}", err);
        }
    }
}

fn load_project(path: &Path) -> miette::Result<(Project, ProjectData)> {
    let project = Project::open(path)?;

    let path = project.root_path().join("Arcana.bin");

    let data = match std::fs::File::open(path) {
        Err(err) if err.kind() == ErrorKind::NotFound => ProjectData::default(),
        Ok(file) => match bincode::deserialize_from(file) {
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
