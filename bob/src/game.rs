use std::{collections::VecDeque, error::Error, sync::Arc};

use blink_alloc::Blink;
use edict::{world::WorldBuilder, Component, Entities, Scheduler, World};
use futures::Future;
use gametime::{
    Clock, ClockStep, FrequencyNumExt, FrequencyTicker, TimeSpan, TimeSpanNumExt, TimeStamp,
};
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};

use crate::{
    egui::{EguiFilter, EguiResource},
    events::{Event, EventLoop, EventLoopBuilder},
    funnel::{Filter, Funnel},
    render::{render_system, RenderGraph, TargetId},
    window::{BobWindow, Windows},
};

/// Configuration for the game.
pub struct Game {
    /// Main ECS world.
    pub world: World,

    /// Events funnel.
    pub funnel: Funnel,

    /// System variable-rate scheduler.
    /// Those systems run every frame.
    pub var_scheduler: Scheduler,

    /// System fixed-rate scheduler.
    /// Those systems run every fixed time interval.
    pub fixed_scheduler: Scheduler,

    /// Render target entity to render into main window.
    /// Should be initialized with entity id of the final render target
    /// of the render graph.
    /// If set to some, the engine will spawn a window
    /// and attach it to this render target.
    pub render_window: Option<TargetId>,
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
/// User-code may destroy window at any time by removing it from the world and dropping.
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

pub struct FPS {
    frames: VecDeque<TimeStamp>,
}

impl FPS {
    pub fn new() -> Self {
        FPS {
            frames: VecDeque::with_capacity(500),
        }
    }

    pub fn add(&mut self, time: TimeStamp) {
        while self.frames.len() >= 500 {
            self.frames.pop_front();
        }
        self.frames.push_back(time);
        while *self.frames.back().unwrap() > *self.frames.front().unwrap() + 30u32.seconds() {
            self.frames.pop_front();
        }
    }

    pub fn fps(&self) -> f32 {
        if self.frames.len() < 2 {
            return 0.0;
        }
        let first = *self.frames.front().unwrap();
        let last = *self.frames.back().unwrap();
        let duration = last - first;
        let average = duration / (self.frames.len() as u64 - 1);
        average.as_secs_f32().recip()
    }
}

pub fn run_game<F, Fut, E>(setup: F)
where
    F: FnOnce(Game) -> Fut + 'static,
    Fut: Future<Output = Result<Game, E>>,
    E: Error + 'static,
{
    crate::install_tracing_subscriber();

    // Build the world.
    // Register external resources.
    let mut world_builder = WorldBuilder::new();
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

    world.insert_resource(Limiter(FrequencyTicker::new(60u32.khz(), clocks.now())));
    world.insert_resource(FixedTicker(FrequencyTicker::new(1u32.hz(), clocks.now())));
    world.insert_resource(FPS::new());

    // Run the event loop.
    EventLoopBuilder::new().run(|events| async move {
        world.insert_resource(events);
        let (device, queue) = init_graphics();
        world.insert_resource(device);
        world.insert_resource(queue);
        world.insert_resource(RenderGraph::new());
        world.insert_resource(Windows {
            windows: Vec::new(),
        });
        world.insert_resource(EguiResource::new());

        // Setup the funnel
        let funnel = Funnel::new();

        // Construct the game config.
        let game = Game {
            world,
            funnel,
            var_scheduler: Scheduler::new(),
            fixed_scheduler: Scheduler::new(),
            render_window: None,
        };

        // Run the app setup closure.
        let game = setup(game).await.unwrap();

        let Game {
            mut world,
            mut funnel,
            mut var_scheduler,
            mut fixed_scheduler,
            render_window,
        } = game;

        var_scheduler.add_system(render_system);

        if let Some(render_window) = render_window {
            let world = world.local();
            let events = world.get_resource::<EventLoop>().unwrap();

            // Create main window.
            let window = Window::new(&events).unwrap();
            funnel.add(MainWindowFilter { id: window.id() });
            funnel.add(WindowsFilter);
            funnel.add(EguiFilter);

            let surface = world
                .expect_resource_mut::<nix::Device>()
                .new_surface(&window, &window)
                .unwrap();

            world
                .get_resource_mut::<EguiResource>()
                .unwrap()
                .add_window(window.id(), &events);

            drop(events);

            world
                .expect_resource_mut::<Windows>()
                .windows
                .push(BobWindow::new(window, surface, render_window));
        }

        let mut blink = Blink::new();
        let mut last_fixed = TimeStamp::start();
        let mut events_array = Vec::new();

        loop {
            if world.get_resource::<Quit>().is_some() {
                return;
            }

            let mut world = world.local();
            let events = world.get_resource::<EventLoop>().unwrap();

            let deadline = world
                .get_resource::<Limiter>()
                .and_then(|limiter| limiter.0.next_tick());

            events_array.extend(events.next(deadline.map(|s| clocks.stamp_instant(s))).await);
            drop(events);

            for event in events_array.drain(..) {
                funnel.filter(&blink, &mut world, event);
            }
            blink.reset();

            let clock_step = clocks.step();

            *world.expect_resource_mut::<ClockStep>() = ClockStep {
                now: clock_step.now,
                step: clock_step.step,
            };

            let ticks = world
                .expect_resource_mut::<FixedTicker>()
                .0
                .ticks(clock_step.now);

            for now in ticks {
                debug_assert!(now <= clock_step.now);
                debug_assert!(now >= last_fixed);
                let step = now - last_fixed;
                *world.expect_resource_mut::<ClockStep>() = ClockStep { now, step };
                if cfg!(debug_assertions) {
                    fixed_scheduler.run_sequential(&mut world);
                } else {
                    fixed_scheduler.run_rayon(&mut world);
                }
                last_fixed = now;
                blink.reset();
            }

            let ticks = world
                .expect_resource_mut::<Limiter>()
                .0
                .ticks(clock_step.now)
                .count();

            if ticks > 0 {
                world.expect_resource_mut::<FPS>().add(clock_step.now);

                if cfg!(debug_assertions) {
                    var_scheduler.run_sequential(&mut world);
                } else {
                    var_scheduler.run_rayon(&mut world);
                }
                blink.reset();
            }
        }
    });
}

#[cfg(feature = "graphics")]
fn init_graphics() -> (nix::Device, nix::Queue) {
    let instance = nix::Instance::load().expect("Failed to init graphics");

    let (device, mut queues) = instance
        .create(nix::DeviceDesc {
            idx: 0,
            queue_infos: &[0],
            features: nix::Features::SURFACE,
        })
        .unwrap();
    let queue = queues.pop().unwrap();
    (device, queue)
}
