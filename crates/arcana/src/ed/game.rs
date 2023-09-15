//! This module implements main Ed tool - game.
//! Game Tool is responsible for managing game's plugins
//! and run instances of the game.

use std::sync::Arc;

use arcana_project::Project;
use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    edict::World,
    events::{Event, EventLoop},
    game::Game,
};

use super::plugins::Plugins;

/// Game instances.
pub struct Games {
    /// Game instances.
    games: HashMap<WindowId, Game>,
}

impl Games {
    pub fn new() -> Self {
        Games {
            games: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }

    pub fn launch(
        world: &mut World,
        events: &EventLoop,
        device: &mev::Device,
        queue: &Arc<Mutex<mev::Queue>>,
    ) {
        let world = world.local();
        let project = world.expect_resource_mut::<Project>();
        let plugins = world.expect_resource_mut::<Plugins>();
        let mut games = world.expect_resource_mut::<Self>();
        match plugins.enabled_plugins(&project) {
            Some(enabled_plugins) => {
                let game = Game::launch(events, enabled_plugins, device.clone(), queue.clone());
                games.games.insert(game.window_id(), game);
            }
            None => tracing::error!("Not all enabled plugins were linked"),
        }
    }

    pub fn handle_event(
        &mut self,
        window_id: WindowId,
        event: WindowEvent<'static>,
    ) -> Option<WindowEvent<'static>> {
        if let Some(game) = self.games.get_mut(&window_id) {
            game.on_event(Event::WindowEvent { window_id, event });
            None
        } else {
            Some(event)
        }
    }

    pub fn tick(&mut self) {
        let mut to_remove = Vec::new();
        for (id, game) in &mut self.games {
            game.tick();

            if game.should_quit() {
                to_remove.push(*id);
            }
        }
        for id in to_remove {
            self.games.remove(&id);
        }
    }
}
