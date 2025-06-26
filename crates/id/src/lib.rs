//! Strong id utility.

mod seqgen;
mod uidgen;

use std::{fmt, hash::Hash, num::NonZeroU64};

use arcana_base_encoding::base58;

pub use self::{seqgen::SeqIdGen, uidgen::TimeUidGen};

/// ID trait to be implemented by all id types.
pub trait Id: fmt::Debug + Copy + Ord + Eq + Hash {
    fn new(value: NonZeroU64) -> Self;
    fn get(self) -> u64;
    fn get_nonzero(self) -> NonZeroU64;
}

/// Marker trait that signals that id is generated
/// in a way that guarantees or at least attempts to guarantee uniqueness.
///
/// There's is no way to truly guarantee uniqueness of generated ids
/// without some kind of global coordination.
pub trait Uid: Id + serde::Serialize + for<'de> serde::Deserialize<'de> {}

/// Error that is returned when trying to create id from zero value.
#[derive(Debug)]
pub struct ZeroIDError;

/// Creates id from integer literal.
#[macro_export]
macro_rules! static_id {
    ($value:literal) => {{
        const {
            assert!($value != 0, "Id cannot be zero");
        }
        $crate::Id::new(unsafe { ::core::num::NonZeroU64::new_unchecked($value) })
    }};
    ($value:literal as $id:ty) => {
        const {
            assert!($value != 0, "Id cannot be zero");
            <$id>::new(unsafe { ::core::num::NonZeroU64::new_unchecked($value) })
        }
    };
}

