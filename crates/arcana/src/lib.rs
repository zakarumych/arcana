#![feature(allocator_api)]
#![allow(warnings)]

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

#[cfg(feature = "ed")]
pub use arcana_project as project;

pub mod alloc;

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

#[cfg(feature = "app")]
pub mod app;

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
