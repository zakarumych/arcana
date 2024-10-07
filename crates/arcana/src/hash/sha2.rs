use std::{
    io::{self, Read},
    path::Path,
};

use sha2::{Digest, Sha256, Sha512};

use crate::io::BufferRead;

use super::{Hash256, Hash512};

pub fn sha256(data: &[u8]) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    Hash256::from_u8(result.into())
}

pub fn sha512(data: &[u8]) -> Hash512 {
    let mut hasher = Sha512::new();
    hasher.update(data);
    let result = hasher.finalize();
    Hash512::from_u8(result.into())
}

pub fn sha256_io(read: impl Read) -> io::Result<Hash256> {
    let mut hasher = Sha256::new();
    let mut scratch = [0u8; 1024];

    let mut buffer_read = BufferRead::new_borrowed(&mut scratch, read);

    loop {
        let bytes = buffer_read.fill_buf(1)?;
        if bytes.is_empty() {
            break;
        }
        hasher.update(bytes);
        let len = bytes.len();
        buffer_read.consume(len);
    }

    let result = hasher.finalize();
    Ok(Hash256::from_u8(result.into()))
}

pub fn sha256_file(path: impl AsRef<Path>) -> io::Result<Hash256> {
    let file = std::fs::File::open(path)?;
    sha256_io(file)
}

pub fn sha512_io(read: impl Read) -> io::Result<Hash512> {
    let mut hasher = Sha512::new();
    let mut scratch = [0u8; 1024];

    let mut buffer_read = BufferRead::new_borrowed(&mut scratch, read);

    loop {
        let bytes = buffer_read.fill_buf(1)?;
        if bytes.is_empty() {
            break;
        }
        hasher.update(bytes);
        let len = bytes.len();
        buffer_read.consume(len);
    }

    let result = hasher.finalize();
    Ok(Hash512::from_u8(result.into()))
}

pub fn sha512_file(path: impl AsRef<Path>) -> io::Result<Hash512> {
    let file = std::fs::File::open(path)?;
    sha512_io(file)
}
