use edict::{world::WorldBuilder, Entities, World};
use futures::Future;
use gametime::{Clock, ClockStep, FrequencyNumExt, FrequencyTicker, TimeSpan};
use winit::{event::WindowEvent, window::WindowId};

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
}

/// Marker resource.
/// When this resource is present in the world,
/// the game will quit.
pub struct Quit;

pub struct MainWindowFilter {
    id: WindowId,
}

pub struct WindowsFilter;

impl Filter for WindowsFilter {
    #[inline]
    fn filter(&mut self, world: &mut World, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
            } => {
                let mut windows = world.query_mut::<(Entities, &WindowId)>();
                windows.retain(|id| *id != window_id);
                None
            }
            _ => Some(event),
        }
    }
}

impl Filter for MainWindowFilter {
    #[inline]
    fn filter(&mut self, world: &mut World, event: Event) -> Option<Event> {
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

pub fn run_game<F, Fut>(setup: F)
where
    F: FnOnce(Game) -> Fut + 'static,
    Fut: Future<Output = Game>,
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

    // Run the event loop.
    EventLoopBuilder::new().run(|events| async move {
        // Create main window.
        let window = winit::window::Window::new(&events).unwrap();

        // Setup the funnel
        let mut funnel = Funnel::new();
        funnel.add(MainWindowFilter { id: window.id() });

        // Construct the game config.
        let game = Game { world, funnel };

        // Run the app setup closure.
        let game = setup(game).await;

        let Game {
            mut world,
            mut funnel,
        } = game;

        loop {
            let events = events
                .next_rate(&clocks, &world.expect_resource::<FrequencyTicker>())
                .await;
            let mut events = events.peekable();
            if events.peek().is_none() {
                println!("No events");
            }
            for event in events {
                println!("{:?}", event);
                funnel.filter(&mut world, event);
            }

            let step = clocks.step();
            *world.expect_resource_mut::<ClockStep>() = step;

            let ticks = world
                .expect_resource_mut::<FrequencyTicker>()
                .ticks(clocks.now());

            for tick in ticks {
                println!("Tick: {:?}", tick);
            }
        }
    });
}
