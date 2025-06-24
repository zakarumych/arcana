#![feature(allocator_api, maybe_uninit_slice)]
#![deny(unsafe_op_in_unsafe_fn, unused_must_use, non_snake_case)]
#![recursion_limit = "512"]

#[macro_export]
macro_rules! for_tuple {
    ($macro:ident) => {
        $crate::for_tuple!($macro for A B C D E F G H I J K L M N O P);
    };
    ($macro:ident for ) => {
        $macro!();
    };
    ($macro:ident for $head:ident $($tail:ident)*) => {
        $crate::for_tuple!($macro for $($tail)*);
        $macro!($head $($tail)*);
    };
}

#[macro_export]
macro_rules! for_tuple_2 {
    ($macro:ident) => {
        $crate::for_tuple_2!($macro for
            AA AB AC AD AE AF AG AH AI AJ AK AL AM AN AO AP,
            BA BB BC BD BE BF BG BH BI BJ BK BL BM BN BO BP
        );
    };
    ($macro:ident for ,) => {
        $macro!(,);
    };
    ($macro:ident for $a_head:ident $($a_tail:ident)*, $b_head:ident $($b_tail:ident)*) => {
        $crate::for_tuple_2!($macro for $($a_tail)*, $($b_tail)*);

        $macro!($a_head $($a_tail)*, $b_head $($b_tail)*);
    };
}

#[macro_export]
macro_rules! for_tuple_2x {
    ($macro:ident) => {
        $crate::for_tuple_2x!($macro for
            AA AB AC AD AE AF AG AH AI AJ AK AL AM AN AO AP,
            BA BB BC BD BE BF BG BH BI BJ BK BL BM BN BO BP
        );
    };
    ($macro:ident for , ) => {
        $macro!(,);
    };
    ($macro:ident for , $b_head:ident $($b_tail:ident)*) => {
        $macro!(, $b_head $($b_tail)*);
        $crate::for_tuple_2x!($macro for AA AB AC AD AE AF AG AH AI AJ AK AL AM AN AO AP, $($b_tail)*);
    };
    ($macro:ident for $a_head:ident $($a_tail:ident)*, $($b:ident)*) => {
        $crate::for_tuple_2x!($macro for $($a_tail)*, $($b)*);

        $macro!($a_head $($a_tail)*, $($b)*);
    };
}

/// `std::format` where all arguments are constants.
/// Uses thread-local to store result after first formatting.
///
/// This helps avoiding re-formatting of the same string each time code is executed.
///
/// String created will never be freed.
/// This is OK since we were going go use it until the end of the program.
#[macro_export]
macro_rules! const_format {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        ::std::thread_local! {
            static VALUE: &'static str = { ::std::format!($fmt $(, $arg)*).leak() };
        }
        let s: &'static str = VALUE.with(|s| *s);
        s
    }};
}

extern crate self as arcana;

// Re-exports
pub use {
    arcana_intern::{ident, name, Ident, IdentError, Name, NameError},
    arcana_project as project, bytemuck, gametime, hashbrown, na, parking_lot, tokio, tracing,
    vtid::{HasVtid, Vtid},
};

pub mod arena;
pub mod assets;
pub mod base58;
pub mod code;
pub mod ecs;
pub mod ed;
pub mod events;
pub mod graphics;
pub mod hash;
pub mod id;
pub mod input;
pub mod io;
pub mod model;
mod num2name;
pub mod plugin;
pub mod render;
pub mod serde_with;
pub mod slot;
pub mod tany;
pub mod task;
pub mod unfold;
pub mod viewport;
pub mod work;

#[macro_export]
macro_rules! static_assert {
    ($cond:expr) => {
        const _: () = {
            assert!($cond);
        };
    };
    ($cond:expr, $($arg:tt)+) => {
        const _: () = {
            assert!($cond, $($arg)+);
        };
    };
}

static_assert!(
    size_of::<usize>() <= size_of::<u64>(),
    "Unchecked cast from usize to u64 is performed in Arcana"
);

/// Returns version of the arcana crate.
pub fn version() -> &'static str {
    // Version of each crate in the workspace is the same.
    env!("CARGO_PKG_VERSION")
}

#[inline(always)]
pub fn type_id<T: 'static>() -> std::any::TypeId {
    std::any::TypeId::of::<T>()
}

/// Module that contains non-public items exposed for macros.
#[doc(hidden)]
pub mod for_macro {
    use crate::{assets::import::Importer, work::Job};

    pub fn is_job<T: Job>() {}
    pub fn is_importer<T: Importer>() {}
}
