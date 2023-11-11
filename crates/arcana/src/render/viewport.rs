use std::{borrow::Borrow, hash::Hash, num::NonZeroU64};

use hashbrown::HashMap;
use winit::window::WindowId;

/// Resource that contains RTTs.
pub struct RTTs {
    textures: HashMap<RTT, mev::Image>,
    last: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RTT(NonZeroU64);

impl RTT {
    pub fn new(v: u64) -> Option<Self> {
        NonZeroU64::new(v).map(RTT)
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl RTTs {
    pub fn new() -> Self {
        RTTs {
            textures: HashMap::new(),
            last: 0,
        }
    }

    pub fn allocate(&mut self) -> RTT {
        self.last += 1;

        // Safety: `last` is always non-zero.
        // Wrap-around is impossible because `next` is a `u64`.
        unsafe { RTT(NonZeroU64::new_unchecked(self.last)) }
    }

    pub fn get(&self, id: RTT) -> Option<&mev::Image> {
        self.textures.get(&id)
    }

    pub fn insert(&mut self, id: RTT, texture: mev::Image) {
        self.textures.insert(id, texture);
    }

    pub fn remove(&mut self, id: RTT) {
        self.textures.remove(&id);
    }
}

/// Viewport bound to a window.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Viewport {
    Window(WindowId),
    Texture(RTT),
}

impl From<WindowId> for Viewport {
    fn from(id: WindowId) -> Self {
        Viewport::Window(id)
    }
}
