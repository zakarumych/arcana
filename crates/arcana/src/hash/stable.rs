use std::{
    hash::{BuildHasher, Hash, Hasher},
    io::Read,
    path::Path,
};

use ahash::{AHasher, RandomState};
use hashbrown::HashMap;

use super::Hash64;

/// Builder for stable hasher.
#[derive(Default)]
pub struct StableHashBuilder;

impl BuildHasher for StableHashBuilder {
    type Hasher = AHasher;

    fn build_hasher(&self) -> Self::Hasher {
        stable_hasher()
    }
}

pub type StableHashMap<K, V> = HashMap<K, V, StableHashBuilder>;

pub const fn stable_hash_map<K, V>() -> StableHashMap<K, V> {
    HashMap::with_hasher(StableHashBuilder)
}

/// Return stable hasher instance.
/// Hashes produced by this hasher are stable across different runs and compilations of the program.
pub fn stable_hasher() -> AHasher {
    RandomState::with_seeds(1, 2, 3, 4).build_hasher()
}

/// Computes stable hash for the value.
/// Hashes produced by this function are stable across different runs and compilations of the program.
pub fn stable_hash<T>(value: &T) -> Hash64
where
    T: Hash + ?Sized,
{
    let mut hasher = stable_hasher();
    value.hash(&mut hasher);
    Hash64::from_u8(hasher.finish().to_ne_bytes())
}

/// Compute stable hash of data form reader.
/// Hashes produced by this function are stable across different runs and compilations of the program.
pub fn stable_hash_read<R: Read>(mut reader: R) -> std::io::Result<Hash64> {
    let mut hasher = stable_hasher();

    let mut buffer = [0; 1024];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.write(&buffer[..bytes_read]);
    }

    Ok(Hash64::from_u8(hasher.finish().to_ne_bytes()))
}

/// Compute stable hash of file content.
pub fn stable_hash_file(path: impl AsRef<Path>) -> std::io::Result<Hash64> {
    let file = std::fs::File::open(path)?;
    stable_hash_read(file)
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGB format.
pub fn rgb_hash<T>(value: &T) -> [u8; 3]
where
    T: Hash + ?Sized,
{
    let [r, g, b, ..] = *stable_hash(value).as_u8();
    [r, g, b]
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGBA format.
pub fn rgba_hash<T>(value: &T) -> [u8; 4]
where
    T: Hash + ?Sized,
{
    let [r, g, b, a, ..] = *stable_hash(value).as_u8();
    [r, g, b, a]
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGBA format with premultiplied alpha.
pub fn rgba_premultiplied_hash<T>(value: &T) -> [u8; 4]
where
    T: Hash + ?Sized,
{
    let [r, g, b, a, ..] = *stable_hash(value).as_u8();
    let r = (r * a / 255) as u8;
    let g = (g * a / 255) as u8;
    let b = (b * a / 255) as u8;
    [r, g, b, a]
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGB format.
/// Resulting color will always have 100% saturation and 100% value in HSV space.
pub fn hue_hash<T>(value: &T) -> [u8; 3]
where
    T: Hash + ?Sized,
{
    let [hash] = *stable_hash(value).as_u64();
    let zone = (hash & 0xFFFFFFFF) % 6;
    let mag = (hash >> 32) as u8;
    match zone {
        0 => [255, mag, 0],
        1 => [mag, 255, 0],
        2 => [0, 255, mag],
        3 => [0, mag, 255],
        4 => [mag, 0, 255],
        _ => [255, 0, mag],
    }
}

/// Mixes string value into the hash.
/// Resulting hash is only as good as the source hash.
#[doc(hidden)]
pub const fn mix_hash_with_string(mut hash: u64, s: &str) -> u64 {
    hash = hash.wrapping_mul(0x9E3779B97F4A7C15);
    let mut bytes = s.as_bytes();

    while let Some((first, rest)) = bytes.split_first() {
        hash = hash.wrapping_add(*first as u64);
        hash = hash.wrapping_mul(0x9E3779B97F4A7C15);
        bytes = rest;
    }

    hash
}
