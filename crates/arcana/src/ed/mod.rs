use std::path::Path;

use arcana_project::Project;

use crate::{
    events::EventLoop,
    gametime::{Clock, FrequencyNumExt, FrequencyTicker},
};

mod app;
mod game;
mod plugins;

/// Runs the editor application
pub fn run(path: &impl AsRef<Path>) {
    if let Err(err) = _run(path.as_ref()) {
        eprintln!("Error: {}", err);
    }
}

fn _run(path: &Path) -> miette::Result<()> {
    let project = Project::open(path)?;

    crate::install_tracing_subscriber();
    let mut clock = Clock::new();
    let mut limiter = FrequencyTicker::new(30u64.hz(), clock.now());

    EventLoop::run(|events| async move {
        let mut app = app::App::new(&events, project).unwrap();

        loop {
            let deadline = clock.stamp_instant(limiter.next_tick().unwrap());

            for event in events.next(Some(deadline)).await {
                app.on_event(event);
            }

            if app.should_quit() {
                drop(app);
                return;
            }

            let step = clock.step();
            if limiter.step_tick_count(step.step) > 0 {
                app.tick(&events);
                app.render();
            }
        }
    })
}
