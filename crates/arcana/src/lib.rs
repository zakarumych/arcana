#![feature(allocator_api)]
#![allow(warnings)]

#[macro_export]
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

/// `std::format` where all arguments are constants.
#[macro_export]
macro_rules! const_format {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        std::thread_local! {
            static VALUE: &'static str = std::format!($fmt $(, $arg)*).leak();
        }
        let s: &'static str = VALUE.with(|s| *s);
        s
    }};
}

// Re-exports

pub use {blink_alloc, bytemuck, edict, gametime, na, parking_lot, tokio};

pub use arcana_project as project;

#[cfg(feature = "derive")]
pub use arcana_proc::*;

#[cfg(feature = "app")]
pub mod app;

#[cfg(feature = "ed")]
pub mod ed;

feature_client! {
    pub use mev;
    pub use winit;
    pub mod egui;
    pub mod events;
    pub mod funnel;
    pub mod game;
    pub mod render;
    pub mod texture;
    pub mod window;
}

pub mod alloc;
pub mod assets;
pub mod bundle;
pub mod flow;
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
            queues: &[0],
            features: mev::Features::SURFACE,
        })
        .unwrap();
    let queue = queues.pop().unwrap();
    (device, queue)
}

#[cfg(feature = "client")]
#[macro_export]
macro_rules! feature_client {
    ($($tt:tt)*) => {$($tt)*};
}

#[cfg(not(feature = "client"))]
#[macro_export]
macro_rules! feature_client {
    ($($tt:tt)*) => {};
}

#[cfg(feature = "server")]
#[macro_export]
macro_rules! feature_server {
    ($($tt:tt)*) => {$($tt)*};
}

#[cfg(not(feature = "server"))]
#[macro_export]
macro_rules! feature_server {
    ($($tt:tt)*) => {};
}

#[cfg(feature = "ed")]
#[macro_export]
macro_rules! feature_ed {
    ($($tt:tt)*) => {$($tt)*};
}

#[cfg(not(feature = "ed"))]
#[macro_export]
macro_rules! feature_ed {
    ($($tt:tt)*) => {};
}

#[cfg(feature = "client")]
#[macro_export]
macro_rules! not_feature_client {
    ($($tt:tt)*) => {};
}

#[cfg(not(feature = "client"))]
#[macro_export]
macro_rules! not_feature_client {
    ($($tt:tt)*) => {$($tt)*};
}

#[cfg(feature = "server")]
#[macro_export]
macro_rules! not_feature_server {
    ($($tt:tt)*) => {};
}

#[cfg(not(feature = "server"))]
#[macro_export]
macro_rules! not_feature_server {
    ($($tt:tt)*) => {$($tt)*};
}

#[cfg(feature = "ed")]
#[macro_export]
macro_rules! not_feature_ed {
    ($($tt:tt)*) => {};
}

#[cfg(not(feature = "ed"))]
#[macro_export]
macro_rules! not_feature_ed {
    ($($tt:tt)*) => {$($tt)*};
}

/// Conditional compilation based on features enabled in arcana crate.
#[macro_export]
macro_rules! feature {
    (client => $($tt:tt)*) => { $crate::feature_client!($($tt)*) };
    (server => $($tt:tt)*) => { $crate::feature_server!($($tt)*) };
    (ed => $($tt:tt)*) => { $crate::feature_ed!($($tt)*) };

    (!client => $($tt:tt)*) => { $crate::not_feature_client!($($tt)*) };
    (!server => $($tt:tt)*) => { $crate::not_feature_server!($($tt)*) };
    (!ed => $($tt:tt)*) => { $crate::not_feature_ed!($($tt)*) };

    (if client { $($yes:tt)* } $(else { $($no:tt)* })?) => { $crate::feature_client!($($yes)*); $($crate::not_feature_client!($($no)*);)? };
    (if server { $($yes:tt)* } $(else { $($no:tt)* })?) => { $crate::feature_server!($($yes)*); $($crate::not_feature_server!($($no)*);)? };
    (if ed { $($yes:tt)* } $(else { $($no:tt)* })?) => { $crate::feature_ed!($($yes)*); $($crate::not_feature_ed!($($no)*);)? };
}

// #[global_allocator]
// static ALLOC: alloc::ArcanaAllocator = alloc::ArcanaAllocator;

fn move_element<T>(slice: &mut [T], from_index: usize, to_index: usize) {
    if from_index == to_index {
        return;
    }
    if from_index < to_index {
        let sub = &mut slice[from_index..=to_index];
        sub.rotate_left(1);
    } else {
        let sub = &mut slice[to_index..=from_index];
        sub.rotate_right(1);
    }
}
