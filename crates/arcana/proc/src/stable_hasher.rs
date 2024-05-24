use std::hash::{BuildHasher, Hash, Hasher};

use ahash::{AHasher, RandomState};

/// Return stable hasher instance.
/// Hashes produced by this hasher are stable across different runs and compilations of the program.
pub fn stable_hasher() -> AHasher {
    RandomState::with_seeds(1, 2, 3, 4).build_hasher()
}

/// Computes stable hash for the value.
/// Hashes produced by this function are stable across different runs and compilations of the program.
pub fn stable_hash<T>(value: &T) -> u64
where
    T: Hash + ?Sized,
{
    let mut hasher = stable_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}
