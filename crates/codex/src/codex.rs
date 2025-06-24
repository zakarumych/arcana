use arcana_id::{make_uid, Stid};
use arcana_intern::Name;
use arcana_tany::TAny;
use hashbrown::{hash_map::Entry, HashMap};

use crate::{
    flow::{FlowCodexFn, FlowNodeDesc},
    pure::{PureCodexFn, PureNodeDesc},
};

make_uid! {
    /// ID of the code graph
    pub CodexId;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputId {
    pub node: usize,
    pub idx: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId {
    pub node: usize,
    pub idx: usize,
}

#[derive(Default)]
pub(crate) struct CodexValues {
    values: HashMap<ValueId, TAny>,
}

impl CodexValues {
    pub fn new() -> Self {
        CodexValues {
            values: HashMap::new(),
        }
    }

    pub(crate) fn has(&self, id: ValueId) -> bool {
        self.values.contains_key(&id)
    }

    pub fn get<T: 'static>(&self, id: ValueId) -> Option<&T> {
        self.values.get(&id).and_then(|s| s.downcast_ref())
    }

    pub fn set<T>(&mut self, id: ValueId, value: T)
    where
        T: Send + Sync + 'static,
    {
        match self.values.entry(id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set(value);
            }
            Entry::Vacant(entry) => {
                entry.insert(TAny::new(value));
            }
        }
    }

    pub fn clone_from<T>(&mut self, id: ValueId, value: &T)
    where
        T: Clone + Send + Sync + 'static,
    {
        match self.values.entry(id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().clone_from(value);
            }
            Entry::Vacant(entry) => {
                entry.insert(TAny::new(value.clone()));
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CodexNodeSignature {
    /// Pure codex node gets executed every type its output is required.
    Pure(PureNodeDesc),

    /// Flow codex node that gets executed when triggered by connected inflow.
    Flow(FlowNodeDesc),
}

/// Codex descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CodexNodeDesc {
    pub(crate) name: Name,
    pub(crate) signature: CodexNodeSignature,
}

pub(crate) enum CodexNode {
    Pure {
        desc: PureNodeDesc,
        inputs: Vec<ValueId>,
        fun: PureCodexFn,
    },
    Flow {
        desc: FlowNodeDesc,
        inputs: Vec<ValueId>,
        follows: Vec<InputId>,
        fun: FlowCodexFn,
    },
}

/// Component that represent codex associated with an entity.
pub struct Codex {
    id: CodexId,
    pub(crate) nodes: Vec<CodexNode>,
}

impl Codex {
    pub fn id(&self) -> CodexId {
        self.id
    }
}
