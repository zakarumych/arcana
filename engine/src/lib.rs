#![doc = include_str!("../../README.md")]

// Re-exports

pub use {edict, gametime, na};

#[cfg(feature = "winit")]
pub use winit;

#[cfg(all(feature = "input", feature = "graphics"))]
pub mod game;

#[cfg(feature = "input")]
pub mod events;

#[cfg(feature = "windowing")]
pub mod window;

#[cfg(feature = "input")]
pub mod funnel;

#[cfg(feature = "derive")]
pub use engine_proc::*;
