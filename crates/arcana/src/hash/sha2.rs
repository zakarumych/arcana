use std::{
    io::{self, Read},
    path::Path,
};

use sha2::{Digest, Sha256, Sha512};

use super::{Hash256, Hash512};

pub fn sha256(data: &[u8]) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    Hash256(result.into())
}

pub fn sha512(data: &[u8]) -> Hash512 {
    let mut hasher = Sha512::new();
    hasher.update(data);
    let result = hasher.finalize();
    Hash512(result.into())
}

pub fn sha256_io(mut reader: impl Read) -> io::Result<Hash256> {
    let mut hasher = Sha256::new();

    let mut sratch = [0; 1024];
    let mut cursor = 0;
    loop {
        let bytes_read = match reader.read(&mut sratch[cursor..]) {
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if bytes_read == 0 {
            break;
        }

        cursor += bytes_read;

        let rem = cursor & 0x3f;
        let blocks = cursor - rem;

        if blocks > 0 {
            hasher.update(&sratch[..blocks]);

            let (head, tail) = sratch.split_at_mut(blocks);
            head[..rem].copy_from_slice(&tail[..rem]);

            cursor = rem;
        }
    }

    if cursor > 0 {
        hasher.update(&sratch[..cursor]);
    }

    let result = hasher.finalize();
    Ok(Hash256(result.into()))
}

pub fn sha256_file(path: impl AsRef<Path>) -> io::Result<Hash256> {
    let file = std::fs::File::open(path)?;
    sha256_io(file)
}

pub fn sha512_io(mut reader: impl Read) -> io::Result<Hash512> {
    let mut hasher = Sha512::new();

    let mut sratch = [0; 1024];
    let mut cursor = 0;
    loop {
        let bytes_read = match reader.read(&mut sratch[cursor..]) {
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if bytes_read == 0 {
            break;
        }

        cursor += bytes_read;

        let rem = cursor & 0x7f;
        let blocks = cursor - rem;

        if blocks > 0 {
            hasher.update(&sratch[..blocks]);

            let (head, tail) = sratch.split_at_mut(blocks);
            head[..rem].copy_from_slice(&tail[..rem]);

            cursor = rem;
        }
    }

    if cursor > 0 {
        hasher.update(&sratch[..cursor]);
    }

    let result = hasher.finalize();
    Ok(Hash512(result.into()))
}

pub fn sha512_file(path: impl AsRef<Path>) -> io::Result<Hash512> {
    let file = std::fs::File::open(path)?;
    sha512_io(file)
}
