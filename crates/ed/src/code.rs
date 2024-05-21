//! This module UI to generate flows.

use std::{any::Any, f32::consts::E, ops::Range};

use arcana::{
    code::{CodeDesc, CodeFn, CodeId, Input, Output, OutputCache, OutputId},
    Res, Stid, World,
};
use egui_snarl::{InPinId, NodeId, OutPinId, Snarl};
use hashbrown::HashSet;
use smallvec::SmallVec;

fn schedule_pure_inputs(
    node: NodeId,
    inputs: Range<usize>,
    snarl: &Snarl<CodeDesc>,
) -> Vec<NodeId> {
    let mut scheduled = HashSet::new();
    let mut queue = Vec::new();
    let mut schedule = Vec::new();

    for input in inputs {
        let inpin = snarl.in_pin(InPinId { node, input });
        assert!(inpin.remotes.len() <= 1);

        if inpin.remotes.is_empty() {
            continue;
        }

        let producer = inpin.remotes[0];
        queue.push(producer.node);
    }

    while let Some(node) = queue.pop() {
        if scheduled.contains(&node) {
            continue;
        }

        let mut can_schedule = true;

        match snarl.get_node(node) {
            None => continue,
            Some(CodeDesc::Pure { inputs, .. }) => {
                for input in 0..inputs.len() {
                    let inpin = snarl.in_pin(InPinId { node, input });
                    assert!(inpin.remotes.len() <= 1);

                    if !inpin.remotes.is_empty() {
                        let producer = inpin.remotes[0];
                        if !scheduled.contains(&producer.node) {
                            can_schedule = false;
                            queue.push(producer.node);
                        }
                    }
                }
            }
            Some(CodeDesc::Flow { .. }) => continue,
        }

        if can_schedule {
            scheduled.insert(node);
            schedule.push(node);
        }
    }

    schedule
}

pub enum Error {
    NodeNotFound,
    WrongNodeKind,
    WrongCode,
    BadConnection,
}

/// Execute specific pure code.
fn execute_pure(
    node: NodeId,
    snarl: &Snarl<CodeDesc>,
    world: &mut World,
    cache: &mut OutputCache,
    mut get_code: impl FnMut(CodeId) -> CodeFn,
) -> Result<(), Error> {
    match snarl.get_node(node) {
        None => Err(Error::NodeNotFound),
        Some(CodeDesc::Flow { .. }) => Err(Error::WrongNodeKind),
        Some(CodeDesc::Pure {
            inputs,
            outputs,
            code,
        }) => {
            let code_fn = match get_code(*code) {
                CodeFn::Pure(code_fn) => code_fn,
                CodeFn::Flow(_) => return Err(Error::WrongCode),
            };

            let mut outputs = (0..outputs.len())
                .map(|output| {
                    cache.output(OutputId {
                        node: node.0,
                        output,
                    })
                })
                .collect::<SmallVec<[_; 8]>>();

            let inputs = (0..inputs.len())
                .map(|input| {
                    let in_pin = snarl.in_pin(InPinId { node, input });
                    assert!(in_pin.remotes.len() <= 1);
                    let out_pin_id = in_pin.remotes[0];

                    cache.input(OutputId {
                        node: out_pin_id.node.0,
                        output: out_pin_id.output,
                    })
                })
                .collect::<SmallVec<[_; 8]>>();

            code_fn(&inputs, &mut outputs, world);
            Ok(())
        }
    }
}

/// Execute specific flow code.
fn execute_flow(
    node: NodeId,
    inflow: usize,
    snarl: &Snarl<CodeDesc>,
    world: &mut World,
    cache: &mut OutputCache,
    mut get_code: impl FnMut(CodeId) -> CodeFn,
) -> Result<Option<usize>, Error> {
    match snarl.get_node(node) {
        None => Err(Error::NodeNotFound),
        Some(CodeDesc::Pure { .. }) => Err(Error::WrongNodeKind),
        Some(CodeDesc::Flow {
            inflows,
            inputs,
            outputs,
            code,
            ..
        }) => {
            // Check flow connection.
            if inflow >= *inflows {
                return Err(Error::BadConnection);
            }

            // Grab code function.
            let code_fn = match get_code(*code) {
                CodeFn::Pure(_) => return Err(Error::WrongCode),
                CodeFn::Flow(code_fn) => code_fn,
            };

            // Schedule pure deps.
            let schedule = schedule_pure_inputs(node, *inflows..*inflows + inputs.len(), snarl);

            // Execute pure deps.
            for node in schedule {
                execute_pure(node, snarl, world, cache, &mut get_code)?;
            }

            // Collect outputs.
            let mut outputs = (0..outputs.len())
                .map(|output| {
                    cache.output(OutputId {
                        node: node.0,
                        output,
                    })
                })
                .collect::<SmallVec<[_; 8]>>();

            let next = {
                // Collect inputs.
                let inputs = (*inflows..*inflows + inputs.len())
                    .map(|input| {
                        let in_pin = snarl.in_pin(InPinId { node, input });
                        assert!(in_pin.remotes.len() <= 1);
                        let out_pin_id = in_pin.remotes[0];

                        cache.input(OutputId {
                            node: out_pin_id.node.0,
                            output: out_pin_id.output,
                        })
                    })
                    .collect::<SmallVec<[_; 8]>>();

                code_fn(inflow, &inputs, &mut outputs, world)
            };

            for (output, slot) in outputs.into_iter().enumerate() {
                cache.set_output(
                    OutputId {
                        node: node.0,
                        output,
                    },
                    slot,
                )
            }

            Ok(next)
        }
    }
}

/// Trigger one specific out flow.
pub fn trigger_flow(
    mut outflow: OutPinId,
    snarl: &Snarl<CodeDesc>,
    world: &mut World,
    cache: &mut OutputCache,
    mut get_code: impl FnMut(CodeId) -> CodeFn,
) -> Result<(), Error> {
    loop {
        match snarl.get_node(outflow.node) {
            None => return Err(Error::NodeNotFound),
            Some(CodeDesc::Pure { .. }) => return Err(Error::WrongNodeKind),
            Some(CodeDesc::Flow { outflows, .. }) => {
                if outflow.output >= *outflows {
                    return Err(Error::BadConnection);
                }
            }
        }

        let outpin = snarl.out_pin(outflow);
        assert!(outpin.remotes.len() <= 1);

        if outpin.remotes.is_empty() {
            // Leaf pin.
            break;
        }

        let inflow = outpin.remotes[0];

        let next = execute_flow(
            inflow.node,
            inflow.input,
            snarl,
            world,
            cache,
            &mut get_code,
        )?;

        match next {
            None => break,
            Some(output) => {
                outflow = OutPinId {
                    node: inflow.node,
                    output,
                };
            }
        }
    }

    Ok(())
}
