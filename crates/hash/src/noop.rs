use std::hash::{BuildHasher, Hasher};

use hashbrown::HashMap;

#[derive(Default)]
pub struct NoHashBuilder;

impl BuildHasher for NoHashBuilder {
    type Hasher = NoHasher;

    fn build_hasher(&self) -> Self::Hasher {
        NoHasher(0)
    }
}

pub struct NoHasher(u64);

impl Hasher for NoHasher {
    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!()
    }

    fn write_u128(&mut self, i: u128) {
        self.0 = i as u64;
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }

    fn write_u32(&mut self, i: u32) {
        self.0 = i as u64;
    }

    fn write_u16(&mut self, i: u16) {
        self.0 = i as u64;
    }

    fn write_u8(&mut self, i: u8) {
        self.0 = i as u64;
    }

    fn write_usize(&mut self, i: usize) {
        self.0 = i as u64;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub type NoHashMap<K, V> = HashMap<K, V, NoHashBuilder>;

pub const fn no_hash_map<K, V>() -> NoHashMap<K, V> {
    HashMap::with_hasher(NoHashBuilder)
}
