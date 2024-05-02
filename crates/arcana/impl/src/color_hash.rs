use std::hash::Hash;

use crate::stable_hash;

pub fn color_hash<T>(value: &T) -> [u8; 3]
where
    T: Hash + ?Sized,
{
    let hash = stable_hash(value);
    let r = (hash >> 16) as u8;
    let g = (hash >> 8) as u8;
    let b = hash as u8;
    [r, g, b]
}
