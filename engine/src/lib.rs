#![doc = include_str!("../../README.md")]

// Re-exports

pub use {edict, gametime, na};

#[cfg(feature = "winit")]
pub use winit;

mod events;
mod game;
mod rate;
mod window;

pub use crate::{
    events::{Event, EventLoop, EventLoopBuilder},
    game::{run_game, Game},
};

#[cfg(feature = "derive")]
pub use engine_proc::*;
