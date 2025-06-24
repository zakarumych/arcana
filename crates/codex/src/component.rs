use edict::{component::Component, entity::EntityId, flow::FlowEntity};
use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

use crate::{
    codex::{Codex, CodexId, CodexNode, CodexValues, InputId, ValueId},
    flow::FlowContext,
    pure::PureContext,
};

pub struct CodexComponent {
    id: CodexId,
    trigger: SmallVec<[InputId; 8]>,
    continuations: SmallVec<[(usize, usize, CodexValues); 8]>,
}

impl Component for CodexComponent {}

impl CodexComponent {
    pub fn new(id: CodexId) -> Self {
        CodexComponent {
            id,
            trigger: SmallVec::new(),
            continuations: SmallVec::new(),
        }
    }

    pub fn id(&self) -> CodexId {
        self.id
    }

    pub fn trigger(&mut self, input: InputId) {
        self.trigger.push(input);
    }
}

impl CodexComponent {
    pub(crate) fn push_continuation(&mut self, node: usize, outflow: usize, values: CodexValues) {
        self.continuations.push((node, outflow, values));
    }

    /// Returns next input to execute.
    pub(crate) fn drain_executions(
        &mut self,
        entity: EntityId,
        codex: &Codex,
        executions: &mut SmallVec<[(EntityId, CodexId, InputId, CodexValues); 32]>,
    ) {
        for input in self.trigger.drain(..) {
            executions.push((entity, codex.id(), input, CodexValues::new()));
        }

        for (node, outflow, values) in self.continuations.drain(..) {
            let codex_node = codex.nodes.get(node);

            match codex_node {
                None => {
                    tracing::error!("Node {} not found in codex {}", node, codex.id());
                    continue;
                }
                Some(CodexNode::Pure { .. }) => {
                    tracing::error!("Pure node continuation is not allowed");
                    continue;
                }
                Some(CodexNode::Flow { desc, follows, .. }) => {
                    if outflow >= desc.outflows.len() {
                        tracing::error!(
                            "Outflow index {} out of bounds for flow node {}",
                            outflow,
                            node
                        );
                        continue;
                    } else {
                        executions.push((entity, codex.id(), follows[outflow], values));
                    }
                }
            }
        }
    }
}
