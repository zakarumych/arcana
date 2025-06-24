use std::path::PathBuf;

/// Provides access to source files.
/// Converts source URL to local path.
///
/// If URL is file:// it is converted to absolute path.
/// If URL is data:// the temporary file is created.
/// If URL is http:// or https:// the file is downloaded asynchronously.
/// Other URL schemas are not supported yet.
pub trait AssetSources {
    /// Returns path to the source.
    /// If source is not available, returns `None`.
    fn get(&mut self, source: &str) -> Option<PathBuf>;

    /// Returns path to the source.
    /// If source is not available,
    /// append it to the missing list and returns `None`.
    fn get_or_append(&mut self, source: &str, missing: &mut Vec<String>) -> Option<PathBuf> {
        match self.get(source) {
            None => {
                missing.push(source.to_owned());
                None
            }
            Some(path) => Some(path),
        }
    }
}

impl<S: ?Sized> AssetSources for &mut S
where
    S: AssetSources,
{
    fn get(&mut self, source: &str) -> Option<PathBuf> {
        (*self).get(source)
    }
}
