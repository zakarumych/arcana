use std::{
    fmt::{self, Debug, Display, LowerHex, UpperHex},
    num::{NonZeroU64, ParseIntError},
    str::FromStr,
};

use serde::{
    de::{Error, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};

/// 64-bit id value.
/// FFI-safe.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AssetId(pub NonZeroU64);

impl Serialize for AssetId {
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use std::io::Write;

        if serializer.is_human_readable() {
            let mut hex = [0u8; 16];
            write!(std::io::Cursor::new(&mut hex[..]), "{:016x}", self.0).expect("Must fit");
            let hex = std::str::from_utf8(&hex).expect("Must be UTF-8");
            serializer.serialize_str(hex)
        } else {
            serializer.serialize_u64(self.0.get())
        }
    }
}

struct AssetIdVisitor;

impl<'de> serde::de::Visitor<'de> for AssetIdVisitor {
    type Value = AssetId;

    #[inline(always)]
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a non-zero 64-bit integer or a hex string")
    }

    #[inline(always)]
    fn visit_u64<E>(self, v: u64) -> Result<AssetId, E>
    where
        E: Error,
    {
        match NonZeroU64::new(v) {
            None => Err(E::invalid_value(Unexpected::Unsigned(0), &self)),
            Some(value) => Ok(AssetId(value)),
        }
    }

    #[inline(always)]
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        if v <= 0 {
            Err(E::invalid_value(Unexpected::Signed(v), &self))
        } else {
            Ok(AssetId(NonZeroU64::new(v as u64).unwrap()))
        }
    }

    #[inline(always)]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        v.parse().map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for AssetId {
    #[inline(always)]
    fn deserialize<D>(deserializer: D) -> Result<AssetId, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u64(AssetIdVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseAssetIdError {
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),

    #[error("AssetId cannot be zero")]
    ZeroId,
}

impl FromStr for AssetId {
    type Err = ParseAssetIdError;

    #[inline(always)]
    fn from_str(s: &str) -> Result<Self, ParseAssetIdError> {
        let value = u64::from_str_radix(s, 16)?;
        match NonZeroU64::new(value) {
            None => Err(ParseAssetIdError::ZeroId),
            Some(value) => Ok(AssetId(value)),
        }
    }
}

#[derive(Debug)]
pub struct ZeroIDError;

impl AssetId {
    #[inline(always)]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            None => None,
            Some(value) => Some(AssetId(value)),
        }
    }

    #[inline(always)]
    pub fn value(&self) -> NonZeroU64 {
        self.0
    }
}

impl From<NonZeroU64> for AssetId {
    #[inline(always)]
    fn from(value: NonZeroU64) -> Self {
        AssetId(value)
    }
}

impl TryFrom<u64> for AssetId {
    type Error = ZeroIDError;

    fn try_from(value: u64) -> Result<Self, ZeroIDError> {
        match NonZeroU64::try_from(value) {
            Ok(value) => Ok(AssetId(value)),
            Err(_) => Err(ZeroIDError),
        }
    }
}

impl Debug for AssetId {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LowerHex::fmt(&self.0.get(), f)
    }
}

impl UpperHex for AssetId {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        UpperHex::fmt(&self.0.get(), f)
    }
}

impl LowerHex for AssetId {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LowerHex::fmt(&self.0.get(), f)
    }
}

impl Display for AssetId {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LowerHex::fmt(&self.0.get(), f)
    }
}
