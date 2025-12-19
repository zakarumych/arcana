use arcana_tany::TAny;
use edict::{component::Component, entity::EntityId};
use smallvec::SmallVec;

use crate::codex::{Codex, CodexId, CodexNode, CodexValues, InputId, OutputId};

pub struct CodexComponent {
    id: CodexId,
    triggers: SmallVec<[(usize, usize, CodexValues); 8]>,
}

impl Component for CodexComponent {}

impl CodexComponent {
    pub fn new(id: CodexId) -> Self {
        CodexComponent {
            id,
            triggers: SmallVec::new(),
        }
    }

    pub fn id(&self) -> CodexId {
        self.id
    }

    /// Trigger starting node execution.
    pub fn trigger(&mut self, node: usize, values: impl Iterator<Item = TAny>) {
        let values = CodexValues::from_iter(values.map(|v| {
            let id = OutputId {
                node,
                idx: self.triggers.len(),
            };
            (id, v)
        }));

        self.triggers.push((node, 0, values));
    }

    pub(crate) fn push_trigger(&mut self, node: usize, outflow: usize, values: CodexValues) {
        self.triggers.push((node, outflow, values));
    }

    /// Returns next input to execute.
    pub(crate) fn drain_executions(
        &mut self,
        entity: EntityId,
        id: CodexId,
        codex: &Codex,
        executions: &mut SmallVec<[(EntityId, CodexId, InputId, CodexValues); 32]>,
    ) {
        for (node, outflow, values) in self.triggers.drain(..) {
            let codex_node = codex.nodes.get(node);

            match codex_node {
                None => {
                    tracing::error!("Node {} not found in codex {}", node, id);
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
                        if let Some(follow) = follows[outflow] {
                            executions.push((entity, id, follow, values));
                        }
                    }
                }
                Some(CodexNode::Event { follow, .. }) => {
                    if outflow != 0 {
                        tracing::error!(
                            "Event node trigger must have outflow 0, got {} for node {}",
                            outflow,
                            node
                        );
                        continue;
                    }

                    if let Some(follow) = *follow {
                        executions.push((entity, id, follow, values));
                    }
                }
            }
        }
    }
}
