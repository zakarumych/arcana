use std::{fmt::Display, path::Path};

use arcana_project::Project;

use crate::{
    events::EventLoop,
    gametime::{Clock, FrequencyNumExt, FrequencyTicker},
};

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
mod game;
mod plugins;

trait ResultExt {
    fn log_err(self);
}

impl<E> ResultExt for Result<(), E>
where
    E: Display,
{
    fn log_err(self) {
        if let Err(err) = self {
            tracing::error!("{err}");
        }
    }
}

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
    path.push("Arcana.toml");

    let project = Project::open(&path)?;

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
