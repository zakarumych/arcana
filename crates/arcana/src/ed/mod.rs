use std::path::Path;

use arcana_project::Project;

mod app;
mod console;
mod game;
mod plugins;

/// Editor tab.
#[derive(serde::Serialize, serde::Deserialize)]
enum Tab {
    Plugins,
    Console,
}

/// Runs the editor application
pub fn run(path: &Path) {
    if let Err(err) = _run(path) {
        eprintln!("Error: {}", err);
    }
}

fn _run(path: &Path) -> miette::Result<()> {
    let mut path = path.to_owned();
    assert!(path.pop());
    assert!(path.pop());
    path.push("Arcana.toml");
    let project = Project::open(&path)?;
    crate::app::try_run(|events, event_collector| app::App::new(events, event_collector, project))
}
