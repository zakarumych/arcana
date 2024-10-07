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

pub fn sha256_io(read: impl Read) -> io::Result<Hash256> {
    // let mut hasher = Sha256::new();
    // let mut scratch = [0; 1024];

    // read_in_blocks(read, &mut scratch, 64, false, |block| match block {
    //     ReadBlockItem::Block(data) | ReadBlockItem::Tail(data) => {
    //         hasher.update(data);
    //         Ok(())
    //     }
    //     ReadBlockItem::Error(err, _data) => Err(err),
    // })?;

    // let result = hasher.finalize();
    // Ok(Hash256(result.into()))
    todo!()
}

pub fn sha256_file(path: impl AsRef<Path>) -> io::Result<Hash256> {
    let file = std::fs::File::open(path)?;
    sha256_io(file)
}

pub fn sha512_io(reader: impl Read) -> io::Result<Hash512> {
    // let mut hasher = Sha512::new();
    // let mut scratch = [0; 1024];

    // read_in_blocks(reader, &mut scratch, 128, false, |block| match block {
    //     ReadBlockItem::Block(data) | ReadBlockItem::Tail(data) => {
    //         hasher.update(data);
    //         Ok(())
    //     }
    //     ReadBlockItem::Error(err, _data) => Err(err),
    // })?;

    // let result = hasher.finalize();
    // Ok(Hash512(result.into()))
    todo!()
}

pub fn sha512_file(path: impl AsRef<Path>) -> io::Result<Hash512> {
    let file = std::fs::File::open(path)?;
    sha512_io(file)
}
