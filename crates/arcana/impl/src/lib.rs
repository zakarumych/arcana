#![feature(allocator_api)]
#![deny(unsafe_op_in_unsafe_fn)]
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
            static VALUE: &'static str = const { ::std::format!($fmt $(, $arg)*).leak() };
        }
        let s: &'static str = VALUE.with(|s| *s);
        s
    }};
}

extern crate self as arcana;

// Re-exports
pub use {
    arcana_names::{ident, name, Ident, Name},
    arcana_proc::{stable_hash_tokens, with_stid, WithStid},
    arcana_project as project,
    blink_alloc::{self, Blink, BlinkAlloc},
    bytemuck,
    edict::{self, prelude::*},
    gametime::{self, Clock, ClockStep, Frequency, FrequencyTicker, FrequencyTickerIter},
    hashbrown, na, parking_lot, tokio,
};

pub use mev;
pub mod arena;
pub mod assets;
pub mod code;
pub mod events;
pub mod flow;
pub mod id;
pub mod input;
pub mod model;
mod num2name;
pub mod plugin;
pub mod refl;
pub mod render;
pub mod serde_with;
mod stable_hasher;
mod stid;
pub mod texture;
pub mod unfold;
pub mod viewport;
pub mod work;

pub use self::{
    id::{BaseId, Id, IdGen},
    num2name::{hash_to_name, num_to_name},
    stable_hasher::{
        hue_hash, mix_hash_with_string, rgb_hash, rgba_hash, rgba_premultiplied_hash, stable_hash,
        stable_hash_read, stable_hasher,
    },
    stid::{Stid, WithStid},
};

/// Returns version of the arcana crate.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Triggers panic.
/// Use when too large capacity is requested.
#[inline(always)]
#[cold]
fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}

#[inline(always)]
fn alloc_guard(alloc_size: usize) {
    if usize::BITS < 64 && alloc_size > isize::MAX as usize {
        capacity_overflow()
    }
}
