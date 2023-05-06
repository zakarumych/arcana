#![doc = include_str!("../../README.md")]
#![feature(allocator_api)]

macro_rules! offset_of {
    ($struct:ident . $field:ident) => {
        unsafe {
            let uninit: core::mem::MaybeUninit<$struct> = core::mem::MaybeUninit::uninit();
            if false {
                let $struct { $field: _, .. } = uninit.assume_init();
                0
            } else {
                let ptr = uninit.as_ptr();
                core::ptr::addr_of!((*ptr).$field)
                    .cast::<u8>()
                    .offset_from(ptr.cast::<u8>()) as usize
            }
        }
    };
}

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
