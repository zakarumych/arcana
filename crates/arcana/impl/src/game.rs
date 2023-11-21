use std::{collections::VecDeque, ptr::NonNull, sync::Arc, time::Instant};

use arcana_project::Ident;
use blink_alloc::Blink;
use edict::{world::WorldBuilder, Component, EntityId, IntoSystem, Scheduler, System, World};
use gametime::{
    Clock, ClockStep, Frequency, FrequencyNumExt, FrequencyTicker, TimeSpan, TimeSpanNumExt,
    TimeStamp,
};
use mev::ImageDesc;
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{Window, WindowBuilder, WindowId},
};

use crate::{
    events::{Event, EventFilter, EventFunnel},
    flow::init_flows,
    init_mev,
    plugin::{ArcanaPlugin, PluginInit},
    render::{render_system, RenderGraph, RenderState},
    viewport::Viewport,
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
            frames: VecDeque::with_capacity(500),
        }
    }

    pub fn add(&mut self, time: TimeStamp) {
        while self.frames.len() >= 500 {
            self.frames.pop_front();
        }
        self.frames.push_back(time);
        while *self.frames.back().unwrap() > *self.frames.front().unwrap() + 30.seconds() {
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
    funnel: EventFunnel,
    blink: Blink,
    fixed_now: TimeStamp,
    fixed_scheduler: Scheduler,
    var_scheduler: Scheduler,
    render_state: RenderState,
}

impl Component for Game {
    fn name() -> &'static str {
        "Game"
    }
}

impl Game {
    pub fn launch<'a>(
        plugins: impl IntoIterator<Item = (&'a Ident, &'a dyn ArcanaPlugin)>,
        filters: impl IntoIterator<Item = (&'a Ident, &'a Ident)>,
        systems: impl IntoIterator<Item = (&'a Ident, &'a Ident)>,
        device: mev::Device,
        queue: Arc<Mutex<mev::Queue>>,
        window: Option<Window>,
    ) -> Self {
        // Build the world.
        // Register external resources.
        let mut world_builder = WorldBuilder::new();
        world_builder.register_external::<mev::Surface>();
        world_builder.register_external::<Window>();

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

        world.insert_resource(Limiter(FrequencyTicker::new(120u32.khz())));
        world.insert_resource(FixedTicker(FrequencyTicker::new(1u32.hz())));
        world.insert_resource(FPS::new());
        world.insert_resource(RenderGraph::new());

        world.insert_resource(device);
        world.insert_resource(queue);

        // Setup the funnel
        let mut funnel = EventFunnel::new();

        let mut var_scheduler = Scheduler::new();
        let fixed_scheduler = Scheduler::new();

        if let Some(window) = window {
            // If window is provided, register it as a resource.
            // Quit when the window is closed.
            world.insert_resource(Viewport::new_window(window));
        } else {
            world.insert_resource(Viewport::new_texture());
        }

        let blink = Blink::new();
        let fixed_now = TimeStamp::start();

        init_flows(&mut world, &mut var_scheduler);

        struct PluginInit<'a> {
            plugin: &'a Ident,
            systems: Vec<(&'a Ident, Box<dyn System + Send>)>,
            #[cfg(feature = "client")]
            filters: Vec<(&'a Ident, Box<dyn EventFilter>)>,
        }

        let mut init_plugins = plugins
            .into_iter()
            .map(|(name, plugin)| {
                let init = plugin.init(&mut world);
                PluginInit {
                    plugin: name,
                    systems: init.systems,
                    #[cfg(feature = "client")]
                    filters: init.filters,
                }
            })
            .collect::<Vec<PluginInit>>();

        for (plugin, name) in systems {
            let p = init_plugins
                .iter_mut()
                .find(|p| p.plugin == plugin)
                .expect("Plugin not found");

            let idx = p
                .systems
                .iter()
                .position(|(system, _)| *system == name)
                .expect("System not found");

            let system = p.systems.swap_remove(idx).1;
            var_scheduler.add_boxed_system(system);
        }

        for (plugin, name) in filters {
            let p = init_plugins
                .iter_mut()
                .find(|p| p.plugin == plugin)
                .expect("Plugin not found");

            let idx = p
                .filters
                .iter()
                .position(|(filter, _)| *filter == name)
                .expect("System not found");

            let filter = p.filters.swap_remove(idx).1;
            funnel.add_boxed(filter);
        }

        Game {
            clock: GameClock::new(),
            world,
            funnel,
            blink,
            fixed_now,
            fixed_scheduler,
            var_scheduler,
            render_state: RenderState::default(),
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
        self.funnel.filter(&self.blink, &mut self.world, event)
    }

    pub fn should_quit(&self) -> bool {
        self.world.get_resource::<Quit>().is_some()
    }

    pub fn render(&mut self) {
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

        for fixed_step in ticks {
            self.fixed_now += fixed_step;

            *self.world.expect_resource_mut::<ClockStep>() = ClockStep {
                now: self.fixed_now,
                step: fixed_step,
            };
            // if cfg!(debug_assertions) {
            self.fixed_scheduler.run_sequential(&mut self.world);
            // } else {
            //     self.fixed_scheduler.run_rayon(&mut self.world);
            // }
            self.blink.reset();
        }

        let mut ticks = self
            .world
            .expect_resource_mut::<Limiter>()
            .0
            .tick_count(step.step);

        if ticks > 0 {
            self.world.expect_resource_mut::<FPS>().add(step.now);
            *self.world.expect_resource_mut::<ClockStep>() = step;

            // if cfg!(debug_assertions) {
            self.var_scheduler.run_sequential(&mut self.world);
            // } else {
            //     self.var_scheduler.run_rayon(&mut self.world);
            // }
            self.blink.reset();
        }
    }
}

// /// Runs game in standalone mode
// pub fn run(plugins: &'static [&'static dyn ArcanaPlugin]) {
//     let event_collector = egui_tracing::EventCollector::default();

//     use tracing_subscriber::layer::SubscriberExt as _;

//     if let Err(err) = tracing::subscriber::set_global_default(
//         tracing_subscriber::fmt()
//             .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
//             .finish()
//             .with(tracing_error::ErrorLayer::default())
//             .with(event_collector.clone()),
//     ) {
//         panic!("Failed to install tracing subscriber: {}", err);
//     }

//     EventLoop::run(|events| async move {
//         let (device, queue) = init_mev();
//         let mut game = Game::launch(
//             &events,
//             plugins.iter().copied(),
//             device,
//             Arc::new(Mutex::new(queue)),
//         );

//         loop {
//             for event in events.next(Some(Instant::now())).await {
//                 game.on_event(event);
//             }

//             if game.should_quit() {
//                 drop(game);
//                 return;
//             }

//             game.tick();
//         }
//     });
// }

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
