//! Hashing stuff.

mod noop;
mod sha2;
mod stable;

use core::fmt;

use crate::base58::{base58_dec_slice, base58_enc_fmt, base58_enc_str};

pub use self::{
    noop::{no_hash_map, NoHashBuilder, NoHashMap, NoHasher},
    sha2::{sha256, sha256_file, sha256_io, sha512, sha512_file, sha512_io},
    stable::{
        hue_hash, mix_hash_with_string, rgb_hash, rgba_hash, rgba_premultiplied_hash, stable_hash,
        stable_hash_file, stable_hash_map, stable_hash_read, stable_hasher, StableHashBuilder,
        StableHashMap,
    },
};

/// 64-bit hash value.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash64(pub [u64; 1]);

impl Hash64 {
    pub const fn from_u8(data: [u8; 8]) -> Self {
        Hash64(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u16(data: [u16; 4]) -> Self {
        Hash64(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u32(data: [u32; 2]) -> Self {
        Hash64(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u64(data: [u64; 1]) -> Self {
        Hash64(unsafe { core::mem::transmute(data) })
    }

    pub const fn as_u8(&self) -> &[u8; 8] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u16(&self) -> &[u16; 4] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u32(&self) -> &[u32; 2] {
        unsafe { core::mem::transmute(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> &[u64; 1] {
        &self.0
    }
}

impl serde::Serialize for Hash64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut encoded = String::new();
            base58_enc_str(self.as_u8(), &mut encoded);
            serializer.serialize_str(&encoded)
        } else {
            serializer.serialize_bytes(self.as_u8())
        }
    }
}

impl<'de> serde::Deserialize<'de> for Hash64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Hash64Visitor;

        impl<'de> serde::de::Visitor<'de> for Hash64Visitor {
            type Value = Hash64;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base58 encoded 64-bit hash or 8 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Hash64, E>
            where
                E: serde::de::Error,
            {
                let mut hash = [0; 8];
                base58_dec_slice(value.as_bytes(), &mut hash).map_err(serde::de::Error::custom)?;
                Ok(Hash64::from_u8(hash))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Hash64, E>
            where
                E: serde::de::Error,
            {
                if value.len() != 8 {
                    return Err(serde::de::Error::invalid_length(value.len(), &"8 bytes"));
                }

                let mut hash = [0; 8];
                hash.copy_from_slice(value);
                Ok(Hash64::from_u8(hash))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Hash64Visitor)
        } else {
            deserializer.deserialize_bytes(Hash64Visitor)
        }
    }
}

impl fmt::Debug for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::LowerHex::fmt(self, f)
        } else {
            base58_enc_fmt(self.as_u8(), f)
        }
    }
}

impl fmt::Display for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(self.as_u8(), f)
    }
}

impl fmt::LowerHex for Hash64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a] = self.as_u64();

        if let Some(width) = f.width() {
            if f.sign_aware_zero_pad() {
                let width = width.min(16);
                if f.alternate() {
                    write!(f, "{:#0width$x}", a)
                } else {
                    write!(f, "{:0width$x}", a)
                }
            } else {
                if f.alternate() {
                    let width = width.saturating_sub(18);
                    write!(f, "{:width$}{:#016x}", "", a)
                } else {
                    let width = width.saturating_sub(16);
                    write!(f, "{:width$}{:016x}", "", a)
                }
            }
        } else {
            if f.alternate() {
                write!(f, "{:#016x}", a)
            } else {
                write!(f, "{:016x}", a)
            }
        }
    }
}

impl fmt::UpperHex for Hash64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a] = self.as_u64();

        if let Some(width) = f.width() {
            if f.sign_aware_zero_pad() {
                let width = width.min(16);
                if f.alternate() {
                    write!(f, "{:#0width$X}", a)
                } else {
                    write!(f, "{:0width$X}", a)
                }
            } else {
                if f.alternate() {
                    let width = width.saturating_sub(18);
                    write!(f, "{:width$}{:#016X}", "", a)
                } else {
                    let width = width.saturating_sub(16);
                    write!(f, "{:width$}{:016X}", "", a)
                }
            }
        } else {
            if f.alternate() {
                write!(f, "{:#016X}", a)
            } else {
                write!(f, "{:016X}", a)
            }
        }
    }
}

