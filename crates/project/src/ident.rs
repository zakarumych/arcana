use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

#[macro_export]
macro_rules! ident {
    ($i:ident) => {
        $crate::Ident::from_ident_str(stringify!($i))
    };
}

/// String wrapper that ensures it is a valid unicode identifier.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Ident {
    s: str,
}

impl Ident {
    pub fn from_str<S>(s: &S) -> miette::Result<&Self>
    where
        S: AsRef<str> + ?Sized,
    {
        let s = s.as_ref();
        validate(s)?;
        Ok(Ident::from_ident_str(s))
    }

    /// This function is safe because it cannot be used
    /// to violate Rust safety rules.
    ///
    /// However, it is possible to create invalid `Ident` that may cause unexpected behavior.
    pub const fn from_ident_str(s: &str) -> &Self {
        // Safety: Types are layout compatible.
        unsafe { &*(s as *const str as *const Ident) }
    }

    pub fn to_buf(&self) -> IdentBuf {
        IdentBuf {
            s: self.s.to_owned(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.s
    }
}

impl Hash for Ident {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.s.hash(state)
    }
}

impl Deref for Ident {
    type Target = str;

    #[inline(never)]
    fn deref(&self) -> &str {
        &self.s
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        &self.s
    }
}

impl Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        &self.s
    }
}

impl fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.s, f)
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.s, f)
    }
}

impl PartialEq<IdentBuf> for Ident {
    fn eq(&self, other: &IdentBuf) -> bool {
        self.s == *other.s
    }
}

impl PartialOrd<IdentBuf> for Ident {
    fn partial_cmp(&self, other: &IdentBuf) -> Option<Ordering> {
        Some(self.s.cmp(&*other.s))
    }
}

impl serde::Serialize for Ident {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&self.s, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for &'de Ident {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const EXPECTED_IDENT: &'static str = "Expected unicode identifier";

        let s: &'de str = serde::Deserialize::deserialize(deserializer)?;

        if s.is_empty() {
            return Err(serde::de::Error::invalid_length(1, &EXPECTED_IDENT));
        }

        let good_first = unicode_ident::is_xid_start;
        let bad_first = |c: char| !good_first(c);

        if s.starts_with(bad_first) {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(s.chars().next().unwrap()),
                &EXPECTED_IDENT,
            ));
        }

        let good = unicode_ident::is_xid_continue;
        let bad = |c: char| !good(c);

        match s.matches(bad).next() {
            None => Ok(Ident::from_ident_str(s)),
            Some(c) => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(c),
                &EXPECTED_IDENT,
            )),
        }
    }
}

/// String wrapper that ensures it is a valid unicode identifier.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct IdentBuf {
    s: String,
}

impl IdentBuf {
    pub fn from_string(s: String) -> miette::Result<Self> {
        validate(&s)?;
        Ok(IdentBuf { s })
    }

    pub fn from_str<S>(s: S) -> miette::Result<Self>
    where
        S: AsRef<str>,
    {
        let s = s.as_ref();
        validate(s)?;
        Ok(IdentBuf { s: s.to_owned() })
    }

    pub fn from_ident_string(s: String) -> Self {
        IdentBuf { s }
    }

    pub fn into_string(self) -> String {
        self.s
    }
}

impl Hash for IdentBuf {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (*self.s).hash(state)
    }
}

impl Deref for IdentBuf {
    type Target = Ident;

    #[inline(never)]
    fn deref(&self) -> &Ident {
        Ident::from_ident_str(&*self.s)
    }
}

impl AsRef<Ident> for IdentBuf {
    fn as_ref(&self) -> &Ident {
        Ident::from_ident_str(&*self.s)
    }
}

impl AsRef<str> for IdentBuf {
    fn as_ref(&self) -> &str {
        &*self.s
    }
}

impl Borrow<Ident> for IdentBuf {
    fn borrow(&self) -> &Ident {
        Ident::from_ident_str(&*self.s)
    }
}

impl Borrow<str> for IdentBuf {
    fn borrow(&self) -> &str {
        &*self.s
    }
}

impl fmt::Debug for IdentBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.s, f)
    }
}

impl fmt::Display for IdentBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.s, f)
    }
}

impl PartialEq<Ident> for IdentBuf {
    fn eq(&self, other: &Ident) -> bool {
        *self.s == other.s
    }
}

impl PartialOrd<Ident> for IdentBuf {
    fn partial_cmp(&self, other: &Ident) -> Option<Ordering> {
        Some((*self.s).cmp(&other.s))
    }
}

impl serde::Serialize for IdentBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&*self.s, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for IdentBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const EXPECTED_IDENT: &'static str = "Expected unicode identifier";

        let s: String = serde::Deserialize::deserialize(deserializer)?;

        if s.is_empty() {
            return Err(serde::de::Error::invalid_length(1, &EXPECTED_IDENT));
        }

        let good_first = unicode_ident::is_xid_start;
        let bad_first = |c: char| !good_first(c);

        if s.starts_with(bad_first) {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(s.chars().next().unwrap()),
                &EXPECTED_IDENT,
            ));
        }

        let good = unicode_ident::is_xid_continue;
        let bad = |c: char| !good(c);

        match s.matches(bad).next() {
            None => Ok(IdentBuf { s }),
            Some(c) => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(c),
                &EXPECTED_IDENT,
            )),
        }
    }
}

/// Validates that string is a valid unicode identifier.
/// Returns error if it is not.
fn validate(s: &str) -> miette::Result<()> {
    if s.is_empty() {
        miette::bail!("Ident must not be empty");
    }

    let bad_first = |c: char| !unicode_ident::is_xid_start(c);

    if s.starts_with(bad_first) {
        miette::bail!("'{s}' is not valid Ident. First char must have XID_Start property");
    }

    let bad = |c: char| !unicode_ident::is_xid_continue(c);

    match s.matches(bad).next() {
        None => Ok(()),
        Some(c) => {
            miette::bail!(
                "'{s}' is not valid Ident. 2nd and later chars must have XID_Continue property. '{c}' doesn't"
            );
        }
    }
}
