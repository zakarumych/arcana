use foldhash::fast::FixedState;
use hashbrown::{hash_map::RawEntryMut, HashMap};
use parking_lot::RwLock;

pub struct Interner {
    strings: RwLock<HashMap<&'static str, (), FixedState>>,
}

impl Interner {
    const fn new() -> Self {
        Interner {
            strings: RwLock::new(HashMap::with_hasher(FixedState::with_seed(0))),
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

    pub fn intern_static(&self, s: &'static str) -> &'static str {
        let strings = self.strings.read();
        if let Some((s, ())) = strings.get_key_value(s) {
            return *s;
        }
        drop(strings);
        self.intern_static_insert(s)
    }

    #[cold]
    #[inline(never)]
    fn intern_static_insert(&self, s: &'static str) -> &'static str {
        let mut strings = self.strings.write();

        match strings.raw_entry_mut().from_key(s) {
            RawEntryMut::Occupied(entry) => *entry.key(),
            RawEntryMut::Vacant(entry) => {
                entry.insert(s, ());
                s
            }
        }
    }

    pub fn intern_string(&self, s: String) -> &'static str {
        let strings = self.strings.read();
        if let Some((s, ())) = strings.get_key_value(&*s) {
            return *s;
        }
        drop(strings);
        self.intern_string_insert(s)
    }

    #[cold]
    #[inline(never)]
    fn intern_string_insert(&self, s: String) -> &'static str {
        let mut strings = self.strings.write();

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
