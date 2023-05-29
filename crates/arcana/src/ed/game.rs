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

use crate::{
    events::{Event, EventLoop},
    mev,
    plugin::ArcanaPlugin,
};

struct PluginLibrary {
    name: String,
    enabled: HashMap<String, bool>,

    /// Linked library
    lib: libloading::Library,
    plugins: &'static [&'static dyn ArcanaPlugin],
}

pub struct Project {}

impl Project {
    pub fn enabled_plugins_mut(
        &mut self,
    ) -> impl Iterator<Item = (&str, impl Iterator<Item = (&str, &mut bool)>)> {
        self.libs.iter_mut().map(|lib| {
            (
                &*lib.name,
                lib.enabled
                    .iter_mut()
                    .map(|(plugin, enabled)| (&**plugin, enabled)),
            )
        })
    }

    pub fn launch(
        &mut self,
        events: &EventLoop,
        device: &mev::Device,
        queue: &Arc<Mutex<mev::Queue>>,
    ) {
        self.bin.launch(
            events,
            device,
            queue,
            self.plugins.iter().flat_map(|(lib, plugins)| {
                plugins.iter().filter_map(move |(plugin, enabled)| {
                    if *enabled {
                        Some((&**lib, &**plugin))
                    } else {
                        None
                    }
                })
            }),
        );
    }

    pub fn on_event(&mut self, event: Event) -> Option<Event> {
        self.bin.on_event(event)
    }

    pub fn tick(&mut self) {
        self.bin.tick();
    }
}

fn find_workspace_target(path: &Path) -> &Path {
    let mut candidate = path;
    let mut next = Some(path);

    while let Some(path) = next {
        if path.join("Cargo.toml").is_file() {
            candidate = path;
        }
        next = path.parent();
    }

    candidate
}

fn invalid_name_character(c: char) -> bool {
    !c.is_alphanumeric() && c != '_'
}
