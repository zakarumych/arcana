use std::{fmt, hash::Hash, num::NonZeroU64};

pub trait Id: fmt::Debug + Copy + Ord + Eq + Hash {
    fn new(value: NonZeroU64) -> Self;
    fn get(self) -> u64;
    fn get_nonzero(self) -> NonZeroU64;
}

/// Creates id from integer literal.
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

/// Produces hash based on hashable values.
/// The hash is guaranteed to be stable across different runs and compilations of the program
/// as long as the values do not change.
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

/// Produces id by hashing the module path and values.
/// The hash is guaranteed to be stable across different runs and compilations of the program.
///
/// Unlike `hash_id!` result will change if macro invocation is moved to different module or module path changes.
/// Use it if you may use same values in different modules but want to have different ids.
///
/// It also supports hashing single identifier.
#[macro_export]
macro_rules! local_hash_id {
    ($type:ident $(,)?) => {{
        $crate::hash_id!(::core::module_path!(), ::core::stringify!($type))
    }};
    ($type:ident => $id:ty) => {{
        $crate::hash_id!(::core::module_path!(), ::core::stringify!($type) => $id)
    }};
    ($($value:expr),+ $(,)?) => {
        $crate::hash_id!(::core::module_path!(), $($value,)+)
    };
    ($($value:expr),+ => $id:ty) => {
        $crate::hash_id!(::core::module_path!(), $($value,)+ => $id)
    };
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
            value: ::core::num::NonZeroU64,
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::id::fmt_id(self.value.get(), stringify!($name), f)
            }
        }

        impl $name {
            #[cfg_attr(inline_more, inline(always))]
            pub const fn new(value: ::core::num::NonZeroU64) -> Self {
                $name {
                    value,
                }
            }

            #[cfg_attr(inline_more, inline(always))]
            pub const fn get(self) -> u64 {
                self.value.get()
            }

            #[cfg_attr(inline_more, inline(always))]
            pub const fn get_nonzero(self) -> ::core::num::NonZeroU64 {
                self.value
            }
        }

        impl $crate::Id for $name {
            #[cfg_attr(inline_more, inline(always))]
            fn new(value: ::core::num::NonZeroU64) -> Self {
                $name {
                    value,
                }
            }

            #[cfg_attr(inline_more, inline(always))]
            fn get(self) -> u64 {
                self.value.get()
            }

            #[cfg_attr(inline_more, inline(always))]
            fn get_nonzero(self) -> ::core::num::NonZeroU64 {
                self.value
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                self.value.serialize(serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let value = ::serde::Deserialize::deserialize(deserializer)?;
                Ok($name { value })
            }
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BaseId {
    value: NonZeroU64,
}

impl BaseId {
    #[cfg_attr(inline_more, inline(always))]
    pub const fn new(value: NonZeroU64) -> Self {
        BaseId { value }
    }

    #[cfg_attr(inline_more, inline(always))]
    pub const fn get(self) -> u64 {
        self.value.get()
    }

    #[cfg_attr(inline_more, inline(always))]
    pub const fn get_nonzero(self) -> NonZeroU64 {
        self.value
    }
}

#[cfg_attr(inline_more, inline(always))]
#[doc(hidden)]
pub fn fmt_id(mut value: u64, kind: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    const BASE32: &[u8; 32] = b"0123456789abcdefghjkmnpqrstuvyxz";

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
