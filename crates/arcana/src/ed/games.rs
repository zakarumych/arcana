//! This module implements main Ed tool - game.
//! Game Tool is responsible for managing game's plugins
//! and run instances of the game.

use std::{num::NonZeroU64, sync::Arc};

use arcana_project::{Ident, Item, Project};
use gametime::ClockStep;
use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{WindowBuilder, WindowId},
};

use crate::{
    edict::World,
    events::{Event, EventLoop},
    game::Game,
    plugin::ArcanaPlugin,
    render::{RTTs, RTT},
};

use super::plugins::Plugins;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct GameId(NonZeroU64);

/// Game instances.
pub struct Games {
    /// Game instances.
    games: HashMap<GameId, Game>,
    windowed_games: HashMap<WindowId, GameId>,
    last: u64,
}

impl Games {
    pub fn new() -> Self {
        Games {
            games: HashMap::new(),
            windowed_games: HashMap::new(),
            last: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }

    /// Launches a new game instance in its own window.
    pub fn launch(
        events: &EventLoop,
        world: &mut World,
        device: &mev::Device,
        queue: &Arc<Mutex<mev::Queue>>,
        windowed: bool,
    ) -> Option<GameId> {
        let world = world.local();
        let project = world.expect_resource_mut::<Project>();
        let plugins = world.expect_resource_mut::<Plugins>();
        let mut games = world.expect_resource_mut::<Self>();
        let active_plugins = plugins.active_plugins()?;

        let active = |exported: fn(&dyn ArcanaPlugin) -> &[&Ident]| {
            let plugins = &plugins;
            move |item: &&Item| {
                if !item.enabled {
                    return false;
                }

                if !plugins.is_active(&item.plugin) {
                    return false;
                }

                let plugin = plugins.get_plugin(&item.plugin).unwrap();
                if !exported(plugin).iter().any(|name| **name == *item.name) {
                    return false;
                }
                true
            }
        };

        let active_systems = project
            .manifest()
            .systems
            .iter()
            .filter(active(|p| p.systems()))
            .map(|i| (&*i.plugin, &*i.name));
        let active_filters = project
            .manifest()
            .filters
            .iter()
            .filter(active(|p| p.filters()))
            .map(|i| (&*i.plugin, &*i.name));

        games.last += 1;
        let id = GameId(unsafe { NonZeroU64::new_unchecked(games.last) });

        match windowed {
            false => {
                let game = Game::launch(
                    active_plugins,
                    active_filters,
                    active_systems,
                    device.clone(),
                    queue.clone(),
                    None,
                );
                games.games.insert(id, game);
            }
            true => {
                let window = WindowBuilder::new()
                    .with_title("Arcana Game")
                    .build(events)
                    .unwrap();

                let window_id = window.id();

                let game = Game::launch(
                    active_plugins,
                    active_filters,
                    active_systems,
                    device.clone(),
                    queue.clone(),
                    Some(window),
                );

                games.games.insert(id, game);
                games.windowed_games.insert(window_id, id);
            }
        };

        Some(id)
    }

    pub fn handle_event(
        &mut self,
        window_id: WindowId,
        event: WindowEvent<'static>,
    ) -> Option<WindowEvent<'static>> {
        if let Some(id) = self.windowed_games.get(&window_id) {
            match self.games.get_mut(id) {
                None => {
                    self.windowed_games.remove(&window_id);
                }
                Some(game) => {
                    game.on_event(Event::WindowEvent { window_id, event });
                }
            }
            None
        } else {
            Some(event)
        }
    }

    pub fn render(&mut self) {
        for game in self.games.values_mut() {
            game.render_to_window();
        }
    }

    pub fn tick(&mut self, step: ClockStep) {
        let mut to_remove = Vec::new();
        for (id, game) in &mut self.games {
            game.tick(step);
            if game.should_quit() {
                to_remove.push(*id);
            }
        }
        for id in to_remove {
            self.games.remove(&id);
        }
    }

    pub fn show(world: &mut World, ui: &mut egui::Ui, id: GameId) {
        let world = world.local();
        let mut games = world.expect_resource_mut::<Self>();

        let Some(game) = games.games.get_mut(&id) else {
            ui.centered_and_justified(|ui| {
                ui.label("Game is not running");
            });
            return;
        };

        ui.horizontal_top(|ui| {
            let r = ui.button(egui_phosphor::regular::PAUSE);
            if r.clicked() {
                game.pause();
            }
            let r = ui.button(egui_phosphor::regular::PLAY);
            if r.clicked() {
                game.set_rate_ratio(1, 1);
            }
            let r = ui.button(egui_phosphor::regular::FAST_FORWARD);
            if r.clicked() {
                game.set_rate_ratio(2, 1);
            }

            let mut rate = game.get_rate();

            let value = egui::Slider::new(&mut rate, 0.0..=10.0);
            let r = ui.add(value);
            if r.changed() {
                game.set_rate(rate as f32);
            }
        });

        let size = ui.available_size();
        let Ok(image) = game.render_with_texture(mev::Extent2::new(size.x as u32, size.y as u32))
        else {
            ui.centered_and_justified(|ui| {
                ui.label("GPU OOM");
            });
            return;
        };

        let mut rtts = world.expect_resource_mut::<RTTs>();
        rtts.insert(RTT::new(id.0.get()).unwrap(), image);

        ui.add(egui::Image::new(egui::load::SizedTexture {
            id: egui::TextureId::User(id.0.get()),
            size: size.into(),
        }));
    }
}
