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
        $crate::Name::from_name_static(stringify!($i))
    };
}

#[macro_export]
macro_rules! format_name {
    ($fmt:literal $(, $arg:expr)*) => { {
        $crate::Name::from_string(::std::format!($fmt $(, $arg)*)).unwrap()
    }};
}

#[macro_export]
macro_rules! try_format_name {
    ($fmt:literal $(, $arg:expr)*) => { {
        $crate::Name::from_string(::std::format!($fmt $(, $arg)*))
    }};
}

/// Interned string wrapper that ensures it is a valid name.
/// Names should not have control characters and should not be empty.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Name {
    pub(crate) s: &'static str,
}

impl Name {
    /// Creates a new `Name` from a static string.
    ///
    /// Returns an error if the string is empty or contains control characters.
    ///
    /// Valid string will be interned.
    ///
    /// Unlike `from_static`, this function will allocate storage for the string if it is not interned yet.
    pub fn from_static(s: &'static str) -> Result<Self, NameError> {
        validate_name(s)?;

        let s = INTERNER.intern_static(s);
        Ok(Name { s })
    }

    /// Creates a new `Name` from a string-like object.
    ///
    /// Returns an error if the string is empty or contains control characters.
    ///
    /// Valid string will be interned.
    pub fn from_str<S>(s: &S) -> Result<Self, NameError>
    where
        S: AsRef<str> + ?Sized,
    {
        let s = s.as_ref();
        validate_name(s)?;

        let s = INTERNER.intern(s);
        Ok(Name { s })
    }

    /// Creates a new `Name` from a `String` object.
    ///
    /// Returns an error if the string is empty or contains control characters.
    ///
    /// Valid string will be interned.
    ///
    /// Unlike `from_str`, this function takes ownership of the string and use it if it is not interned yet.
    pub fn from_string(s: String) -> Result<Self, NameError> {
        validate_name(&*s)?;

        let s = INTERNER.intern_string(s);
        Ok(Name { s })
    }

    /// Creates a new `Name` from a static string.
    ///
    /// Does not perform any validation.
    ///
    /// Otherwise same as `from_static`.
    #[inline(always)]
    pub fn from_name_static(s: &'static str) -> Self {
        debug_assert!(validate_name(s).is_ok());

        let s = INTERNER.intern_static(s);
        Name { s }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &'static str {
        self.s
    }

    #[inline(always)]
    pub fn from_ident(ident: Ident) -> Self {
        // NOTE: ident.as_str() is already interned.
        Name { s: ident.as_str() }
    }
}

impl From<Ident> for Name {
    #[inline(always)]
    fn from(value: Ident) -> Self {
        Name::from_ident(value)
    }
}

impl Hash for Name {
    #[inline(always)]
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
        // There's no reason to compare strings
        // because `Name`s are interned with deduplication.
        std::ptr::eq(self.s, other.s)
    }
}

impl Eq for Name {}

impl PartialEq<Ident> for Name {
    #[inline(always)]
    fn eq(&self, other: &Ident) -> bool {
        // There's no reason to compare strings
        // because `Name`s and `Ident`s are interned with deduplication.
        std::ptr::eq(self.s, other.s)
    }
}

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
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for NameError {
    #[inline]
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
#[inline(never)]
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
