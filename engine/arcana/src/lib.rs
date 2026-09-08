#![deny(unsafe_op_in_unsafe_fn, unused_must_use, non_snake_case)]
// #![recursion_limit = "512"]

// #[macro_export]
// macro_rules! for_tuple {
//     ($macro:ident) => {
//         $crate::for_tuple!($macro for A B C D E F G H I J K L M N O P);
//     };
//     ($macro:ident for ) => {
//         $macro!();
//     };
//     ($macro:ident for $head:ident $($tail:ident)*) => {
//         $crate::for_tuple!($macro for $($tail)*);
//         $macro!($head $($tail)*);
//     };
// }

// #[macro_export]
// macro_rules! for_tuple_2 {
//     ($macro:ident) => {
//         $crate::for_tuple_2!($macro for
//             AA AB AC AD AE AF AG AH AI AJ AK AL AM AN AO AP,
//             BA BB BC BD BE BF BG BH BI BJ BK BL BM BN BO BP
//         );
//     };
//     ($macro:ident for ,) => {
//         $macro!(,);
//     };
//     ($macro:ident for $a_head:ident $($a_tail:ident)*, $b_head:ident $($b_tail:ident)*) => {
//         $crate::for_tuple_2!($macro for $($a_tail)*, $($b_tail)*);

//         $macro!($a_head $($a_tail)*, $b_head $($b_tail)*);
//     };
// }

// #[macro_export]
// macro_rules! for_tuple_2x {
//     ($macro:ident) => {
//         $crate::for_tuple_2x!($macro for
//             AA AB AC AD AE AF AG AH AI AJ AK AL AM AN AO AP,
//             BA BB BC BD BE BF BG BH BI BJ BK BL BM BN BO BP
//         );
//     };
//     ($macro:ident for , ) => {
//         $macro!(,);
//     };
//     ($macro:ident for , $b_head:ident $($b_tail:ident)*) => {
//         $macro!(, $b_head $($b_tail)*);
//         $crate::for_tuple_2x!($macro for AA AB AC AD AE AF AG AH AI AJ AK AL AM AN AO AP, $($b_tail)*);
//     };
//     ($macro:ident for $a_head:ident $($a_tail:ident)*, $($b:ident)*) => {
//         $crate::for_tuple_2x!($macro for $($a_tail)*, $($b)*);

//         $macro!($a_head $($a_tail)*, $($b)*);
//     };
// }

// Re-exports
pub use {
    arcana_alloc as alloc, arcana_assets as assets, arcana_base_encoding as base_encoding,
    arcana_ecs as ecs, arcana_error as error, arcana_graphics as graphics, arcana_hash as hash,
    arcana_id as id, arcana_input as input,
    arcana_intern::{
        Ident, IdentError, Name, NameError, format_ident, format_name, ident, name,
        try_format_ident, try_format_name, validate_ident, validate_name,
    },
    arcana_io as io, arcana_model as model, arcana_num2name as num2name, arcana_plugin as plugin,
    arcana_proc::*,
    arcana_project as project, arcana_unfold as unfold, arcana_work_graph as work_graph, athena,
    bytemuck, gametime, hashbrown, mev, parking_lot, smol_str, tokio, tracing,
};

extern crate self as arcana;

pub mod render;

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
    use arcana_assets::import::Importer;
    use arcana_work_graph::Job;

    pub fn is_job<T: Job>() {}
    pub fn is_importer<T: Importer>() {}
}

/// A wrapper type for constant values.
///
/// Prevents accidental modification of the value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Const<T: ?Sized>(T);

impl<T> Const<T> {
    /// Creates a new constant value.
    #[inline(always)]
    pub const fn new(value: T) -> Self {
        Const(value)
    }

    /// Consumes the `Const` wrapper and returns the inner value.
    #[inline(always)]
    pub fn into_inner(me: Self) -> T {
        me.0
    }
}

impl<T: ?Sized> Const<T> {
    /// Returns a mutable reference to the constant value, allowing modification.
    ///
    /// The caller must think twice about their life choices before using this method.
    #[inline(always)]
    pub fn cheat(me: &mut Self) -> &mut T {
        &mut me.0
    }
}

impl<T: ?Sized> std::ops::Deref for Const<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for Const<T> {
    #[inline(always)]
    fn from(value: T) -> Self {
        Const::new(value)
    }
}

impl<T: ?Sized> AsRef<T> for Const<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> std::borrow::Borrow<T> for Const<T> {
    #[inline(always)]
    fn borrow(&self) -> &T {
        &self.0
    }
}
