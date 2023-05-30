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

pub use {blink_alloc, bytemuck, edict, gametime, na, parking_lot, tokio};

#[cfg(feature = "client")]
pub use mev;

#[cfg(feature = "client")]
pub use winit;

#[cfg(feature = "client")]
pub mod game;

#[cfg(feature = "client")]
pub mod events;

#[cfg(feature = "client")]
pub mod funnel;

#[cfg(feature = "client")]
pub mod render;

#[cfg(feature = "derive")]
pub use arcana_proc::*;

#[cfg(feature = "client")]
pub mod egui;

#[cfg(feature = "client")]
pub mod texture;

#[cfg(feature = "ed")]
pub mod ed;

pub mod assets;
pub mod bundle;
pub mod plugin;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(feature = "client")]
pub fn init_mev() -> (mev::Device, mev::Queue) {
    let instance = mev::Instance::load().expect("Failed to init graphics");

    let (device, mut queues) = instance
        .create(mev::DeviceDesc {
            idx: 0,
            queue_infos: &[0],
            features: mev::Features::SURFACE,
        })
        .unwrap();
    let queue = queues.pop().unwrap();
    (device, queue)
}
