use std::hash::BuildHasher;

use hashbrown::{hash_map::RawEntryMut, HashMap};
use parking_lot::RwLock;

struct DefaultAHasherBuilder;

impl BuildHasher for DefaultAHasherBuilder {
    type Hasher = ahash::AHasher;

    fn build_hasher(&self) -> Self::Hasher {
        ahash::AHasher::default()
    }
}

pub struct Interner {
    strings: RwLock<HashMap<&'static str, (), DefaultAHasherBuilder>>,
}

impl Interner {
    const fn new() -> Self {
        Interner {
            strings: RwLock::new(HashMap::with_hasher(DefaultAHasherBuilder)),
        }
    }

    pub fn intern(&self, s: &str) -> &'static str {
        let strings = self.strings.read();
        if let Some((s, ())) = strings.get_key_value(s) {
            return *s;
        }
        drop(strings);
        self.intern_insert(s)
    }

    #[cold]
    #[inline(never)]
    fn intern_insert(&self, s: &str) -> &'static str {
        let mut strings = self.strings.write();

        match strings.raw_entry_mut().from_key(s) {
            RawEntryMut::Occupied(entry) => *entry.key(),
            RawEntryMut::Vacant(entry) => {
                let s = s.to_owned().leak();
                entry.insert(&*s, ());
                s
            }
        }
    }
}

pub static INTERNER: Interner = Interner::new();
