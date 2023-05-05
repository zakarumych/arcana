#![doc = include_str!("../../README.md")]
#![feature(allocator_api)]

// Re-exports

pub use {blink_alloc, edict, gametime, na};

#[cfg(feature = "graphics")]
pub use nix;

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

#[cfg(feature = "graphics")]
pub mod render;

#[cfg(feature = "derive")]
pub use bob_proc::*;

pub mod egui;
