use crate::assets::AssetId;

/// Single dependency for a asset.
#[derive(Debug)]
pub struct AssetDependency {
    /// Source path.
    pub source: String,

    /// Target format.
    pub target: String,
}

/// Provides access to asset dependencies.
/// Converts source and target to asset id.
pub trait AssetDependencies {
    /// Returns dependency id.
    /// If dependency is not available, returns `None`.
    fn get(&mut self, source: &str, target: &str) -> Option<AssetId>;

    /// Returns dependency id.
    /// If dependency is not available,
    /// append it to the missing list and returns `None`.
    fn get_or_append(
        &mut self,
        source: &str,
        target: &str,
        missing: &mut Vec<AssetDependency>,
    ) -> Option<AssetId> {
        match self.get(source, target) {
            None => {
                missing.push(AssetDependency {
                    source: source.to_owned(),
                    target: target.to_owned(),
                });
                None
            }
            Some(id) => Some(id),
        }
    }
}

impl<D: ?Sized> AssetDependencies for &mut D
where
    D: AssetDependencies,
{
    fn get(&mut self, source: &str, target: &str) -> Option<AssetId> {
        (*self).get(source, target)
    }
}