/// 128-bit hash value.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash128(pub [u64; 2]);

impl Hash128 {
    pub const fn from_u8(data: [u8; 16]) -> Self {
        Hash128(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u16(data: [u16; 8]) -> Self {
        Hash128(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u32(data: [u32; 4]) -> Self {
        Hash128(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u64(data: [u64; 2]) -> Self {
        Hash128(data)
    }

    pub const fn as_u8(&self) -> &[u8; 16] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u16(&self) -> &[u16; 8] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u32(&self) -> &[u32; 4] {
        unsafe { core::mem::transmute(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> &[u64; 2] {
        &self.0
    }
}

impl serde::Serialize for Hash128 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut encoded = String::new();
            base58_enc_str(self.as_u8(), &mut encoded);
            serializer.serialize_str(&encoded)
        } else {
            serializer.serialize_bytes(self.as_u8())
        }
    }
}

impl<'de> serde::Deserialize<'de> for Hash128 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Hash128Visitor;

        impl<'de> serde::de::Visitor<'de> for Hash128Visitor {
            type Value = Hash128;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base58 encoded 128-bit hash or 16 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Hash128, E>
            where
                E: serde::de::Error,
            {
                let mut hash = [0; 16];
                base58_dec_slice(value.as_bytes(), &mut hash).map_err(serde::de::Error::custom)?;
                Ok(Hash128::from_u8(hash))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Hash128, E>
            where
                E: serde::de::Error,
            {
                if value.len() != 16 {
                    return Err(serde::de::Error::invalid_length(value.len(), &"16 bytes"));
                }

                let mut hash = [0; 16];
                hash.copy_from_slice(value);
                Ok(Hash128::from_u8(hash))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Hash128Visitor)
        } else {
            deserializer.deserialize_bytes(Hash128Visitor)
        }
    }
}

impl fmt::Debug for Hash128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::LowerHex::fmt(self, f)
        } else {
            base58_enc_fmt(self.as_u8(), f)
        }
    }
}

impl fmt::Display for Hash128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(self.as_u8(), f)
    }
}

impl fmt::LowerHex for Hash128 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b] = self.as_u64();

        if let Some(width) = f.width() {
            if f.sign_aware_zero_pad() {
                let width = width.min(32) - 16;
                if f.alternate() {
                    write!(f, "{:#0width$x}{:016x}", a, b)
                } else {
                    write!(f, "{:0width$x}{:016x}", a, b)
                }
            } else {
                if f.alternate() {
                    let width = width.saturating_sub(34);
                    write!(f, "{:width$}{:#016x}{:016x}", "", a, b)
                } else {
                    let width = width.saturating_sub(32);
                    write!(f, "{:width$}{:016x}{:016x}", "", a, b)
                }
            }
        } else {
            if f.alternate() {
                write!(f, "{:#016x}{:016x}", a, b)
            } else {
                write!(f, "{:016x}{:016x}", a, b)
            }
        }
    }
}

impl fmt::UpperHex for Hash128 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b] = self.as_u64();

        if let Some(width) = f.width() {
            if f.sign_aware_zero_pad() {
                let width = width.min(32) - 16;
                if f.alternate() {
                    write!(f, "{:#0width$X}{:016X}", a, b)
                } else {
                    write!(f, "{:0width$X}{:016X}", a, b)
                }
            } else {
                if f.alternate() {
                    let width = width.saturating_sub(34);
                    write!(f, "{:width$}{:#016X}{:016X}", "", a, b)
                } else {
                    let width = width.saturating_sub(32);
                    write!(f, "{:width$}{:016X}{:016X}", "", a, b)
                }
            }
        } else {
            if f.alternate() {
                write!(f, "{:#016X}{:016X}", a, b)
            } else {
                write!(f, "{:016X}{:016X}", a, b)
            }
        }
    }
}

/// 256-bit hash value.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Hash256(pub [u64; 4]);

