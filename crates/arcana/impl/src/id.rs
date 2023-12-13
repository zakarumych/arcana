use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZeroU64,
};

/// Typed identifier.
///
/// Targets are inputs and outputs of jobs.
/// See [`Target`] trait and [`Job API`](crate::work::job) for more info.
#[repr(transparent)]
pub struct Id<T: ?Sized> {
    value: NonZeroU64,
    _marker: PhantomData<T>,
}

impl<T: ?Sized> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Id<T> {}

impl<T: ?Sized> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id{}", self.value.get())
    }
}

impl<T: ?Sized> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: ?Sized> Eq for Id<T> {}

impl<T: ?Sized> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state)
    }
}

impl<T: ?Sized> Id<T> {
    pub const fn new(value: NonZeroU64) -> Self {
        Id {
            value,
            _marker: PhantomData,
        }
    }

    pub const fn get(self) -> u64 {
        self.value.get()
    }

    pub const fn get_nonzero(self) -> NonZeroU64 {
        self.value
    }

    pub const fn cast<U: ?Sized>(self) -> Id<U> {
        Id {
            value: self.value,
            _marker: PhantomData,
        }
    }
}

pub struct IdGen {
    next_id: u64,
}

impl IdGen {
    pub const fn new() -> Self {
        IdGen { next_id: 1 }
    }

    pub fn next<T: ?Sized>(&mut self) -> Id<T> {
        assert_ne!(self.next_id, 0, "IdGen overflow");
        let value = NonZeroU64::new(self.next_id).unwrap();
        self.next_id += 1;
        Id::new(value)
    }
}
