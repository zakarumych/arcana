use std::fmt;

use crate::backend::SurfaceErrorKind;

#[derive(Debug)]
pub struct SurfaceError(pub(crate) SurfaceErrorKind);

impl fmt::Display for SurfaceError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for SurfaceError {}