impl Hash256 {
    pub const fn from_u8(data: [u8; 32]) -> Self {
        Hash256(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u16(data: [u16; 16]) -> Self {
        Hash256(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u32(data: [u32; 8]) -> Self {
        Hash256(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u64(data: [u64; 4]) -> Self {
        Hash256(data)
    }

    pub const fn as_u8(&self) -> &[u8; 32] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u16(&self) -> &[u16; 16] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u32(&self) -> &[u32; 8] {
        unsafe { core::mem::transmute(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> &[u64; 4] {
        &self.0
    }
}

impl serde::Serialize for Hash256 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut encoded = String::new();
            base58_enc_str(self.as_u8(), &mut encoded);
            serializer.serialize_str(&encoded)
        } else {
            serializer.serialize_bytes(self.as_u8())
        }
    }
}

impl<'de> serde::Deserialize<'de> for Hash256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Hash256Visitor;

        impl<'de> serde::de::Visitor<'de> for Hash256Visitor {
            type Value = Hash256;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base58 encoded 128-bit hash or 16 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Hash256, E>
            where
                E: serde::de::Error,
            {
                let mut hash = [0; 32];
                base58_dec_slice(value.as_bytes(), &mut hash).map_err(serde::de::Error::custom)?;
                Ok(Hash256::from_u8(hash))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Hash256, E>
            where
                E: serde::de::Error,
            {
                if value.len() != 32 {
                    return Err(serde::de::Error::invalid_length(value.len(), &"16 bytes"));
                }

                let mut hash = [0; 32];
                hash.copy_from_slice(value);
                Ok(Hash256::from_u8(hash))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Hash256Visitor)
        } else {
            deserializer.deserialize_bytes(Hash256Visitor)
        }
    }
}

impl fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::LowerHex::fmt(self, f)
        } else {
            base58_enc_fmt(self.as_u8(), f)
        }
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(self.as_u8(), f)
    }
}

impl fmt::LowerHex for Hash256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d] = self.as_u64();

        if let Some(width) = f.width() {
            if f.sign_aware_zero_pad() {
                let width = width.min(64) - 48;
                if f.alternate() {
                    write!(f, "{:#0width$x}{:016x}{:016x}{:016x}", a, b, c, d)
                } else {
                    write!(f, "{:0width$x}{:016x}{:016x}{:016x}", a, b, c, d)
                }
            } else {
                if f.alternate() {
                    let width = width.saturating_sub(66);
                    write!(f, "{:width$}{:#016x}{:016x}{:016x}{:016x}", "", a, b, c, d)
                } else {
                    let width = width.saturating_sub(64);
                    write!(f, "{:width$}{:016x}{:016x}{:016x}{:016x}", "", a, b, c, d)
                }
            }
        } else {
            if f.alternate() {
                write!(f, "{:#016x}{:016x}{:016x}{:016x}", a, b, c, d)
            } else {
                write!(f, "{:016x}{:016x}{:016x}{:016x}", a, b, c, d)
            }
        }
    }
}

impl fmt::UpperHex for Hash256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d] = self.as_u64();

        if let Some(width) = f.width() {
            if f.sign_aware_zero_pad() {
                let width = width.min(64) - 48;
                if f.alternate() {
                    write!(f, "{:#0width$X}{:016X}{:016X}{:016X}", a, b, c, d)
                } else {
                    write!(f, "{:0width$X}{:016X}{:016X}{:016X}", a, b, c, d)
                }
            } else {
                if f.alternate() {
                    let width = width.saturating_sub(66);
                    write!(f, "{:width$}{:#016X}{:016X}{:016X}{:016X}", "", a, b, c, d)
                } else {
                    let width = width.saturating_sub(64);
                    write!(f, "{:width$}{:016X}{:016X}{:016X}{:016X}", "", a, b, c, d)
                }
            }
        } else {
            if f.alternate() {
                write!(f, "{:#016X}{:016X}{:016X}{:016X}", a, b, c, d)
            } else {
                write!(f, "{:016X}{:016X}{:016X}{:016X}", a, b, c, d)
            }
        }
    }
}

/// 512-bit hash value.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Hash512(pub [u64; 8]);

