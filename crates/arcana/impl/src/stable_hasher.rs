use std::hash::{Hash, Hasher};

use ahash::AHasher;

pub fn stable_hasher() -> AHasher {
    AHasher::new_with_keys(
        0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645,
        0x5851_F42D_4C95_7F2D_1405_7B7E_F767_814F,
    )
}

pub fn stable_hash<T>(value: &T) -> u64
where
    T: Hash + ?Sized,
{
    let mut hasher = stable_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}
