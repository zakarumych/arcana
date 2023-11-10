use std::{path::Path, process::Child};

use arcana_project::Project;
use parking_lot::Mutex;

use crate::plugin::GLOBAL_CHECK;

mod app;
mod console;
mod game;
mod ide;
mod memory;
mod plugins;
mod systems;

/// Editor tab.
#[derive(serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    Console,
    Systems,
    Filters,
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
    crate::app::try_run(|events, event_collector| app::App::new(events, event_collector, project))
}

static SUBPROCESSES: Mutex<Vec<Child>> = Mutex::new(Vec::new());
