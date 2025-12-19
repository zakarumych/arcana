use arcana_id::Stid;
use arcana_intern::Name;
use arcana_model::TypeModel;
use edict::flow::FlowEntity;

use crate::codex::{CodexValues, NodeInput, OutputId};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PureNodeDesc {
    pub inputs: Vec<(Name, Stid)>,
    pub outputs: Vec<(Name, Stid)>,
}

pub struct PureContext<'a> {
    node: usize,
    inputs: &'a [NodeInput],
    values: &'a mut CodexValues,
}

impl<'a> PureContext<'a> {
    pub(crate) fn new(node: usize, inputs: &'a [NodeInput], values: &'a mut CodexValues) -> Self {
        PureContext {
            node,
            inputs,
            values,
        }
    }

    pub fn get<T>(&self, idx: usize) -> Option<T>
    where
        T: TypeModel,
    {
        match self.inputs[idx] {
            NodeInput::Connected(id) => self.values.get(id).cloned(),
            NodeInput::Specified(ref value) => T::try_clone_from_value(value),
            NodeInput::Unspecified => None,
        }
    }

    pub fn get_opaque<T>(&self, idx: usize) -> Option<&T>
    where
        T: 'static,
    {
        match self.inputs[idx] {
            NodeInput::Connected(id) => self.values.get(id),
            NodeInput::Specified(_) => None,
            NodeInput::Unspecified => None,
        }
    }

    pub fn set<T>(&mut self, idx: usize, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.values.set(
            OutputId {
                node: self.node,
                idx,
            },
            value,
        );
    }
}

/// Type of pure codex function.
/// Generally it should not have any visible side effects.
/// Its execution may occur at any point or not occur at all.
pub type PureCodexFn = fn(entity: FlowEntity, ctx: PureContext);
