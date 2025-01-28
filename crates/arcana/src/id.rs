//! Strong id utility.

use std::{
    fmt,
    hash::Hash,
    num::NonZeroU64,
    thread::sleep,
    time::{Duration, SystemTime},
};

use crate::base58::{base58_dec_len, base58_dec_slice, base58_enc_fmt, Base58DecodingError};

pub trait Id: fmt::Debug + Copy + Ord + Eq + Hash {
    fn new(value: NonZeroU64) -> Self;
    fn get(self) -> u64;
    fn get_nonzero(self) -> NonZeroU64;
}

/// Marker trait that signals that id is generated
/// in a way that guarantees or at least tries to guarantee uniqueness.
///
/// There's is no way to truly guarantee uniqueness of generated ids
/// without some kind of global coordination.
pub trait Uid: Id {}

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
        let mut hasher = $crate::hash::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000;
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($($value:expr),+ => $id:ty) => {{
        let mut hasher = $crate::hash::stable_hasher();
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
macro_rules! name_hash_id {
    ($ident:ident) => {{
        let hash = const { $crate::stable_hash_tokens!($ident) | 0x8000_0000_0000_0000 };
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($ident:ident => $id:ty) => {
        const {
            let hash = $crate::stable_hash_tokens!($ident) | 0x8000_0000_0000_0000;
            <$id>::new(unsafe { ::core::num::NonZeroU64::new_unchecked(hash) })
        }
    };
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
        let mut hasher = $crate::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher);
        let hash = $crate::mix_hash_with_string(hash, ::core::module_path!()) | 0x8000_0000_0000_0000;
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($($value:expr),+ => $id:ty) => {{
        let mut hasher = $crate::stable_hasher();
        $(::core::hash::Hash::hash(&{$value}, &mut hasher);)+
        let hash = ::core::hash::Hasher::finish(&hasher);
        let hash = $crate::mix_hash_with_string(hash, ::core::module_path!()) | 0x8000_0000_0000_0000;
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
macro_rules! local_name_hash_id {
    ($ident:ident) => {{
        let hash = const {
            let hash = $crate::stable_hash_tokens!($ident);
            $crate::hash::mix_hash_with_string(hash, ::core::module_path!()) | 0x8000_0000_0000_0000
        };
        $crate::Id::new(::core::num::NonZeroU64::new(hash).unwrap())
    }};
    ($ident:ident => $id:ty) => {
        const {
            let hash = $crate::stable_hash_tokens!($ident);
            let hash = $crate::hash::mix_hash_with_string(hash, ::core::module_path!())
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
                $crate::id::fmt_id(self.value.get(), f)
            }
        }

        impl ::core::fmt::Display for $name {
            #[inline(always)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::id::fmt_id(self.value.get(), f)
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
            type Err = $crate::id::ParseIdError;

            #[inline(always)]
            fn from_str(s: &str) -> Result<Self, $crate::id::ParseIdError> {
                let value = $crate::id::parse_id(s)?;
                Ok($name { value })
            }
        }

        impl ::core::convert::TryFrom<u64> for $name {
            type Error = $crate::id::ZeroIDError;

            fn try_from(value: u64) -> Result<Self, $crate::id::ZeroIDError> {
                match ::core::num::NonZeroU64::try_from(value) {
                    Ok(value) => Ok($name { value }),
                    Err(_) => Err($crate::id::ZeroIDError),
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
            pub fn generate(generator: &mut impl $crate::id::GenId<Value = ::core::num::NonZeroU64>) -> Self {
                $name::new(generator.generate())
            }
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseIdError {
    #[error(transparent)]
    DecodingError(#[from] Base58DecodingError),

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
            pub fn generate(generator: &mut impl $crate::id::GenUid<Value = ::core::num::NonZeroU64>) -> Self {
                $name::new(generator.generate())
            }
        }

        impl ::serde::Serialize for $name {
            #[inline(always)]
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                self.value.serialize(serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            #[inline(always)]
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct Visitor;

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    #[inline(always)]
                    fn expecting(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                        write!(f, "a non-zero 64-bit integer or a hex string")
                    }

                    #[inline(always)]
                    fn visit_u64<E>(self, v: u64) -> Result<$name, E>
                    where
                        E: ::serde::de::Error,
                    {
                        match ::core::num::NonZeroU64::new(v) {
                            None => Err(E::invalid_value(::serde::de::Unexpected::Unsigned(0), &self)),
                            Some(value) => Ok($name { value }),
                        }
                    }

                    #[inline(always)]
                    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        if v <= 0 {
                            Err(E::invalid_value(::serde::de::Unexpected::Signed(v), &self))
                        } else {
                            Ok($name { value: ::core::num::NonZeroU64::new(v as u64).unwrap() })
                        }
                    }

                    #[inline(always)]
                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
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

#[cfg_attr(feature = "inline-more", inline(always))]
#[doc(hidden)]
pub fn fmt_id(value: u64, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    base58_enc_fmt(&value.to_le_bytes(), &mut *f)
}

#[cfg_attr(feature = "inline-more", inline(always))]
#[doc(hidden)]
pub fn parse_id(string: &str) -> Result<NonZeroU64, ParseIdError> {
    let mut value = [0u8; 8];

    if base58_dec_len(string.len()) > 8 {
        return Err(ParseIdError::TooLong);
    }

    base58_dec_slice(string.as_bytes(), &mut value)?;
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

/// Marker trait that signals that ID values are generated
/// in a way that guarantees or at least tries to guarantee uniqueness.
pub trait GenUid: GenId {}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SeqIdGen {
    next_id: u64,
}

impl Default for SeqIdGen {
    #[inline(always)]
    fn default() -> Self {
        SeqIdGen::new()
    }
}

impl SeqIdGen {
    #[inline(always)]
    pub const fn new() -> Self {
        SeqIdGen { next_id: 1 }
    }

    #[inline(always)]
    pub const fn next(&mut self) -> NonZeroU64 {
        if self.next_id == 0 {
            panic!("SeqIdGen overflow");
        }

        let value = NonZeroU64::new(self.next_id).unwrap();
        self.next_id += 1;
        value
    }
}

impl GenId for SeqIdGen {
    type Value = NonZeroU64;

    #[inline(always)]
    fn generate(&mut self) -> NonZeroU64 {
        self.next()
    }
}

/// Time based ID generator.
/// Uses seconds since predefined moment as a base for ID values.
/// Adds counter for sub-second generated IDs.
/// And generator ID to avoid collisions between different instances.
pub struct TimeUidGen {
    /// counter for sub-second IDs generated.
    ///
    /// Only 10 bits are actually used.
    counter: u16,

    /// Contains second number of last generated ID.
    /// If new ID is generated in the same second, counter is incremented.
    /// Otherwise new second is used and counter is reset.
    ///
    /// Only 34 bits are actually used.
    second: u64,

    /// Generator ID.
    /// Used to avoid collisions between different instances.
    ///
    /// Only 20 bits are actually used.
    generator_id: u32,

    /// Start time for the generator.
    start: SystemTime,
}

impl TimeUidGen {
    pub fn with_start(generator_id: u32, start: SystemTime) -> Self {
        let generator_id = generator_id & 0xFFFFF;

        let now = SystemTime::now();

        // Find second number from the start.
        let second = now
            .duration_since(start)
            .expect("start for TimeUidGen is in the future")
            .as_secs();

        // Counter is initialized in a way that will require
        // second to pass before first ID is generated.
        // This avoids collisions with IDs generated by previous run of the process in the same second.
        TimeUidGen {
            counter: 0x3FF,
            second,
            generator_id,
            start,
        }
    }

    pub fn random_with_start(start: SystemTime) -> Self {
        let generator_id = rand::random::<u32>() & 0xFFFFF;
        TimeUidGen::with_start(generator_id, start)
    }

    pub fn random() -> Self {
        TimeUidGen::random_with_start(SystemTime::UNIX_EPOCH)
    }

    pub fn next(&mut self) -> NonZeroU64 {
        loop {
            let now = SystemTime::now();
            let second = now
                .duration_since(self.start)
                .expect("TimeUidGen start is in the future")
                .as_secs();

            if second > 0x3FFFFFFFF {
                panic!("Too distant future");
            }

            if second != self.second {
                self.second = second;
                self.counter = 0;
            }

            self.counter = self.counter.wrapping_add(1);
            if self.counter & 0x3FF == 0 {
                // In rare case when we generate IDs faster than one per millisecond
                // we need to wait for next second.
                let next_second = self.start + Duration::from_secs(second + 1);
                let duration = next_second.duration_since(now).unwrap();
                sleep(duration);
                continue;
            }

            self.second = second;

            let counter = (self.counter & 0x3FF) as u64;
            let second = self.second & 0x3FFFFFFFF;
            let generator_id = (self.generator_id & 0xFFFFF) as u64;

            let id = (second << 30) | (counter << 20) | generator_id;

            // counter is never 0.
            return NonZeroU64::new(id).unwrap();
        }
    }
}

impl GenId for TimeUidGen {
    type Value = NonZeroU64;

    #[inline(always)]
    fn generate(&mut self) -> NonZeroU64 {
        self.next()
    }
}

impl GenUid for TimeUidGen {}
