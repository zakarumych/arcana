use std::{future::Future, task::Poll};

use arcana_id::Stid;
use arcana_intern::Name;
use edict::{entity::EntityId, flow::FlowEntity, world::World};

use crate::{
    codex::{Codex, CodexId, CodexValues, ValueId},
    component::CodexComponent,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FlowNodeDesc {
    pub inflows: Vec<Name>,
    pub outflows: Vec<Name>,
    pub inputs: Vec<(Name, Stid)>,
    pub outputs: Vec<(Name, Stid)>,
}

/// Context for flow codex execution.
pub struct FlowContext<'a> {
    values: &'a mut CodexValues,
    node: usize,
    outflow: &'a mut Option<usize>,
}

impl<'a> FlowContext<'a> {
    pub(crate) fn new(
        node: usize,
        values: &'a mut CodexValues,
        outflow: &'a mut Option<usize>,
    ) -> Self {
        FlowContext {
            node,
            values,
            outflow,
        }
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

    /// Sync flow codex must call this method to specify which flow output will be triggered.
    pub fn ready(self, outflow: usize) {
        *self.outflow = Some(outflow);
    }

    /// Async flow codex must call this method to specify future that will be awaited
    /// before proceeding with execution.
    pub fn delay<Fut>(self, entity: FlowEntity, fut: Fut)
    where
        Fut: Future<Output = usize> + Send + 'static,
    {
        let node = self.node;
        let values = core::mem::take(self.values);

        entity.spawn_flow(move |entity: FlowEntity| async move {
            let outflow = fut.await;

            entity.map(|mut e| {
                if let Some(codex) = e.get_mut::<&mut CodexComponent>() {
                    codex.push_continuation(node, outflow, values);
                }
            });
        });
    }
}

/// Type of code function.
/// It takes list of inputs and outputs to produce.
/// It also takes index of input flow that triggered execution.
/// It returns output flow index to trigger next flow function.
pub type FlowCodexFn = fn(
    inflow: usize,
    entity: FlowEntity,
    inputs: &[ValueId],
    outputs: &[ValueId],
    ctx: FlowContext,
);

fn enqueue_async_continue(
    entity: EntityId,
    codes: CodexId,
    node: usize,
    outflow: usize,
    values: CodexValues,
    world: &World,
) {
}
