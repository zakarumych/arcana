use std::path::Path;

mod vscode;

/// IDE support for Arcana
pub trait Ide {
    type Error;

    /// Opens the given path in the IDE.
    fn open(&self, path: &Path, line: Option<u32>) -> Result<(), Self::Error>;
}
