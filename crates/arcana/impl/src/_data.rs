//! This module provides runtime-type information functionality for Arcana engine.
//! Runtime-typing is used extensively during development, and may be used outside hotpath in production.
//!
//! Arcana provides data-driver approach to runtime-typing with code-gen support.
//! User registers types manually, providing any information that may be required,
//! almost all of which is optional where it is assigned to DataId.
//!

use std::{
    any::TypeId,
    mem::{align_of, size_of},
    num::NonZeroU64,
    sync::Arc,
};

use hashbrown::{hash_map::Entry, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DataId {
    id: NonZeroU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DataField {
    offset: usize,
    id: DataId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DataLayout {
    size: usize,
    align: usize,
    fields: Arc<[DataField]>,
}

struct Data {
    /// Data name.
    name: Arc<str>,

    /// Data layout in memory.
    layout: Option<DataLayout>,
}

pub struct Registry {
    /// Collection of registered data.
    data: HashMap<DataId, Data>,

    /// Auxiliary table to find data by layout.
    layout_lookup: HashMap<DataLayout, Vec<DataId>>,

    /// Auxiliary table to find data by type id.
    type_lookup: HashMap<TypeId, DataId>,

    next_id: u64,
}

impl Registry {
    pub fn register(
        &mut self,
        type_id: TypeId,
        name: impl Into<Arc<str>>,
        mut layout: Option<DataLayout>,
    ) -> DataId {
        match self.type_lookup.entry(type_id) {
            Entry::Occupied(entry) => {
                let id = *entry.get();

                let data = self
                    .data
                    .get_mut(&id)
                    .expect("Type id not found in data table");

                assert_eq!(data.name, name.into(), "Type id name mismatch");

                match (&data.layout, layout) {
                    (Some(old), Some(new)) => {
                        assert_eq!(*old, new, "Type id layout mismatch");
                    }
                    (Some(_), None) => {}
                    (None, Some(mut new)) => {
                        match self.layout_lookup.entry(new) {
                            Entry::Occupied(mut entry) => {
                                // Replace with one in the table to reduce copies.
                                new = entry.key().clone();
                                entry.get_mut().push(id);
                            }
                            Entry::Vacant(entry) => {
                                new = entry.key().clone();
                                entry.insert(vec![id]);
                            }
                        }
                        data.layout = Some(new);
                    }
                    (None, None) => {}
                }

                id
            }
            Entry::Vacant(entry) => {
                assert_ne!(self.next_id, 0, "data id overflow");

                let id = DataId {
                    id: unsafe { NonZeroU64::new_unchecked(self.next_id) },
                };

                self.next_id += 1;

                if let Some(l) = layout {
                    match self.layout_lookup.entry(l) {
                        Entry::Occupied(mut entry) => {
                            // Replace with one in the table to reduce copies.
                            layout = Some(entry.key().clone());
                            entry.get_mut().push(id);
                        }
                        Entry::Vacant(entry) => {
                            layout = Some(entry.key().clone());
                            entry.insert(vec![id]);
                        }
                    }
                }

                let data = Data {
                    name: name.into(),
                    layout,
                };

                let old = self.data.insert(id, data);
                debug_assert!(old.is_none());

                entry.insert(id);
                id
            }
        }
    }

    pub fn register_primitive<T: 'static>(&mut self, name: &'static str) {
        let type_id = TypeId::of::<T>();
        let layout = DataLayout {
            size: size_of::<T>(),
            align: align_of::<T>(),
            fields: Arc::new([]),
        };
        self.register(type_id, name, Some(layout));
    }

    pub fn new_empty() -> Self {
        Registry {
            data: HashMap::new(),
            layout_lookup: HashMap::new(),
            type_lookup: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn new() -> Self {
        let mut registry = Self::new_empty();

        registry.register_primitive::<u8>("u8");
        registry.register_primitive::<u16>("u16");
        registry.register_primitive::<u32>("u32");
        registry.register_primitive::<u64>("u64");
        registry.register_primitive::<u128>("u128");
        registry.register_primitive::<usize>("usize");

        registry.register_primitive::<i8>("i8");
        registry.register_primitive::<i16>("i16");
        registry.register_primitive::<i32>("i32");
        registry.register_primitive::<i64>("i64");
        registry.register_primitive::<i128>("i128");
        registry.register_primitive::<isize>("isize");

        registry
    }
}
