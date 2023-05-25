use std::path::PathBuf;

use bob::{
    events::EventLoop,
    gametime::{Clock, FrequencyNumExt, FrequencyTicker},
};
use clap::Parser;

mod app;
mod game;
mod project;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    project: Option<PathBuf>,
}

fn main() -> miette::Result<()> {
    bob::install_tracing_subscriber();

    let args = Args::parse();

    let mut clock = Clock::new();
    let mut limiter = FrequencyTicker::new(30u64.hz(), clock.now());

    EventLoop::run(|events| async move {
        let mut app = app::App::new(&events).unwrap();

        if let Some(path) = args.project {
            if let Err(err) = app.open_project(&path) {
                tracing::error!("Failed to open project: {}", err);
            }
        }

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
                app.tick();
                app.render();
            }
        }
    })
}
