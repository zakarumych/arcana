use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZeroU64,
};

pub trait Id: fmt::Debug + Copy + Ord + Eq + Hash {
    fn new(value: NonZeroU64) -> Self;
    fn get(self) -> u64;
    fn get_nonzero(self) -> NonZeroU64;
}

#[macro_export]
macro_rules! static_id {
    ($value:literal) => {{
        const VALUE: u64 = {
            let v = $value;
            assert!(v != 0, "Id cannot be zero");
            v
        };
        $crate::Id::new(unsafe { ::core::num::NonZeroU64::new_unchecked(VALUE) })
    }};
    ($value:literal as $id:ty) => {{
        const VALUE: u64 = {
            let v = $value;
            assert!(v != 0, "Id cannot be zero");
            v
        };
        <$id as $crate::Id>::new(unsafe { ::core::num::NonZeroU64::new_unchecked(VALUE) })
    }};
}

#[macro_export]
macro_rules! hash_id {
    ($($value:expr),+ $(,)?) => {{
        let mut hasher = $crate::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher);
        $crate::Id::new(unsafe { ::core::num::NonZeroU64::new_unchecked(hash | 1) })
    }};
    ($($value:expr),+ => $id:ty) => {{
        let mut hasher = $crate::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher);
        <$id as $crate::Id>::new(unsafe { ::core::num::NonZeroU64::new_unchecked(hash | 1) })
    }};
}

#[macro_export]
macro_rules! make_id {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident $(;)?
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        $vis struct $name {
            id: $crate::BaseId,
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                self.id.fmt(stringify!($name), f)
            }
        }

        impl $name {
            #[inline(always)]
            pub const fn new(value: ::core::num::NonZeroU64) -> Self {
                $name {
                    id: $crate::BaseId::new(value),
                }
            }

            #[inline(always)]
            pub const fn get(self) -> u64 {
                self.id.get()
            }

            #[inline(always)]
            pub const fn get_nonzero(self) -> ::core::num::NonZeroU64 {
                self.id.get_nonzero()
            }
        }

        impl $crate::Id for $name {
            #[inline(always)]
            fn new(value: ::core::num::NonZeroU64) -> Self {
                $name {
                    id: $crate::BaseId::new(value),
                }
            }

            #[inline(always)]
            fn get(self) -> u64 {
                self.id.get()
            }

            #[inline(always)]
            fn get_nonzero(self) -> ::core::num::NonZeroU64 {
                self.id.get_nonzero()
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                self.get_nonzero().serialize(serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let value = ::serde::Deserialize::deserialize(deserializer)?;
                Ok($name::new(value))
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

impl BaseId {
    #[inline(always)]
    #[doc(hidden)]
    pub fn fmt(&self, kind: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const BASE32: &[u8; 32] = b"0123456789abcdefghjkmnpqrstuvyxz";

        let mut value = self.value.get();
        let mut buf = [0u8; 8];
        let mut i = 0;
        while value != 0 {
            buf[i] = BASE32[(value & 31) as usize];
            value >>= 5;
            i += 1;
        }
        buf[..i].reverse();

        // Safety: All bytes in `buf[..i]` are valid UTF-8 chars.
        let id = unsafe { ::core::str::from_utf8_unchecked(&buf[..i]) };
        write!(f, "{kind}({})", id)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct IdGen {
    next_id: u64,
}

impl Default for IdGen {
    fn default() -> Self {
        IdGen::new()
    }
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
