use std::sync::Arc;

use blink_alloc::Blink;
use edict::{
    world::WorldBuilder, ActionBuffer, ActionEncoder, Component, Entities, Scheduler, World,
};
use futures::Future;
use gametime::{Clock, ClockStep, FrequencyNumExt, FrequencyTicker, TimeSpan, TimeStamp};
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};

use crate::{
    events::{Event, EventLoopBuilder},
    funnel::{Filter, Funnel},
};

/// Configuration for the game.
pub struct Game {
    /// Main ECS world.
    world: World,

    /// Events funnel.
    funnel: Funnel,

    /// System variable-rate scheduler.
    var_scheduler: Option<Scheduler>,

    /// System fixed-rate scheduler.
    fixed_scheduler: Option<Scheduler>,

    /// Graphics
    #[cfg(feature = "graphics")]
    graphics: nix::Device,
}

/// Marker resource.
/// When this resource is present in the world,
/// the game will quit.
pub struct Quit;

pub struct MainWindowFilter {
    id: WindowId,
}

pub struct WindowsFilter;

/// Handler for window close event.
/// If handler is not present, the window will be destroyed.
/// If the handler returns true, the window will be destroyed.
/// Otherwise, the window will not be destroyed.
/// User-code may destroy window at any time by removeing it from the world and dropping.
#[derive(Clone, Component)]
pub struct WindowCloseHandler(Arc<dyn Fn(&World) -> bool + Send + Sync>);

impl WindowCloseHandler {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&World) -> bool + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }
}

impl Filter for WindowsFilter {
    #[inline]
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
            } => {
                let result = world
                    .query_mut::<(Entities, &Window, Option<&WindowCloseHandler>)>()
                    .try_for_each(|(e, window, handler)| {
                        if window_id == window.id() {
                            Err((e, handler.cloned()))
                        } else {
                            Ok(())
                        }
                    });

                match result {
                    Err((entity, handler)) => {
                        let remove = match handler {
                            None => true,
                            Some(handler) => (handler.0)(world),
                        };
                        if remove {
                            let _ = world.despawn(entity);
                        }
                        None
                    }
                    Ok(()) => Some(event),
                }
            }
            _ => Some(event),
        }
    }
}

impl Filter for MainWindowFilter {
    #[inline]
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Destroyed,
            } if window_id == self.id => {
                world.insert_resource(Quit);
                None
            }
            _ => Some(event),
        }
    }
}

pub struct FixedTicker(pub FrequencyTicker);

pub struct Limiter(pub FrequencyTicker);

pub fn run_game<F, Fut>(setup: F)
where
    F: FnOnce(&mut Game) -> Fut + 'static,
    Fut: Future,
{
    // Build the world.
    // Register external resources.
    let mut world_builder = WorldBuilder::new();
    world_builder.register_external::<winit::window::Window>();
    world_builder.register_external::<FrequencyTicker>();
    let mut world = world_builder.build();

    // Start global clocks and frequency ticker.
    // Set frequency ticker as a resource.
    // User can change frequency by changing the resource.
    let mut clocks = Clock::new();
    let ticker = 1u32.hz().ticker(clocks.now());
    world.insert_resource(ticker);
    world.insert_resource(ClockStep {
        now: clocks.now(),
        step: TimeSpan::ZERO,
    });

    world.insert_resource(Limiter(FrequencyTicker::new(60u32.hz(), clocks.now())));
    world.insert_resource(FixedTicker(FrequencyTicker::new(60u32.hz(), clocks.now())));

    // Run the event loop.
    EventLoopBuilder::new().run(|events| async move {
        // Create main window.
        let window = Window::new(&events).unwrap();

        // Setup the funnel
        let mut funnel = Funnel::new();
        funnel.add(MainWindowFilter { id: window.id() });
        funnel.add(WindowsFilter);

        world.spawn_external((window,));

        // Construct the game config.
        let mut game = Game {
            world,
            funnel,
            var_scheduler: None,
            fixed_scheduler: None,
        };

        // Run the app setup closure.
        setup(&mut game).await;

        let Game {
            mut world,
            mut funnel,
            mut var_scheduler,
            mut fixed_scheduler,
        } = game;

        let mut blink = Blink::new();
        let mut actions = ActionBuffer::new();
        let mut last_fixed = TimeStamp::start();

        loop {
            if world.get_resource::<Quit>().is_some() {
                return;
            }

            let deadline = world
                .get_resource::<Limiter>()
                .and_then(|limiter| limiter.0.next_tick())
                .map(|stamp| clocks.stamp_instant(stamp));

            let events = events.next(deadline).await;
            let step = clocks.step();

            *world.expect_resource_mut::<ClockStep>() = ClockStep {
                now: step.now,
                step: step.step,
            };

            world
                .get_resource_mut::<Limiter>()
                .map(|mut limiter| limiter.0.ticks(clocks.now()));

            for event in events {
                funnel.filter(&blink, &mut world, event);
            }

            blink.reset();

            if let Some(fixed_scheduler) = &mut fixed_scheduler {
                let ticks = world.expect_resource_mut::<FixedTicker>().0.ticks(step.now);

                for now in ticks {
                    debug_assert!(now <= step.now);
                    debug_assert!(now >= last_fixed);
                    let step = now - last_fixed;
                    *world.expect_resource_mut::<ClockStep>() = ClockStep { now, step };
                    if cfg!(debug_assertions) {
                        fixed_scheduler.run_sequential(&mut world)
                    } else {
                        fixed_scheduler.run_rayon(&mut world)
                    }
                    last_fixed = now;
                }
            }

            blink.reset();

            if let Some(var_scheduler) = &mut var_scheduler {
                if cfg!(debug_assertions) {
                    var_scheduler.run_sequential(&mut world)
                } else {
                    var_scheduler.run_rayon(&mut world)
                }
            }

            blink.reset();
        }
    });
}
