//! A render job that presents an image as luminous virtual CRT pixels.

mod render;

pub use render::CrtScreen;

arcana::declare_plugin!();
