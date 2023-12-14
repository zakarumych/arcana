use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZeroU64,
};

pub trait Id: Copy + Ord + Eq + Hash {
    fn new(value: NonZeroU64) -> Self;
    fn get(self) -> u64;
    fn get_nonzero(self) -> NonZeroU64;

    #[inline(always)]
    fn cast<U: Id>(self) -> U {
        U::new(self.get_nonzero())
    }
}

#[macro_export]
macro_rules! make_id {
    ($vis:vis $name:ident) => {
        $vis struct $name {
            id: $crate::BaseId,
        }

        impl $name {
            #[inline(always)]
            pub const fn new(value: NonZeroU64) -> Self {
                $name {
                    id: $crate::BaseId::new(value),
                }
            }

            #[inline(always)]
            pub const fn get(self) -> u64 {
                self.id.get()
            }

            #[inline(always)]
            pub const fn get_nonzero(self) -> NonZeroU64 {
                self.id.get_nonzero()
            }

            #[inline(always)]
            pub const fn cast<U: ?Sized>(self) -> U {
                U::new(self.get_nonzero())
            }
        }

        impl $crate::Id for $name {
            #[inline(always)]
            fn new(value: NonZeroU64) -> Self {
                $name {
                    id: $crate::BaseId::new(value),
                }
            }

            #[inline(always)]
            fn get(self) -> u64 {
                self.id.get()
            }

            #[inline(always)]
            fn get_nonzero(self) -> NonZeroU64 {
                self.id.get_nonzero()
            }

            #[inline(always)]
            fn cast<U: ?Sized>(self) -> U {
                U::new(self.get_nonzero())
            }
        }
    };
}

/// Typed identifier.
///
/// Targets are inputs and outputs of jobs.
/// See [`Target`] trait and [`Job API`](crate::work::job) for more info.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BaseId {
    value: NonZeroU64,
}

impl BaseId {
    #[inline(always)]
    pub const fn new(value: NonZeroU64) -> Self {
        BaseId { value }
    }

    #[inline(always)]
    pub const fn get(self) -> u64 {
        self.value.get()
    }

    #[inline(always)]
    pub const fn get_nonzero(self) -> NonZeroU64 {
        self.value
    }
}

#[derive(Clone)]
pub struct IdGen {
    next_id: u64,
}

impl IdGen {
    pub const fn new() -> Self {
        IdGen { next_id: 1 }
    }

    pub fn next<T: Id>(&mut self) -> T {
        assert_ne!(self.next_id, 0, "IdGen overflow");
        let value = NonZeroU64::new(self.next_id).unwrap();
        self.next_id += 1;
        Id::new(value)
    }
}
