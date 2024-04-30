use std::{collections::VecDeque, ptr::NonNull, sync::Arc, time::Instant};

use arcana_project::Ident;
use blink_alloc::Blink;
use edict::{
    atomicell::Ref,
    flow::{execute_flows, Flows},
    world::WorldBuilder,
    Component, EntityId, IntoSystem, System, World,
};
use gametime::{
    Clock, ClockStep, Frequency, FrequencyNumExt, FrequencyTicker, TimeSpan, TimeSpanNumExt,
    TimeStamp,
};
use hashbrown::{HashMap, HashSet};
use mev::ImageDesc;
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};

use crate::{
    events::{Event, EventFilter, EventFunnel},
    flow::{init_flows, wake_flows},
    init_mev,
    plugin::{ArcanaPlugin, PluginsHub},
    render::{render_system, RenderGraph, RenderState},
    viewport::Viewport,
    work::WorkGraph,
};

/// Marker resource.
/// When this resource is present in the world,
/// the game will quit.
#[derive(Debug)]
pub struct Quit;

pub struct FixedTicker(pub FrequencyTicker);

pub struct Limiter(pub FrequencyTicker);

pub struct FPS {
    frames: VecDeque<TimeStamp>,
}

impl FPS {
    pub fn new() -> Self {
        FPS {
            frames: VecDeque::with_capacity(5000),
        }
    }

    pub fn add(&mut self, time: TimeStamp) {
        while self.frames.len() == self.frames.capacity() {
            self.frames.pop_front();
        }
        self.frames.push_back(time);
        while *self.frames.back().unwrap() > *self.frames.front().unwrap() + 30.seconds() {
            self.frames.pop_front();
        }
    }

    pub fn average(&self) -> f32 {
        if self.frames.len() < 2 {
            return 0.0;
        }
        let first = *self.frames.front().unwrap();
        let last = *self.frames.back().unwrap();
        let duration = last - first;
        let average = duration / (self.frames.len() as u64 - 1);
        average.as_secs_f32().recip()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = TimeStamp> + '_ {
        self.frames.iter().copied()
    }
}

/// Game clock that uses global clock steps
/// and apply necessary adjustments to produce game clock steps.
pub struct GameClock {
    nom: u64,
    denom: u64,
    until_next: u64,
    now: TimeStamp,
}

impl GameClock {
    pub fn new() -> Self {
        GameClock {
            nom: 1,
            denom: 1,
            until_next: 0,
            now: TimeStamp::start(),
        }
    }

    pub fn pause(&mut self) {
        self.nom = 0;
    }

    pub fn set_rate(&mut self, rate: f32) {
        let (nom, denom) = rate2ratio(rate);
        self.nom = nom;
        self.denom = denom;
    }

    pub fn get_rate(&self) -> f64 {
        self.nom as f64 / self.denom as f64
    }

    pub fn set_rate_ratio(&mut self, nom: u64, denom: u64) {
        self.nom = nom;
        self.denom = denom;
    }

    pub fn get_rate_ratio(&mut self) -> (u64, u64) {
        (self.nom, self.denom)
    }

    pub fn with_rate(rate: f32) -> Self {
        let (nom, denom) = rate2ratio(rate);
        GameClock {
            nom,
            denom,
            until_next: denom,
            now: TimeStamp::start(),
        }
    }

    pub fn with_rate_ratio(nom: u64, denom: u64) -> Self {
        GameClock {
            nom,
            denom,
            until_next: denom,
            now: TimeStamp::start(),
        }
    }

    pub fn update(&mut self, span: TimeSpan) -> ClockStep {
        let nanos = span.as_nanos();
        let nom_nanos = nanos * self.nom;

        if self.until_next > nom_nanos {
            // Same game nanosecond.
            self.until_next -= nom_nanos;
            return ClockStep {
                now: self.now,
                step: TimeSpan::ZERO,
            };
        }

        let game_nanos = (nom_nanos - self.until_next) / self.denom;
        let nom_nanos_left = (nom_nanos - self.until_next) % self.denom;
        self.until_next = self.denom - nom_nanos_left;

        let game_span = TimeSpan::new(game_nanos);
        self.now += game_span;

        ClockStep {
            now: self.now,
            step: game_span,
        }
    }
}

pub struct Game {
    clock: GameClock,
    world: World,
    blink: Blink,
    fixed_now: TimeStamp,
    scheduler: Box<dyn FnMut(&mut World, bool)>,
    flows: Flows,

    funnel: Funnel,

    render_state: RenderState,
    work_graph: WorkGraph,
}

impl Component for Game {
    fn name() -> &'static str {
        "Game"
    }
}

pub type Scheduler = Box<dyn FnMut(&mut World, bool)>;
pub type Funnel = Box<dyn FnMut(&Blink, &mut World, &Event) -> bool>;

pub struct GameInit {
    pub scheduler: Scheduler,
    pub funnel: Funnel,
}

