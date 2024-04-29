use std::hash::{BuildHasher, Hash, Hasher};

use ahash::{AHasher, RandomState};

pub fn stable_hasher() -> AHasher {
    RandomState::with_seeds(1, 2, 3, 4).build_hasher()
}

pub fn stable_hash<T>(value: &T) -> u64
where
    T: Hash + ?Sized,
{
    let mut hasher = stable_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}
