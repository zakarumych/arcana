//! This module UI to generate flows.

use std::ops::Range;

use arcana::{
    code::{CodeDesc, CodeId, CodeSchedule, FlowCode, OutputCache, OutputId, PureCode},
    edict::{self, world::WorldLocal},
    Component, EntityId, Name, World,
};
use egui::{epaint::PathShape, Color32, Painter, PointerButton, Rect, Shape, Stroke, Ui};
use egui_snarl::{
    ui::{CustomPinShape, PinInfo, PinShape, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

use crate::hue_hash;

#[derive(Component)]
#[repr(transparent)]
pub struct Code(pub CodeId);

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CodeNode {
    id: CodeId,
    name: Name,
    desc: CodeDesc,
}

fn schedule_pure_inputs(
    node: NodeId,
    inputs: Range<usize>,
    snarl: &Snarl<CodeNode>,
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

        let mut delay = false;

        match snarl.get_node(node) {
            Some(CodeNode {
                desc: CodeDesc::Pure { inputs, .. },
                ..
            }) => {
                for input in 0..inputs.len() {
                    let inpin = snarl.in_pin(InPinId { node, input });
                    assert!(inpin.remotes.len() <= 1);

                    if !inpin.remotes.is_empty() {
                        let producer = inpin.remotes[0];
                        if !scheduled.contains(&producer.node) {
                            delay = true;
                            queue.push(producer.node);
                        }
                    }
                }
            }
            _ => continue,
        }

        if delay {
            queue.push(node);
        } else {
            scheduled.insert(node);
            schedule.push(node);
        }
    }

    schedule
}

/// Execute specific pure code.
fn execute_pure(
    node: NodeId,
    snarl: &Snarl<CodeNode>,
    world: &WorldLocal,
    cache: &mut OutputCache,
    get_pure_code: &HashMap<CodeId, PureCode>,
) {
    match snarl.get_node(node) {
        None => {
            tracing::error!("Pure node {node:?} was not found");
            return;
        }
        Some(CodeNode {
            desc: CodeDesc::Flow { .. },
            ..
        }) => {
            tracing::error!("Node {node:?} is not pure");
            return;
        }
        Some(CodeNode {
            id,
            desc: CodeDesc::Pure { inputs, outputs },
            ..
        }) => {
            let pure_code = get_pure_code[id];

            let mut outputs = (0..outputs.len())
                .map(|output| {
                    cache.take_output(OutputId {
                        node: node.0,
                        output,
                    })
                })
                .collect::<SmallVec<[_; 8]>>();

            let Some(inputs) = (0..inputs.len())
                .map(|input| {
                    let in_pin = snarl.in_pin(InPinId { node, input });
                    assert!(in_pin.remotes.len() <= 1);
                    let out_pin_id = in_pin.remotes[0];

                    cache.input(OutputId {
                        node: out_pin_id.node.0,
                        output: out_pin_id.output,
                    })
                })
                .collect::<Option<SmallVec<[_; 8]>>>()
            else {
                tracing::error!("Code node {node:?} has missing inputs");
                return;
            };

            pure_code(&inputs, &mut outputs, world);
        }
    }
}

/// Execute specific flow code.
fn execute_flow(
    entity: EntityId,
    node: NodeId,
    inflow: usize,
    snarl: &Snarl<CodeNode>,
    world: &WorldLocal,
    cache: &mut OutputCache,
    get_pure_code: &HashMap<CodeId, PureCode>,
    get_flow_code: &HashMap<CodeId, FlowCode>,
) -> Option<usize> {
    match snarl.get_node(node) {
        None => {
            tracing::error!("Flow node {node:?} was not found");
            return None;
        }
        Some(CodeNode {
            desc: CodeDesc::Pure { .. },
            ..
        }) => {
            tracing::error!("Node {node:?} is not flow");
            return None;
        }
        Some(CodeNode {
            id,
            desc:
                CodeDesc::Flow {
                    inflows,
                    inputs,
                    outputs,
                    ..
                },
            ..
        }) => {
            // Check flow connection.
            if inflow >= *inflows {
                tracing::error!("Flow {node:?} doesn't have inflow {inflow}");
                return None;
            }

            // Grab code function.
            let flow_code = get_flow_code[id];

            // Schedule pure deps.
            let schedule = schedule_pure_inputs(node, *inflows..*inflows + inputs.len(), snarl);

            // Execute pure deps.
            for node in schedule {
                execute_pure(node, snarl, world, cache, get_pure_code);
            }

            // Collect outputs.
            let mut outputs = (0..outputs.len())
                .map(|output| {
                    cache.take_output(OutputId {
                        node: node.0,
                        output,
                    })
                })
                .collect::<SmallVec<[_; 8]>>();

            let next = {
                // Collect inputs.
                let Some(inputs) = (*inflows..*inflows + inputs.len())
                    .map(|input| {
                        let in_pin = snarl.in_pin(InPinId { node, input });
                        assert!(in_pin.remotes.len() <= 1);
                        let out_pin_id = in_pin.remotes[0];

                        cache.input(OutputId {
                            node: out_pin_id.node.0,
                            output: out_pin_id.output,
                        })
                    })
                    .collect::<Option<SmallVec<[_; 8]>>>()
                else {
                    tracing::error!("Code node {node:?} has missing inputs");
                    return None;
                };

                flow_code(entity, node.0, inflow, &inputs, &mut outputs, world)
            };

            for (output, slot) in outputs.into_iter().enumerate() {
                cache.put_output(
                    OutputId {
                        node: node.0,
                        output,
                    },
                    slot,
                )
            }

            next
        }
    }
}

/// Trigger one specific out flow.
fn trigger_impl(
    entity: EntityId,
    mut outflow: OutPinId,
    snarl: &Snarl<CodeNode>,
    world: &WorldLocal,
    cache: &mut OutputCache,
    get_pure_code: &HashMap<CodeId, PureCode>,
    get_flow_code: &HashMap<CodeId, FlowCode>,
) {
    loop {
        match snarl.get_node(outflow.node) {
            None => {
                tracing::error!("Flow node {:?} was not found", outflow.node);
                return;
            }
            Some(CodeNode {
                desc: CodeDesc::Pure { .. },
                ..
            }) => {
                tracing::error!("Node {:?} is not flow", outflow.node);
                return;
            }
            Some(CodeNode {
                desc: CodeDesc::Flow { outflows, .. },
                ..
            }) => {
                if outflow.output >= *outflows {
                    tracing::error!(
                        "Flow {:?} doesn't have outflow {:?}",
                        outflow.node,
                        outflow.output
                    );
                    return;
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
            entity,
            inflow.node,
            inflow.input,
            snarl,
            world,
            cache,
            get_pure_code,
            get_flow_code,
        );

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
}

pub fn trigger(entity: EntityId, outflow: OutPinId, world: &mut World) {
    let mut new_cache = None;

    {
        let world = &*world.local();

        let pure = world.expect_resource::<HashMap<CodeId, PureCode>>();
        let flow = world.expect_resource::<HashMap<CodeId, FlowCode>>();
        let code = world.expect_resource::<HashMap<CodeId, Snarl<CodeNode>>>();

        let Ok(mut view) = world.try_view_one::<(&Code, Option<&mut OutputCache>)>(entity) else {
            tracing::error!("Entity {:?} was despawned", entity);
            return;
        };
        let Some((&Code(id), cache)) = view.get_mut() else {
            tracing::error!("Entity {:?} doesn't have code", entity);
            return;
        };

        let Some(snarl) = code.get(&id) else {
            tracing::error!("Code {id:?} is not found");
            return;
        };

        let cache = match cache {
            Some(cache) => cache,
            None => new_cache.get_or_insert(OutputCache::new()),
        };

        trigger_impl(entity, outflow, snarl, world, cache, &pure, &flow);
    }

    if let Some(new_cache) = new_cache {
        let _ = world.insert(entity, new_cache);
    }
}

pub struct ScheduledCode {
    queue: Vec<(EntityId, OutPinId)>,
}

impl ScheduledCode {
    pub fn new() -> Self {
        ScheduledCode { queue: Vec::new() }
    }

    pub fn trigger(&mut self, world: &mut World) {
        if let Some(mut schedule) = world.get_resource_mut::<CodeSchedule>() {
            self.queue.extend(schedule.drain().map(|(e, o)| {
                (
                    e,
                    OutPinId {
                        node: NodeId(o.node),
                        output: o.output,
                    },
                )
            }));
        }

        for (e, o) in self.queue.drain(..) {
            trigger(e, o, world);
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CodeGraph {
    snarl: Snarl<CodeNode>,
    trigget: Option<(NodeId, OutPinId)>,
}

struct CodeViewer;

impl SnarlViewer<CodeNode> for CodeViewer {
    fn title(&mut self, node: &CodeNode) -> String {
        node.name.to_string()
    }

    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<CodeNode>,
    ) {
        let node = &snarl[node];

        match node.desc {
            CodeDesc::Pure { .. } => {
                ui.label(node.name.to_string());
            }
            CodeDesc::Flow { .. } => {
                ui.label(node.name.to_string());
            }
        }
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        _ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<CodeNode>,
    ) -> PinInfo {
        let node = &snarl[pin.id.node];

        match node.desc {
            CodeDesc::Pure { ref inputs, .. } => {
                let input = inputs[pin.id.input];
                PinInfo::square().with_fill(hue_hash(&input))
            }
            CodeDesc::Flow {
                inflows,
                ref inputs,
                ..
            } => {
                if pin.id.input < inflows {
                    flow_pin()
                } else {
                    let input = inputs[pin.id.input - inflows];
                    PinInfo::square().with_fill(hue_hash(&input))
                }
            }
        }
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        _ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<CodeNode>,
    ) -> PinInfo {
        let node = &snarl[pin.id.node];

        match node.desc {
            CodeDesc::Pure { ref outputs, .. } => {
                let output = outputs[pin.id.output];
                PinInfo::square().with_fill(hue_hash(&output))
            }
            CodeDesc::Flow {
                outflows,
                ref outputs,
                ..
            } => {
                if pin.id.output < outflows {
                    flow_pin()
                } else {
                    let output = outputs[pin.id.output - outflows];
                    PinInfo::square().with_fill(hue_hash(&output))
                }
            }
        }
    }
}

fn draw_flow_pin(painter: &Painter, rect: Rect) {
    painter.add(Shape::Path(PathShape {
        points: vec![rect.left_top(), rect.right_center(), rect.left_bottom()],
        closed: true,
        fill: Color32::WHITE,
        stroke: Stroke::new(2.0, Color32::GRAY),
    }));
}

fn flow_pin() -> PinInfo {
    PinInfo::default().with_shape(PinShape::Custom(CustomPinShape::new(
        |painter, rect, _, _| draw_flow_pin(painter, rect),
    )))
}
