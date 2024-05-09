use std::{
    hash::{BuildHasher, Hash, Hasher},
    io::Read,
};

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

/// Compute stable hash of data form reader.
/// Hashes produced by this function are stable across different runs and compilations of the program.
pub fn stable_hash_read<R: Read>(mut reader: R) -> std::io::Result<u64> {
    let mut hasher = stable_hasher();

    let mut buffer = [0; 1024];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.write(&buffer[..bytes_read]);
    }

    Ok(hasher.finish())
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGB format.
pub fn rgb_hash<T>(value: &T) -> [u8; 3]
where
    T: Hash + ?Sized,
{
    let hash = stable_hash(value);
    let r = ((hash >> 0) & 0xFF) as u8;
    let g = ((hash >> 8) & 0xFF) as u8;
    let b = ((hash >> 16) & 0xFF) as u8;
    [r, g, b]
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGBA format.
pub fn rgba_hash<T>(value: &T) -> [u8; 4]
where
    T: Hash + ?Sized,
{
    let hash = stable_hash(value);
    let r = ((hash >> 0) & 0xFF) as u8;
    let g = ((hash >> 8) & 0xFF) as u8;
    let b = ((hash >> 16) & 0xFF) as u8;
    let a = ((hash >> 24) & 0xFF) as u8;
    [r, g, b, a]
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGBA format with premultiplied alpha.
pub fn rgba_premultiplied_hash<T>(value: &T) -> [u8; 4]
where
    T: Hash + ?Sized,
{
    let hash = stable_hash(value);
    let a = (hash >> 24) & 0xFF;
    let r = (((hash >> 0) & 0xFF) * a / 255) as u8;
    let g = (((hash >> 8) & 0xFF) * a / 255) as u8;
    let b = (((hash >> 16) & 0xFF) * a / 255) as u8;
    [r, g, b, a as u8]
}

/// Computes a color based on the hash of the value.
/// Returns a color in RGB format.
/// Resulting color will always have 100% saturation and 100% value in HSV space.
pub fn hue_hash<T>(value: &T) -> [u8; 3]
where
    T: Hash + ?Sized,
{
    let hash = stable_hash(value);
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
