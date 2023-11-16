#![feature(allocator_api)]
#![allow(warnings)]

/// Finds offset of a field in a struct.
///
/// It uses `addr_of!` macro to get raw pointer to the field for uninit struct
/// and then calculates offset from the beginning of the struct.
#[macro_export]
macro_rules! offset_of {
    ($struct:ident . $field:ident) => {{
        let uninit: ::core::mem::MaybeUninit<$struct> = ::core::mem::MaybeUninit::uninit();

        if false {
            // Safety: Not executed.
            // This is required to make sure that field exists on the struct itself.
            // To avoid `(*struct_ptr).$field` below to invoke `Deref::deref`.
            unsafe {
                let $struct { $field: _, .. } = uninit.assume_init();
            }
        }

        let struct_ptr: *const _ = unsafe { uninit.as_ptr() };
        let field_ptr: *const _ = unsafe { ::core::ptr::addr_of!((*struct_ptr).$field) };

        // # Safety: Cannot overflow because result is field offset.
        unsafe { field_ptr.cast::<u8>().offset_from(struct_ptr.cast::<u8>()) as usize }
    }};
}

/// `std::format` where all arguments are constants.
/// Uses thread-local to store result after first formatting.
///
/// This helps avoiding re-formatting of the same string each time code is executed.
///
/// String created will never be freed.
/// This is OK since we were goint go use it untile the end of the program.
#[macro_export]
macro_rules! const_format {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        ::std::thread_local! {
            static VALUE: &'static str = ::std::format!($fmt $(, $arg)*).leak();
        }
        let s: &'static str = VALUE.with(|s| *s);
        s
    }};
}

// Re-exports
pub use {
    arcana_project as project,
    blink_alloc::{self, Blink, BlinkAlloc},
    bytemuck,
    edict::{self, prelude::*},
    gametime::{self, Clock, ClockStep, Frequency, FrequencyTicker, FrequencyTickerIter},
    na, parking_lot, tokio,
};

#[cfg(feature = "derive")]
pub use arcana_proc::*;

feature_client! {
    pub use mev;
    pub mod events;
    pub mod game;
    pub mod render;
    pub mod texture;
    pub mod viewport;
}

pub mod alloc;
pub mod assets;
pub mod bundle;
pub mod flow;
pub mod plugin;

/// Returns version of the arcana crate.
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
