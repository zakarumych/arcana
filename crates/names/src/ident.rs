use std::{
    borrow::Borrow,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

use crate::{intern::INTERNER, Name};

#[macro_export]
macro_rules! ident {
    ($i:ident) => {
        $crate::Ident::from_ident_str(stringify!($i))
    };
}

/// String wrapper that ensures it is a valid unicode identifier.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Ident {
    pub(crate) s: &'static str,
}

impl Ident {
    pub fn from_str<S>(s: &S) -> Result<Self, IdentError>
    where
        S: AsRef<str> + ?Sized,
    {
        let s = s.as_ref();
        validate_ident(s)?;

        let s = INTERNER.intern(s);
        Ok(Ident { s })
    }

    /// This function is safe because it cannot be used
    /// to violate Rust safety rules.
    ///
    /// However, it is possible to create invalid `Ident` that may cause unexpected behavior.
    #[inline(always)]
    pub fn from_ident_str(s: &'static str) -> Self {
        let s = INTERNER.intern(s);
        Ident { s }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &'static str {
        self.s
    }
}

impl Hash for Ident {
    #[inline(always)]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.s.hash(state)
    }
}

impl Deref for Ident {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        &self.s
    }
}

impl AsRef<str> for Ident {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &self.s
    }
}

impl Borrow<str> for Ident {
    #[inline(always)]
    fn borrow(&self) -> &str {
        &self.s
    }
}

impl fmt::Debug for Ident {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.s, f)
    }
}

impl fmt::Display for Ident {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.s, f)
    }
}

impl PartialEq<Ident> for Ident {
    #[inline(always)]
    fn eq(&self, other: &Ident) -> bool {
        std::ptr::eq(self.s, other.s) || self.s == other.s
    }

    #[inline(always)]
    fn ne(&self, other: &Ident) -> bool {
        (!std::ptr::eq(self.s, other.s)) && self.s != other.s
    }
}

impl Eq for Ident {}

impl PartialEq<Name> for Ident {
    #[inline(always)]
    fn eq(&self, other: &Name) -> bool {
        // There's no reason to compare strings
        // because `Name`s and `Ident`s are interned with deduplication.
        std::ptr::eq(self.s, other.s)
    }
}

impl PartialEq<str> for Ident {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        std::ptr::eq(self.s, other) || self.s == other
    }

    #[inline(always)]
    fn ne(&self, other: &str) -> bool {
        (!std::ptr::eq(self.s, other)) && self.s != other
    }
}

impl PartialEq<&str> for Ident {
    #[inline(always)]
    fn eq(&self, other: &&str) -> bool {
        std::ptr::eq(self.s, *other) || self.s == *other
    }

    #[inline(always)]
    fn ne(&self, other: &&str) -> bool {
        (!std::ptr::eq(self.s, *other)) && self.s != *other
    }
}

impl PartialEq<String> for Ident {
    #[inline(always)]
    fn eq(&self, other: &String) -> bool {
        std::ptr::eq(self.s, other.as_str()) || self.s == other.as_str()
    }

    #[inline(always)]
    fn ne(&self, other: &String) -> bool {
        (!std::ptr::eq(self.s, other.as_str())) && self.s != other.as_str()
    }
}

impl PartialOrd<Ident> for Ident {
    #[inline(always)]
    fn partial_cmp(&self, other: &Ident) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<str> for Ident {
    #[inline(always)]
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        Some(self.s.cmp(other))
    }
}

impl PartialOrd<&str> for Ident {
    #[inline(always)]
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        Some(self.s.cmp(*other))
    }
}

impl PartialOrd<String> for Ident {
    #[inline(always)]
    fn partial_cmp(&self, other: &String) -> Option<std::cmp::Ordering> {
        Some(self.s.cmp(other.as_str()))
    }
}

impl Ord for Ident {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if std::ptr::eq(self.s, other.as_str()) {
            return std::cmp::Ordering::Equal;
        }

        self.s.cmp(other.as_str())
    }
}

impl serde::Serialize for Ident {
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&self.s, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Ident {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const EXPECTED_IDENT: &'static str = "Expected unicode identifier";

        struct IdentVisitor;

        impl<'de> serde::de::Visitor<'de> for IdentVisitor {
            type Value = Ident;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(EXPECTED_IDENT)
            }

            fn visit_str<E>(self, s: &str) -> Result<Ident, E>
            where
                E: serde::de::Error,
            {
                match Ident::from_str(s) {
                    Ok(ident) => Ok(ident),
                    Err(IdentError::Empty) => {
                        Err(serde::de::Error::invalid_length(1, &EXPECTED_IDENT))
                    }
                    Err(IdentError::BadFirst(c)) => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Char(c),
                        &EXPECTED_IDENT,
                    )),
                    Err(IdentError::Bad(c)) => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Char(c),
                        &EXPECTED_IDENT,
                    )),
                }
            }
        }

        deserializer.deserialize_str(IdentVisitor)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdentError {
    Empty,
    BadFirst(char),
    Bad(char),
}

impl fmt::Debug for IdentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for IdentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentError::Empty => write!(f, "Ident must not be empty"),
            IdentError::BadFirst(c) => write!(
                f,
                "'{}' is not valid Ident. First char must have XID_Start property. Try latin letter",
                c
            ),
            IdentError::Bad(c) => write!(
                f,
                "'{}' is not valid Ident. 2nd and later chars must have XID_Continue property. Try latin letter or digit",
                c
            ),
        }
    }
}

/// Validates that string is a valid unicode identifier.
/// Returns error if it is not.
pub fn validate_ident(s: &str) -> Result<(), IdentError> {
    if s.is_empty() {
        return Err(IdentError::Empty);
    }

    let bad_first = |c: char| !unicode_ident::is_xid_start(c);

    if s.starts_with(bad_first) {
        return Err(IdentError::BadFirst(s.chars().next().unwrap()));
    }

    let bad = |c: char| !unicode_ident::is_xid_continue(c);

    match s.find(bad) {
        None => Ok(()),
        Some(pos) => {
            return Err(IdentError::Bad(s[pos..].chars().next().unwrap()));
        }
    }
}
