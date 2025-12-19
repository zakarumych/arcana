use arcana_id::make_uid;
use arcana_intern::Name;
use arcana_model::Value;
use arcana_tany::TAny;
use hashbrown::{hash_map::Entry, HashMap};

use crate::{
    event::EventNodeDesc,
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
pub struct OutputId {
    pub node: usize,
    pub idx: usize,
}
pub(crate) enum NodeInput {
    Connected(OutputId),
    Specified(Value),
    Unspecified,
}

#[derive(Default)]
pub(crate) struct CodexValues {
    values: HashMap<OutputId, TAny>,
}

impl CodexValues {
    pub(crate) fn from_iter(iter: impl Iterator<Item = (OutputId, TAny)>) -> Self {
        CodexValues {
            values: iter.collect(),
        }
    }

    pub(crate) fn has(&self, id: OutputId) -> bool {
        self.values.contains_key(&id)
    }

    pub fn get<T: 'static>(&self, id: OutputId) -> Option<&T> {
        self.values.get(&id).and_then(|s| s.downcast_ref())
    }

    pub fn set<T>(&mut self, id: OutputId, value: T)
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

    pub fn clone_from<T>(&mut self, id: OutputId, value: &T)
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
        inputs: Vec<NodeInput>,
        fun: PureCodexFn,
    },
    Flow {
        desc: FlowNodeDesc,
        inputs: Vec<NodeInput>,
        follows: Vec<Option<InputId>>,
        fun: FlowCodexFn,
    },
    Event {
        desc: EventNodeDesc,
        follow: Option<InputId>,
    },
}

/// Component that represent codex associated with an entity.
pub struct Codex {
    pub(crate) nodes: Vec<CodexNode>,
}

impl Codex {
    pub fn new() -> Self {
        Codex { nodes: Vec::new() }
    }

    pub fn add_pure(&mut self, desc: PureNodeDesc, fun: PureCodexFn) -> usize {
        let node = CodexNode::Pure {
            desc: desc.clone(),
            inputs: Vec::new(),
            fun,
        };
        self.nodes.push(node);
        self.nodes.len() - 1
    }
}
