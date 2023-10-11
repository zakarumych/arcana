use std::{collections::VecDeque, sync::Arc};

use blink_alloc::Blink;
use edict::{world::WorldBuilder, Scheduler, World};
use gametime::{
    Clock, ClockStep, FrequencyNumExt, FrequencyTicker, TimeSpan, TimeSpanNumExt, TimeStamp,
};
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{Window, WindowBuilder, WindowId},
};

use crate::{
    egui::{EguiFilter, EguiResource},
    events::{Event, EventLoop},
    funnel::{EventFilter, EventFunnel},
    plugin::ArcanaPlugin,
    render::{render_system, RenderGraph},
};

/// Marker resource.
/// When this resource is present in the world,
/// the game will quit.
#[derive(Debug)]
pub struct Quit;

pub struct MainWindowFilter {
    id: WindowId,
}

impl EventFilter for MainWindowFilter {
    #[inline]
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
            } => {
                if window_id == self.id {
                    world.insert_resource(Quit);
                    return None;
                }
            }
            _ => {}
        }
        Some(event)
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

pub struct Game {
    clocks: Clock,
    world: World,
    funnel: EventFunnel,
    blink: Blink,
    last_fixed: TimeStamp,
    fixed_scheduler: Scheduler,
    var_scheduler: Scheduler,
}

impl Game {
    pub fn launch<'a>(
        events: &EventLoop,
        plugins: impl IntoIterator<Item = &'a dyn ArcanaPlugin>,
        device: mev::Device,
        queue: Arc<Mutex<mev::Queue>>,
    ) -> Self {
        let window = WindowBuilder::new()
            .with_title("game")
            .build(events)
            .unwrap();

        // Build the world.
        // Register external resources.
        let world_builder = WorldBuilder::new();

        let mut world = world_builder.build();

        // Start global clocks and frequency ticker.
        // Set frequency ticker as a resource.
        // User can change frequency by changing the resource.
        let clocks = Clock::new();
        // let ticker = 1u32.hz().ticker(clocks.now());
        // world.insert_resource(ticker);
        world.insert_resource(ClockStep {
            now: clocks.now(),
            step: TimeSpan::ZERO,
        });

        world.insert_resource(Limiter(FrequencyTicker::new(120u32.khz(), clocks.now())));
        world.insert_resource(FixedTicker(FrequencyTicker::new(1u32.hz(), clocks.now())));
        world.insert_resource(FPS::new());
        world.insert_resource(RenderGraph::new());
        world.insert_resource(EguiResource::new());

        world.insert_resource(device);
        world.insert_resource(queue);

        let main_window_id = window.id();

        let mut egui = EguiResource::new();
        egui.add_window(&window, events);
        world.insert_resource(egui);
        world.insert_resource(window);

        // Setup the funnel
        let mut funnel = EventFunnel::new();

        let mut var_scheduler = Scheduler::new();
        let fixed_scheduler = Scheduler::new();

        funnel.add(MainWindowFilter { id: main_window_id });
        funnel.add(EguiFilter);

        var_scheduler.add_system(render_system);

        let blink = Blink::new();
        let last_fixed = TimeStamp::start();

        for plugin in plugins {
            plugin.init(&mut world, &mut var_scheduler);
            plugin.init_funnel(&mut funnel);
        }

        Game {
            clocks,
            world,
            funnel,
            blink,
            last_fixed,
            fixed_scheduler,
            var_scheduler,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.world.get_resource::<Window>().unwrap().id()
    }

    pub fn on_event(&mut self, event: Event) -> Option<Event> {
        self.funnel.filter(&self.blink, &mut self.world, event)
    }

    pub fn should_quit(&self) -> bool {
        self.world.get_resource::<Quit>().is_some()
    }

    pub fn tick(&mut self) {
        self.blink.reset();

        let clock_step = self.clocks.step();

        *self.world.expect_resource_mut::<ClockStep>() = ClockStep {
            now: clock_step.now,
            step: clock_step.step,
        };

        let ticks = self
            .world
            .expect_resource_mut::<FixedTicker>()
            .0
            .ticks(clock_step.now);

        for now in ticks {
            debug_assert!(now <= clock_step.now);
            debug_assert!(now >= self.last_fixed);
            let step = now - self.last_fixed;
            *self.world.expect_resource_mut::<ClockStep>() = ClockStep { now, step };
            if cfg!(debug_assertions) {
                self.fixed_scheduler.run_sequential(&mut self.world);
            } else {
                self.fixed_scheduler.run_rayon(&mut self.world);
            }
            self.last_fixed = now;
            self.blink.reset();
        }

        let ticks = self
            .world
            .expect_resource_mut::<Limiter>()
            .0
            .ticks(clock_step.now)
            .count();

        if ticks > 0 {
            self.world.expect_resource_mut::<FPS>().add(clock_step.now);

            if cfg!(debug_assertions) {
                self.var_scheduler.run_sequential(&mut self.world);
            } else {
                self.var_scheduler.run_rayon(&mut self.world);
            }
            self.blink.reset();
        }
    }
}
