//! Tiny graphics crate made for nothing but fun.

pub mod generic;

#[cfg_attr(
    any(windows, all(unix, not(any(target_os = "macos", target_os = "ios")))),
    path = "vulkan/mod.rs"
)]
#[cfg_attr(any(target_os = "macos", target_os = "ios"), path = "metal/mod.rs")]
pub mod backend;

// pub use self::{backend::*, generic::*};