impl Hash512 {
    pub const fn from_u8(data: [u8; 64]) -> Self {
        Hash512(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u16(data: [u16; 32]) -> Self {
        Hash512(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u32(data: [u32; 16]) -> Self {
        Hash512(unsafe { core::mem::transmute(data) })
    }

    pub const fn from_u64(data: [u64; 8]) -> Self {
        Hash512(data)
    }

    pub const fn as_u8(&self) -> &[u8; 64] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u16(&self) -> &[u16; 32] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub const fn as_u32(&self) -> &[u32; 16] {
        unsafe { core::mem::transmute(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> &[u64; 8] {
        &self.0
    }
}

impl serde::Serialize for Hash512 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut encoded = String::new();
            base58_enc_str(self.as_u8(), &mut encoded);
            serializer.serialize_str(&encoded)
        } else {
            serializer.serialize_bytes(self.as_u8())
        }
    }
}

impl<'de> serde::Deserialize<'de> for Hash512 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Hash512Visitor;

        impl<'de> serde::de::Visitor<'de> for Hash512Visitor {
            type Value = Hash512;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base58 encoded 128-bit hash or 16 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Hash512, E>
            where
                E: serde::de::Error,
            {
                let mut hash = [0; 64];
                base58_dec_slice(value.as_bytes(), &mut hash).map_err(serde::de::Error::custom)?;
                Ok(Hash512::from_u8(hash))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Hash512, E>
            where
                E: serde::de::Error,
            {
                if value.len() != 64 {
                    return Err(serde::de::Error::invalid_length(value.len(), &"16 bytes"));
                }

                let mut hash = [0; 64];
                hash.copy_from_slice(value);
                Ok(Hash512::from_u8(hash))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Hash512Visitor)
        } else {
            deserializer.deserialize_bytes(Hash512Visitor)
        }
    }
}

impl fmt::Debug for Hash512 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::LowerHex::fmt(self, f)
        } else {
            base58_enc_fmt(self.as_u8(), f)
        }
    }
}

impl fmt::Display for Hash512 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(self.as_u8(), f)
    }
}

impl fmt::LowerHex for Hash512 {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d, e, f, g, h] = self.as_u64();

        if let Some(width) = fmt.width() {
            if fmt.sign_aware_zero_pad() {
                let width = width.min(128) - 112;
                if fmt.alternate() {
                    write!(
                        fmt,
                        "{:#0width$x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                        a, b, c, d, e, f, g, h
                    )
                } else {
                    write!(
                        fmt,
                        "{:0width$x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                        a, b, c, d, e, f, g, h
                    )
                }
            } else {
                if fmt.alternate() {
                    let width = width.saturating_sub(130);
                    write!(
                        fmt,
                        "{:width$}{:#016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                        "", a, b, c, d, e, f, g, h
                    )
                } else {
                    let width = width.saturating_sub(128);
                    write!(
                        fmt,
                        "{:width$}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                        "", a, b, c, d, e, f, g, h
                    )
                }
            }
        } else {
            if fmt.alternate() {
                write!(
                    fmt,
                    "{:#016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                    a, b, c, d, e, f, g, h
                )
            } else {
                write!(
                    fmt,
                    "{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                    a, b, c, d, e, f, g, h
                )
            }
        }
    }
}

impl fmt::UpperHex for Hash512 {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d, e, f, g, h] = self.as_u64();

        if let Some(width) = fmt.width() {
            if fmt.sign_aware_zero_pad() {
                let width = width.min(128) - 112;
                if fmt.alternate() {
                    write!(
                        fmt,
                        "{:#0width$X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}",
                        a, b, c, d, e, f, g, h
                    )
                } else {
                    write!(
                        fmt,
                        "{:0width$X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}",
                        a, b, c, d, e, f, g, h
                    )
                }
            } else {
                if fmt.alternate() {
                    let width = width.saturating_sub(130);
                    write!(
                        fmt,
                        "{:width$}{:#016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}",
                        "", a, b, c, d, e, f, g, h
                    )
                } else {
                    let width = width.saturating_sub(128);
                    write!(
                        fmt,
                        "{:width$}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}",
                        "", a, b, c, d, e, f, g, h
                    )
                }
            }
        } else {
            if fmt.alternate() {
                write!(
                    fmt,
                    "{:#016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}",
                    a, b, c, d, e, f, g, h
                )
            } else {
                write!(
                    fmt,
                    "{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}{:016X}",
                    a, b, c, d, e, f, g, h
                )
            }
        }
    }
}
