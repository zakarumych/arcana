use edict::flow::FlowEntity;
use hashbrown::HashSet;
use smallvec::SmallVec;

use crate::{
    codex::{Codex, CodexNode, CodexValues, InputId, NodeInput, OutputId},
    flow::FlowContext,
    pure::PureContext,
};

fn run_pure(codex: &Codex, node: usize, entity: FlowEntity, values: &mut CodexValues) {
    let mut queue = SmallVec::<[usize; 16]>::new();

    let mut enqueued = HashSet::new();
    let mut pending = HashSet::new();

    queue.push(node);
    enqueued.insert(node);

    while let Some(&node) = queue.last() {
        pending.insert(node);

        let Some(codex_node) = codex.nodes.get(node) else {
            return;
        };

        match codex_node {
            CodexNode::Pure { inputs, fun, .. } => {
                let mut delay = false;

                for input in inputs {
                    if let NodeInput::Connected(output) = *input {
                        if !values.has(output) {
                            if pending.contains(&output.node) {
                                tracing::error!(
                                    "Circular dependency detected in pure node: {}",
                                    output.node
                                );
                                return;
                            }
                            if !enqueued.contains(&output.node) {
                                queue.push(output.node);
                                enqueued.insert(output.node);
                            }
                            delay = true;
                        }
                    }
                }

                if delay {
                    continue;
                }

                fun(entity, PureContext::new(node, inputs, values));
                queue.pop();
            }
            CodexNode::Flow { .. } => {
                tracing::error!("Unexecuted flow node {node} output is required");
                return;
            }
            CodexNode::Event { .. } => {
                tracing::error!("Event node {node} output is required");
                return;
            }
        }
    }
}

pub(crate) fn execute(codex: &Codex, input: InputId, entity: FlowEntity, mut values: CodexValues) {
    if !entity.is_alive() {
        return;
    }

    let mut next = Some(input);

    while let Some(input) = next.take() {
        let Some(codex_node) = codex.nodes.get(input.node) else {
            return;
        };

        let mut outflow = None;

        match codex_node {
            CodexNode::Pure { .. } => {
                unreachable!("Pure nodes should not be executed directly")
            }
            CodexNode::Event { .. } => {
                unreachable!("Event nodes should not be executed")
            }
            CodexNode::Flow {
                desc,
                inputs,
                follows,
                fun,
            } => {
                if input.idx >= desc.inflows.len() {
                    tracing::error!(
                        "Input index {} out of bounds for flow node {}",
                        input.idx,
                        input.node
                    );
                }

                for &input in inputs {
                    if !values.has(input) {
                        run_pure(codex, input.node, entity, &mut values);
                    }
                }

                let mut outputs = SmallVec::<[OutputId; 8]>::with_capacity(desc.outputs.len());

                for idx in 0..outputs.len() {
                    outputs.push(OutputId {
                        node: input.node,
                        idx,
                    });
                }

                fun(
                    input.idx,
                    entity,
                    &inputs,
                    &outputs,
                    FlowContext::new(input.node, &mut values, &mut outflow),
                );

                if let Some(outflow) = outflow {
                    if outflow >= desc.outflows.len() {
                        tracing::error!(
                            "Output index {} out of bounds for flow node {}",
                            outflow,
                            input.node
                        );
                    }

                    next = Some(follows[outflow]);
                }
            }
        }
    }
}
