use std::hash::{BuildHasher, Hash, Hasher};

use ahash::AHasher;

pub fn color_hash<T>(value: &T) -> [u8; 3]
where
    T: Hash,
{
    let mut hasher = AHasher::new_with_keys(
        0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645,
        0x5851_F42D_4C95_7F2D_1405_7B7E_F767_814F,
    );
    value.hash(&mut hasher);
    let hash = hasher.finish();
    let r = (hash >> 16) as u8;
    let g = (hash >> 8) as u8;
    let b = hash as u8;
    [r, g, b]
}
