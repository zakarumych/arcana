use std::path::Path;

use arcana::{
    events::EventLoop,
    gametime::{Clock, FrequencyNumExt, FrequencyTicker},
};
use project::Project;

#[doc(hidden)]
pub mod api;
mod app;
pub mod project;

/// Runs the editor application
pub fn run(path: &Path) -> miette::Result<()> {
    arcana::install_tracing_subscriber();

    let project = match Project::open(path) {
        Ok(project) => project,
        Err(err) => {
            miette::bail!("Failed to open project: {err}");
        }
    };

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
