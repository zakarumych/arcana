#![feature(allocator_api)]
#![deny(unsafe_op_in_unsafe_fn)]

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

extern crate self as arcana;

// Re-exports
pub use {
    arcana_project as project,
    blink_alloc::{self, Blink, BlinkAlloc},
    bytemuck,
    edict::{self, prelude::*},
    gametime::{self, Clock, ClockStep, Frequency, FrequencyTicker, FrequencyTickerIter},
    na, parking_lot, tokio,
};

pub use mev;
// pub mod alloc;
pub mod arena;
pub mod assets;
pub mod bundle;
mod color_hash;
pub mod events;
pub mod flow;
// pub mod game;
pub mod id;
mod num2name;
pub mod plugin;
pub mod render;
mod stable_hasher;
pub mod stid;
pub mod texture;
pub mod viewport;
pub mod work;

pub use self::{
    color_hash::color_hash,
    id::{BaseId, Id, IdGen},
    num2name::{hash_to_name, num_to_name},
    stable_hasher::{stable_hash, stable_hasher},
    stid::Stid,
};

pub use arcana_proc::stid;

/// Returns version of the arcana crate.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

//
// #[global_allocator]
// static ALLOC: alloc::ArcanaAllocator = alloc::ArcanaAllocator;
