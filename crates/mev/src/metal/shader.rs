use std::{fmt, sync::Arc};

use hashbrown::HashMap;

use crate::generic::ShaderCompileError;

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub struct GroupBindings {
    pub bindings: [u8; 64],
}

impl GroupBindings {
    const INVALID: Self = GroupBindings {
        bindings: [0xff; 64],
    };
}

pub struct Bindings {
    pub groups: [GroupBindings; 16],
    pub push_constants: Option<u8>,
}

impl Bindings {
    pub fn new() -> Self {
        Bindings {
            groups: [GroupBindings::INVALID; 16],
            push_constants: None,
        }
    }

    pub fn insert(&mut self, binding: naga::ResourceBinding, slot: u8) {
        self.groups[binding.group as usize].bindings[binding.binding as usize] = slot;
    }

    pub fn set_push_constants(&mut self, slot: u8) {
        self.push_constants = Some(slot);
    }
}

#[derive(Clone)]
pub struct Library {
    library: metal::Library,
    per_entry_point_bindings: HashMap<String, Arc<Bindings>>,
}

impl Library {
    pub(super) fn new(library: metal::Library) -> Self {
        Library {
            library,
            per_entry_point_bindings: HashMap::new(),
        }
    }

    pub(super) fn with_per_entry_point_bindings(
        library: metal::Library,
        per_entry_point_bindings: HashMap<String, Arc<Bindings>>,
    ) -> Self {
        Library {
            library,
            per_entry_point_bindings,
        }
    }

    pub(super) fn get_function(&self, entry: &str) -> Option<metal::Function> {
        self.library.get_function(entry, None).ok()
    }

    pub(super) fn get_bindings(&self, entry: &str) -> Option<Arc<Bindings>> {
        self.per_entry_point_bindings.get(entry).cloned()
    }
}

#[hidden_trait::expose]
impl crate::traits::Library for Library {
    fn entry<'a>(&self, entry: &'a str) -> Shader<'a> {
        Shader {
            library: self.clone(),
            entry: Cow::Borrowed(entry),
        }
    }
}
