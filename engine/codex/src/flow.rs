use std::future::Future;

use arcana_id::Stid;
use arcana_intern::Name;
use arcana_model::TypeModel;
use edict::flow::FlowEntity;

use crate::{
    codex::{CodexValues, NodeInput, OutputId},
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
    node: usize,
    inflow: usize,
    inputs: &'a [NodeInput],
    values: &'a mut CodexValues,
    outflow: &'a mut Option<usize>,
}

impl<'a> FlowContext<'a> {
    pub(crate) fn new(
        node: usize,
        inflow: usize,
        inputs: &'a [NodeInput],
        values: &'a mut CodexValues,
        outflow: &'a mut Option<usize>,
    ) -> Self {
        FlowContext {
            node,
            inflow,
            inputs,
            values,
            outflow,
        }
    }

    pub fn inflow(&self) -> usize {
        self.inflow
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
                    codex.push_trigger(node, outflow, values);
                }
            });
        });
    }
}

/// Type of flow codex function.
/// Unlike PureCodexFn it is expected to have visible side effects.
/// Context provides not only input values, but also inflow index
/// and allows specifying outflow index both synchronously and asynchronously.
pub type FlowCodexFn = fn(entity: FlowEntity, ctx: FlowContext);
