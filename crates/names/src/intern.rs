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
    strings: RwLock<HashMap<String, (), DefaultAHasherBuilder>>,
}

impl Interner {
    const fn new() -> Self {
        Interner {
            strings: RwLock::new(HashMap::with_hasher(DefaultAHasherBuilder)),
        }
    }

    pub fn intern(&self, s: &str) -> &str {
        let strings = self.strings.read();
        if let Some((s, ())) = strings.get_key_value(s) {
            let s = unsafe { Self::interned_string_never_evicted(s) };
            return s;
        }
        drop(strings);
        self.intern_insert(s)
    }

    #[cold]
    #[inline(never)]
    fn intern_insert(&self, s: &str) -> &str {
        let mut strings = self.strings.write();

        match strings.raw_entry_mut().from_key(s) {
            RawEntryMut::Occupied(entry) => {
                let s = unsafe { Self::interned_string_never_evicted(entry.key()) };
                s
            }
            RawEntryMut::Vacant(entry) => {
                let s = s.to_owned();
                let (s, ()) = entry.insert(s.to_owned(), ());
                let s = unsafe { Self::interned_string_never_evicted(s) };
                s
            }
        }
    }

    unsafe fn interned_string_never_evicted<'a, 'b>(s: &'a str) -> &'b str {
        // Safety:
        // This is safe as long as `Interner` promises to never evict strings.
        unsafe { &*(s as *const str) }
    }
}

pub static INTERNER: Interner = Interner::new();
