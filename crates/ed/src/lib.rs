use std::{path::Path, process::Child};

use arcana::{gametime::FrequencyNumExt, plugin::GLOBAL_CHECK, project::Project};
use games::GamesTab;
use parking_lot::Mutex;
use winit::event_loop::{ControlFlow, EventLoopBuilder};

use crate::{app::UserEvent, games::Games};

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

    let mut path = path.to_owned();
    assert!(path.pop());
    assert!(path.pop());
    path.push("Arcana.toml");
    let project = Project::open(&path)?;

    let event_collector = egui_tracing::EventCollector::default();

    use tracing_subscriber::layer::SubscriberExt as _;

    if let Err(err) = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish()
            .with(tracing_error::ErrorLayer::default())
            .with(event_collector.clone()),
    ) {
        panic!("Failed to install tracing subscriber: {}", err);
    }

    let mut clock = Clock::new();
    let mut limiter = FrequencyTicker::new(120u64.hz());

    let events = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let mut app = app::App::new(&events, event_collector, project);

    events.run(move |event, events, flow| {
        let step = clock.step().step;
        limiter.ticks(step);

        app.on_event(event, events);

        if app.should_quit() {
            *flow = ControlFlow::Exit;
            return;
        }

        let until = clock.stamp_instant(limiter.next_tick_stamp(clock.now()).unwrap());
        *flow = ControlFlow::WaitUntil(until)
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
