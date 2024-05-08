use std::{
    hash::{BuildHasher, Hash, Hasher},
    io::Read,
};

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

/// Compute stable hash of data form reader.
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
