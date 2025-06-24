use arcana_id::Stid;
use arcana_intern::Name;
use edict::flow::FlowEntity;

use crate::codex::{CodexValues, ValueId};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PureNodeDesc {
    pub inputs: Vec<(Name, Stid)>,
    pub outputs: Vec<(Name, Stid)>,
}

pub struct PureContext<'a> {
    values: &'a mut CodexValues,
}

impl<'a> PureContext<'a> {
    pub(crate) fn new(values: &'a mut CodexValues) -> Self {
        PureContext { values }
    }

    pub fn get<T: 'static>(&self, id: ValueId) -> Option<&T> {
        self.values.get(id)
    }

    pub fn set<T>(&mut self, id: ValueId, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.values.set(id, value);
    }

    pub fn clone_from<T>(&mut self, id: ValueId, value: &T)
    where
        T: Clone + Send + Sync + 'static,
    {
        self.values.clone_from(id, value);
    }
}

/// Type of pure code function.
/// It takes list of inputs and outputs to produce.
/// Generally it should not have any visible side effects.
/// Its execution may occur at any point or not occur at all.
pub type PureCodexFn =
    fn(entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], ctx: PureContext);
