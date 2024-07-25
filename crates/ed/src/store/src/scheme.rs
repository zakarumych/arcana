use std::str::FromStr;

/// URL schemas supported by the store.
/// Matches should use this enum instead of matching on strings.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Scheme {
    File,
    Data,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct UnsupportedScheme;

impl FromStr for Scheme {
    type Err = UnsupportedScheme;

    #[inline]
    fn from_str(s: &str) -> Result<Self, UnsupportedScheme> {
        match s {
            "file" => Ok(Scheme::File),
            "data" => Ok(Scheme::Data),
            _ => Err(UnsupportedScheme),
        }
    }
}
