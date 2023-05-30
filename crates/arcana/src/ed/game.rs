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
use winit::window::WindowId;

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
}
