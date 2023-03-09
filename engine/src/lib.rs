#![doc = include_str!("../../README.md")]

// Re-exports

pub use {edict, gametime, ultraviolet};

mod rate;

#[cfg(feature = "winit")]
pub use winit;

#[cfg(feature = "winit")]
mod window;

#[cfg(feature = "winit")]
mod events;

#[cfg(all(feature = "winit", feature = "tokio"))]
pub use crate::events::{Event, EventLoop, EventLoopBuilder};

// #[cfg(feature = "derive")]
// pub use engine_proc::*;
