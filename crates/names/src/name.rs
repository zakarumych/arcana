use std::{
    borrow::Borrow,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

use crate::{intern::INTERNER, Ident};

#[macro_export]
macro_rules! name {
    ($i:ident) => {
        $crate::Name::from_name_str(stringify!($i))
    };
}

/// String wrapper that ensures it is a valid unicode identifier.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Name {
    s: &'static str,
}

impl Name {
    #[cfg_attr(inline_more, inline)]
    pub fn from_str<S>(s: &S) -> Result<Self, NameError>
    where
        S: AsRef<str> + ?Sized,
    {
        let s = s.as_ref();
        validate_name(s)?;

        let s = INTERNER.intern(s);
        Ok(Name::from_name_str(s))
    }

    /// This function is safe because it cannot be used
    /// to violate Rust safety rules.
    ///
    /// However, it is possible to create invalid `Name` that may cause unexpected behavior.
    #[inline(always)]
    pub const fn from_name_str(s: &'static str) -> Self {
        Name { s }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &'static str {
        self.s
    }

    #[inline(always)]
    pub fn from_ident(ident: Ident) -> Self {
        Name::from_name_str(ident.as_str())
    }
}

impl From<Ident> for Name {
    #[inline(always)]
    fn from(value: Ident) -> Self {
        Name::from_ident(value)
    }
}

impl Hash for Name {
    #[cfg_attr(inline_more, inline(always))]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.s.hash(state)
    }
}

impl Deref for Name {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        &self.s
    }
}

impl AsRef<str> for Name {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &self.s
    }
}

impl Borrow<str> for Name {
    #[inline(always)]
    fn borrow(&self) -> &str {
        &self.s
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.s, f)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.s, f)
    }
}

impl PartialEq<Name> for Name {
    #[inline(always)]
    fn eq(&self, other: &Name) -> bool {
        std::ptr::eq(self.s, other.s) || self.s == other.s
    }

    #[inline(always)]
    fn ne(&self, other: &Name) -> bool {
        (!std::ptr::eq(self.s, other.s)) && self.s != other.s
    }
}

impl Eq for Name {}

impl PartialEq<str> for Name {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        std::ptr::eq(self.s, other) || self.s == other
    }

    #[inline(always)]
    fn ne(&self, other: &str) -> bool {
        (!std::ptr::eq(self.s, other)) && self.s != other
    }
}

impl PartialEq<&str> for Name {
    #[inline(always)]
    fn eq(&self, other: &&str) -> bool {
        std::ptr::eq(self.s, *other) || self.s == *other
    }

    #[inline(always)]
    fn ne(&self, other: &&str) -> bool {
        (!std::ptr::eq(self.s, *other)) && self.s != *other
    }
}

impl PartialEq<String> for Name {
    #[inline(always)]
    fn eq(&self, other: &String) -> bool {
        std::ptr::eq(self.s, other.as_str()) || self.s == other.as_str()
    }

    #[inline(always)]
    fn ne(&self, other: &String) -> bool {
        (!std::ptr::eq(self.s, other.as_str())) && self.s != other.as_str()
    }
}

impl PartialOrd<Name> for Name {
    #[inline(always)]
    fn partial_cmp(&self, other: &Name) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<str> for Name {
    #[inline(always)]
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        Some(self.s.cmp(other))
    }
}

impl PartialOrd<&str> for Name {
    #[inline(always)]
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        Some(self.s.cmp(*other))
    }
}

impl PartialOrd<String> for Name {
    #[inline(always)]
    fn partial_cmp(&self, other: &String) -> Option<std::cmp::Ordering> {
        Some(self.s.cmp(other.as_str()))
    }
}

impl Ord for Name {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if std::ptr::eq(self.s, other.as_str()) {
            return std::cmp::Ordering::Equal;
        }

        self.s.cmp(other.as_str())
    }
}

impl serde::Serialize for Name {
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&self.s, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Name {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const EXPECTED_NAME: &'static str =
            "Expected non-empty unicode string without control characters";

        struct IdentVisitor;

        impl<'de> serde::de::Visitor<'de> for IdentVisitor {
            type Value = Name;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(EXPECTED_NAME)
            }

            fn visit_str<E>(self, s: &str) -> Result<Name, E>
            where
                E: serde::de::Error,
            {
                match Name::from_str(s) {
                    Ok(ident) => Ok(ident),
                    Err(NameError::Empty) => {
                        Err(serde::de::Error::invalid_length(1, &EXPECTED_NAME))
                    }
                    Err(NameError::Bad(c)) => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Char(c),
                        &EXPECTED_NAME,
                    )),
                }
            }
        }

        deserializer.deserialize_str(IdentVisitor)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Bad(char),
}

impl fmt::Debug for NameError {
    #[cfg_attr(inline_more, inline)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for NameError {
    #[cfg_attr(inline_more, inline)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameError::Empty => write!(f, "Name must not be empty"),
            NameError::Bad(c) => write!(
                f,
                "'{}' is not valid Name. 2nd and later chars must have XID_Continue property. Try latin letter or digit",
                c
            ),
        }
    }
}

/// Validates that string has no control characters.
/// Returns error if it is not.
pub fn validate_name(s: &str) -> Result<(), NameError> {
    if s.is_empty() {
        return Err(NameError::Empty);
    }

    let bad = |c: char| c.is_control();

    match s.find(bad) {
        None => Ok(()),
        Some(pos) => {
            return Err(NameError::Bad(s[pos..].chars().next().unwrap()));
        }
    }
}
