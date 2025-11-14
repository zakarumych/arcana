use std::hash::BuildHasher;

use foldhash::fast::FixedState;
use hashbrown::{hash_map::RawEntryMut, HashMap};
use parking_lot::RwLock;

const SHARD_COUNT: usize = 17;
const SHARD_COUNT64: u64 = 17;

fn idx(s: &str) -> usize {
    let hasher = FixedState::with_seed(0);
    let h = hasher.hash_one(s);
    (h % SHARD_COUNT64) as usize
}

pub struct Interner {
    strings: [RwLock<HashMap<&'static str, (), FixedState>>; SHARD_COUNT],
}

impl Interner {
    const fn new() -> Self {
        Interner {
            strings: [const { RwLock::new(HashMap::with_hasher(FixedState::with_seed(0))) };
                SHARD_COUNT],
        }
    }

    pub fn intern(&self, s: &str) -> &'static str {
        let idx = idx(s);

        let strings = self.strings[idx].read();
        if let Some((s, ())) = strings.get_key_value(s) {
            return *s;
        }
        drop(strings);
        self.intern_insert(s, idx)
    }

    pub fn intern_static(&self, s: &'static str) -> &'static str {
        let idx = idx(s);

        let strings = self.strings[idx].read();
        if let Some((s, ())) = strings.get_key_value(s) {
            return *s;
        }
        drop(strings);
        self.intern_static_insert(s, idx)
    }

    pub fn intern_string(&self, s: String) -> &'static str {
        let idx = idx(&*s);

        let strings = self.strings[idx].read();
        if let Some((s, ())) = strings.get_key_value(&*s) {
            return *s;
        }
        drop(strings);
        self.intern_string_insert(s, idx)
    }

    #[cold]
    #[inline(never)]
    fn intern_insert(&self, s: &str, idx: usize) -> &'static str {
        let mut strings = self.strings[idx].write();

        match strings.raw_entry_mut().from_key(s) {
            RawEntryMut::Occupied(entry) => *entry.key(),
            RawEntryMut::Vacant(entry) => {
                let s = s.to_owned().leak();
                entry.insert(&*s, ());
                s
            }
        }
    }

    #[cold]
    #[inline(never)]
    fn intern_static_insert(&self, s: &'static str, idx: usize) -> &'static str {
        let mut strings = self.strings[idx].write();

        match strings.raw_entry_mut().from_key(s) {
            RawEntryMut::Occupied(entry) => *entry.key(),
            RawEntryMut::Vacant(entry) => {
                entry.insert(s, ());
                s
            }
        }
    }

    #[cold]
    #[inline(never)]
    fn intern_string_insert(&self, s: String, idx: usize) -> &'static str {
        let mut strings = self.strings[idx].write();

        match strings.raw_entry_mut().from_key(&*s) {
            RawEntryMut::Occupied(entry) => *entry.key(),
            RawEntryMut::Vacant(entry) => {
                let s = s.leak();
                entry.insert(&*s, ());
                s
            }
        }
    }
}

pub static INTERNER: Interner = Interner::new();