impl Game {
    pub fn launch<'a>(
        init: impl FnOnce(&mut World) -> GameInit,
        device: mev::Device,
        queue: Arc<Mutex<mev::Queue>>,
        window: Option<Window>,
    ) -> Self {
        let mut world = World::new();

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

        world.insert_resource(FixedTicker(clocks.ticker(120.hz())));

        {
            // Register external resources.
            world.ensure_external_registered::<mev::Surface>();
            world.ensure_external_registered::<Window>();

            // world.insert_resource(Limiter(clocks.ticker(120.hz())));
            world.insert_resource(FPS::new());
            world.insert_resource(RenderGraph::new());
            world.insert_resource(device);
            world.insert_resource(queue);

            if let Some(window) = window {
                // If window is provided, register it as a resource.
                // Quit when the window is closed.
                world.insert_resource(Viewport::new_window(window));
            } else {
                world.insert_resource(Viewport::new_texture());
            };
        }

        let blink = Blink::new();
        let fixed_now = TimeStamp::start();

        init_flows(&mut world);

        let init = init(&mut world);

        tracing::debug!("Game initialized");

        Game {
            clock: GameClock::new(),
            world,
            blink,
            fixed_now,
            scheduler: init.scheduler,
            flows: Flows::default(),

            funnel: init.funnel,

            render_state: RenderState::default(),
            work_graph: WorkGraph::new(HashMap::new(), HashSet::new()).unwrap(),
        }
    }

    pub fn pause(&mut self) {
        self.clock.pause();
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.clock.set_rate(rate);
    }

    pub fn get_rate(&self) -> f64 {
        self.clock.get_rate()
    }

    pub fn set_rate_ratio(&mut self, nom: u64, denom: u64) {
        self.clock.set_rate_ratio(nom, denom);
    }

    pub fn get_rate_ratio(&mut self) -> (u64, u64) {
        self.clock.get_rate_ratio()
    }

    pub fn window_id(&self) -> Option<WindowId> {
        self.world.get_resource::<Window>().map(|w| w.id())
    }

    pub fn on_event(&mut self, event: &Event) -> bool {
        (self.funnel)(&self.blink, &mut self.world, event)
    }

    pub fn quit(&mut self) {
        self.world.insert_resource(Quit);
    }

    pub fn should_quit(&self) -> bool {
        self.world.get_resource::<Quit>().is_some()
    }

    pub fn render(&mut self, now: TimeStamp) {
        self.world.expect_resource_mut::<FPS>().add(now);

        // Just run the render system.
        render_system(&mut self.world, (&mut self.render_state).into())
    }

    /// Render the game to a texture.
    ///
    /// Returns image to which main presentation happens.
    pub fn render_with_texture(
        &mut self,
        extent: mev::Extent2,
    ) -> Result<mev::Image, mev::OutOfMemory> {
        let mut viewport = self.world.expect_resource_mut::<Viewport>();

        #[cold]
        fn new_image(
            viewport: &mut Viewport,
            extent: mev::Extent2,
            world: &World,
        ) -> Result<(), mev::OutOfMemory> {
            let device = world.expect_resource::<mev::Device>();
            let image = device.new_image(mev::ImageDesc {
                dimensions: extent.into(),
                format: mev::PixelFormat::Rgba8Srgb,
                usage: mev::ImageUsage::TARGET | mev::ImageUsage::SAMPLED,
                layers: 1,
                levels: 1,
                name: "Game Viewport",
            })?;
            viewport.set_image(image);
            Ok(())
        }

        if viewport
            .get_image()
            .map_or(true, |i| i.dimensions() != extent)
        {
            tracing::debug!("Creating new image for viewport");

            new_image(&mut *viewport, extent, &self.world)?;
        }

        Ok(viewport.get_image().unwrap().clone())
    }

    pub fn tick(&mut self, step: ClockStep) {
        let step = self.clock.update(step.step);

        self.blink.reset();

        let ticks = self
            .world
            .expect_resource_mut::<FixedTicker>()
            .0
            .ticks(step.step);

        for fixed_stamp in ticks {
            let fixed_step = ClockStep {
                now: fixed_stamp,
                step: fixed_stamp - self.fixed_now,
            };
            self.fixed_now = fixed_stamp;

            *self.world.expect_resource_mut::<ClockStep>() = fixed_step;
            (self.scheduler)(&mut self.world, true);
            self.blink.reset();

            wake_flows(&mut self.world);
            execute_flows(&mut self.world, &mut self.flows);
        }

        {
            *self.world.expect_resource_mut::<ClockStep>() = step;

            (self.scheduler)(&mut self.world, false);
            self.blink.reset();
        }
    }

    pub fn fps(&self) -> Ref<'_, FPS> {
        self.world.expect_resource::<FPS>()
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

fn rate2ratio(rate: f32) -> (u64, u64) {
    let denom = 6469693230;
    let nom = (rate.max(0.0) * 6469693230.0).floor() as u64;

    let gcd = gcd(nom, denom);

    let nom = nom / gcd;
    let denom = denom / gcd;
    (nom, denom)
}
