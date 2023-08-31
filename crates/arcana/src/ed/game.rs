//! This module implements main Ed tool - game.
//! Game Tool is responsible for managing game's plugins
//! and run instances of the game.

use std::{
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    events::{Event, EventLoop},
    game::Game,
    mev,
    plugin::ArcanaPlugin,
};

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

    pub fn launch<'a>(
        &mut self,
        events: &EventLoop,
        plugins: impl IntoIterator<Item = &'a dyn ArcanaPlugin>,
        device: mev::Device,
        queue: Arc<Mutex<mev::Queue>>,
    ) {
        let game = Game::launch(events, plugins, device, queue);
        self.games.insert(game.window_id(), game);
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
