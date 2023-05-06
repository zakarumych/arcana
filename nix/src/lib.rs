//! Tiny graphics crate made for nothing but fun.
#![deny(unsafe_op_in_unsafe_fn)]

pub mod generic;
mod traits;

#[cfg_attr(
    any(windows, all(unix, not(any(target_os = "macos", target_os = "ios")))),
    path = "vulkan/mod.rs"
)]
#[cfg_attr(any(target_os = "macos", target_os = "ios"), path = "metal/mod.rs")]
pub mod backend;

mod private {
    pub trait Sealed {}
}

pub use self::{backend::*, generic::*};
pub use nix_proc::{Arguments, Constants};

#[doc(hidden)]
pub mod for_macro {
    pub use crate::backend::for_macro::*;
    pub use crate::generic::{
        Automatic, Constants, LibraryInput, Sampled, ShaderSource, Storage, Uniform,
    };
}
