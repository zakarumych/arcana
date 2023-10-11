use std::fmt;

use egui_tracing::EventCollector;

use crate::{
    events::{Event, EventLoop},
    gametime::{Clock, FrequencyNumExt, FrequencyTicker},
};

#[macro_export]
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

#[macro_export]
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

/// Extension trait for `Result` type to log error when Ok value is unit.
pub trait ResultExt {
    fn log_err(self);
}

impl<E> ResultExt for Result<(), E>
where
    E: fmt::Display,
{
    fn log_err(self) {
        if let Err(err) = self {
            tracing::error!("{err}");
        }
    }
}

/// Trait for application that can be run.
pub trait Application {
    fn on_event(&mut self, event: Event) -> Option<Event>;
    fn tick(&mut self, events: &EventLoop);
    fn render(&mut self);
    fn should_quit(&self) -> bool;
}

/// Runs the editor application
pub fn run<A, F>(app: F)
where
    A: Application,
    F: FnOnce(&EventLoop, EventCollector) -> A + 'static,
{
    if let Err(err) = try_run(app) {
        eprintln!("Error: {}", err);
    }
}

pub fn try_run<A, F>(app: F) -> miette::Result<()>
where
    A: Application,
    F: FnOnce(&EventLoop, EventCollector) -> A + 'static,
{
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
    let mut limiter = FrequencyTicker::new(30u64.hz(), clock.now());

    EventLoop::run(|events| async move {
        let mut app = app(&events, event_collector);

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