/// Produces hash based on hashable values.
/// The hash is guaranteed to be stable across different runs and compilations of the program
/// as long as the values do not change.
#[macro_export]
macro_rules! hash_id {
    ($($value:expr),+ $(,)?) => {{
        let mut hasher = $crate::for_macro::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000;
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($($value:expr),+ => $id:ty) => {{
        let mut hasher = $crate::for_macro::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000;
        <$id>::new(::core::num::NonZeroU64::new(hash).unwrap())
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
    ($($value:expr),+ $(,)?) => {{
        let mut hasher = $crate::for_macro::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher);
        let hash = $crate::for_macro::mix_hash_with_string(hash, ::core::module_path!()) | 0x8000_0000_0000_0000;
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($($value:expr),+ => $id:ty) => {{
        let mut hasher = $crate::for_macro::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher);
        let hash = $crate::for_macro::mix_hash_with_string(hash, ::core::module_path!()) | 0x8000_0000_0000_0000;
        <$id>::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
}

/// Produces id by hashing the module path and values.
/// The hash is guaranteed to be stable across different runs and compilations of the program.
#[macro_export]
macro_rules! tokens_hash_id {
    ($($tokens:tt)+) => {{
        let hash = const { $crate::for_macro::stable_hash_tokens!($($tokens)+) | 0x8000_0000_0000_0000 };
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
}

/// Produces id by hashing the module path and values.
/// The hash is guaranteed to be stable across different runs and compilations of the program.
///
/// Unlike `tokens_hash_id!` result will change if macro invocation is moved to different module or module path changes.
/// Use it if you may use same values in different modules but want to have different ids.
///
/// It also supports hashing single identifier.
#[macro_export]
macro_rules! local_name_hash_id {
    ($ident:ident) => {{
        let hash = const {
            let hash = $crate::for_macro::stable_hash_tokens!($ident);
            $crate::for_macro::mix_hash_with_string(hash, ::core::module_path!())
                | 0x8000_0000_0000_0000
        };
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($ident:ident => $id:ty) => {
        const {
            let hash = $crate::for_macro::stable_hash_tokens!($ident);
            let hash = $crate::for_macro::mix_hash_with_string(hash, ::core::module_path!())
                | 0x8000_0000_0000_0000;
            <$id>::new(unsafe { ::core::num::NonZeroU64::new_unchecked(hash) })
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! make_id_base {
    (
        $(#[$meta:meta])+
        $vis:vis $name:ident;
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        $vis struct $name {
            value: ::core::num::NonZeroU64,
        }

        impl ::core::fmt::Debug for $name {
            #[inline(always)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::fmt_id(self.value.get(), f)
            }
        }

        impl ::core::fmt::Display for $name {
            #[inline(always)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::fmt_id(self.value.get(), f)
            }
        }

        impl $name {
            #[allow(unused)]
            #[inline(always)]
            pub const fn new(value: ::core::num::NonZeroU64) -> Self {
                $name {
                    value,
                }
            }

            #[allow(unused)]
            #[inline(always)]
            pub const fn get(self) -> u64 {
                self.value.get()
            }

            #[allow(unused)]
            #[inline(always)]
            pub const fn get_nonzero(self) -> ::core::num::NonZeroU64 {
                self.value
            }
        }

        impl $crate::Id for $name {
            #[inline(always)]
            fn new(value: ::core::num::NonZeroU64) -> Self {
                $name {
                    value,
                }
            }

            #[inline(always)]
            fn get(self) -> u64 {
                self.value.get()
            }

            #[inline(always)]
            fn get_nonzero(self) -> ::core::num::NonZeroU64 {
                self.value
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::ParseIdError;

            #[inline(always)]
            fn from_str(s: &str) -> Result<Self, $crate::ParseIdError> {
                let value = $crate::parse_id(s)?;
                Ok($name { value })
            }
        }

        impl ::core::convert::TryFrom<u64> for $name {
            type Error = $crate::ZeroIDError;

            fn try_from(value: u64) -> Result<Self, $crate::ZeroIDError> {
                match ::core::num::NonZeroU64::try_from(value) {
                    Ok(value) => Ok($name { value }),
                    Err(_) => Err($crate::ZeroIDError),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! make_id {
    (
        $(#[$meta:meta])+
        $vis:vis $name:ident;
    ) => {
        $crate::make_id_base!{
            $(#[$meta])+
            $vis $name;
        }

        impl $name {
            #[allow(unused)]
            #[inline(always)]
            pub fn generate(generator: &mut impl $crate::GenId<Value = ::core::num::NonZeroU64>) -> Self {
                $name::new(generator.generate())
            }
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseIdError {
    #[error(transparent)]
    DecodingError(#[from] base58::DecodeError),

    #[error("Id string is too long")]
    TooLong,

    #[error("Id cannot be zero")]
    ZeroId,
}

#[macro_export]
macro_rules! make_uid {
    (
        $(#[$meta:meta])+
        $vis:vis $name:ident;
    ) => {
        $crate::make_id_base!{
            $(#[$meta])+
            $vis $name;
        }

        impl $name {
            #[allow(unused)]
            #[inline(always)]
            pub fn generate(generator: &mut impl $crate::GenUid<Value = ::core::num::NonZeroU64>) -> Self {
                $name::new(generator.generate())
            }
        }

        impl $crate::for_macro::ser::Serialize for $name {
            #[inline(always)]
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: $crate::for_macro::ser::Serializer,
            {
                self.value.serialize(serializer)
            }
        }

        impl<'de> $crate::for_macro::de::Deserialize<'de> for $name {
            #[inline(always)]
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: $crate::for_macro::de::Deserializer<'de>,
            {
                struct Visitor;

                impl<'de> $crate::for_macro::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    #[inline(always)]
                    fn expecting(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                        write!(f, "a non-zero 64-bit integer or a hex string")
                    }

                    #[inline(always)]
                    fn visit_u64<E>(self, v: u64) -> Result<$name, E>
                    where
                        E: $crate::for_macro::de::Error,
                    {
                        match ::core::num::NonZeroU64::new(v) {
                            None => Err(E::invalid_value($crate::for_macro::de::Unexpected::Unsigned(0), &self)),
                            Some(value) => Ok($name { value }),
                        }
                    }

                    #[inline(always)]
                    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                    where
                        E: $crate::for_macro::de::Error,
                    {
                        if v <= 0 {
                            Err(E::invalid_value($crate::for_macro::de::Unexpected::Signed(v), &self))
                        } else {
                            Ok($name { value: ::core::num::NonZeroU64::new(v as u64).unwrap() })
                        }
                    }

                    #[inline(always)]
                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: $crate::for_macro::de::Error,
                    {
                        v.parse().map_err(E::custom)
                    }
                }

                deserializer.deserialize_u64(Visitor)
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

#[inline(always)]
#[doc(hidden)]
pub fn fmt_id(value: u64, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    base58::encode_to_fmt(&value.to_le_bytes(), &mut *f)
}

#[inline]
#[doc(hidden)]
pub fn parse_id(string: &str) -> Result<NonZeroU64, ParseIdError> {
    let mut value = [0u8; 8];

    if base58::decode_len(string.len()) > 8 {
        return Err(ParseIdError::TooLong);
    }

    base58::decode_to_slice(string.as_bytes(), &mut value)?;
    let value = u64::from_le_bytes(value);
    match NonZeroU64::new(value) {
        None => Err(ParseIdError::ZeroId),
        Some(value) => Ok(value),
    }
}

/// Generates ID values.
pub trait GenId {
    /// Type of generated values for IDs.
    type Value;

    /// Generates new ID value.
    fn generate(&mut self) -> Self::Value;
}

impl<T> GenId for &mut T
where
    T: GenId,
{
    type Value = T::Value;

    fn generate(&mut self) -> T::Value {
        (**self).generate()
    }
}

/// Marker trait that signals that ID values are generated
/// in a way that guarantees or at least tries to guarantee uniqueness.
pub trait GenUid: GenId {}

impl<T> GenUid for &mut T where T: GenUid {}

#[doc(hidden)]
pub mod for_macro {
    pub use ::serde::{de, ser};
    pub use arcana_hash::{mix_hash_with_string, stable_hasher};
    pub use arcana_proc::stable_hash_tokens;
}
