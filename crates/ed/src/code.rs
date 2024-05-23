//! This module UI to generate flows.

use std::ops::Range;

use arcana::{
    code::{CodeDesc, CodeId, Continuation, FlowCode, InputId, OutputCache, OutputId, PureCode},
    edict::{
        self, spawn_block,
        world::{World, WorldLocal},
        Component, EntityId,
    },
    events::EventId,
    flow::FlowEntity,
    Name,
};
use egui::{epaint::PathShape, Color32, Painter, PointerButton, Rect, Shape, Stroke, Ui};
use egui_snarl::{
    ui::{CustomPinShape, PinInfo, PinShape, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

use crate::{data::ProjectData, hue_hash};

#[derive(Clone, Copy, Component)]
#[edict(name = "Code")]
pub struct Code {
    id: CodeId,
}

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
    entity: FlowEntity,
    node: NodeId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    get_pure_code: &HashMap<CodeId, PureCode>,
) {
    let Some(code_node) = snarl.get_node(node) else {
        tracing::error!("Pure node {node:?} was not found");
        return;
    };

    match code_node.desc {
        CodeDesc::Pure {
            ref inputs,
            ref outputs,
        } => {
            let pure_code = get_pure_code[&code_node.id];

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

            pure_code(entity, &inputs, &mut outputs);
        }
        _ => {
            tracing::error!("Node {node:?} is not pure");
            return;
        }
    }
}

/// Execute specific flow code.
fn execute_flow(
    mut entity: FlowEntity,
    pin: InPinId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    get_pure_code: &HashMap<CodeId, PureCode>,
    get_flow_code: &HashMap<CodeId, FlowCode>,
) -> Option<usize> {
    let Some(code_node) = snarl.get_node(pin.node) else {
        tracing::error!("Pure node {:?} was not found", pin.node);
        return None;
    };

    match code_node.desc {
        CodeDesc::Flow {
            inflows,
            ref inputs,
            ref outputs,
            ..
        } => {
            // Check flow connection.
            if pin.input >= inflows {
                tracing::error!("Flow {:?} doesn't have inflow {}", pin.node, pin.input);
                return None;
            }

            // Grab code function.
            let flow_code = get_flow_code[&code_node.id];

            // Schedule pure deps.
            let schedule = schedule_pure_inputs(pin.node, inflows..inflows + inputs.len(), snarl);

            // Execute pure deps.
            for node in schedule {
                execute_pure(entity.reborrow(), node, snarl, cache, get_pure_code);
            }

            // Collect outputs.
            let mut outputs = (0..outputs.len())
                .map(|output| {
                    cache.take_output(OutputId {
                        node: pin.node.0,
                        output,
                    })
                })
                .collect::<SmallVec<[_; 8]>>();

            let continuation = {
                // Collect inputs.
                let Some(inputs) = (inflows..inflows + inputs.len())
                    .map(|input| {
                        let in_pin = snarl.in_pin(InPinId {
                            node: pin.node,
                            input,
                        });
                        assert!(in_pin.remotes.len() <= 1);
                        let out_pin_id = in_pin.remotes[0];

                        cache.input(OutputId {
                            node: out_pin_id.node.0,
                            output: out_pin_id.output,
                        })
                    })
                    .collect::<Option<SmallVec<[_; 8]>>>()
                else {
                    tracing::error!("Code node {:?} has missing inputs", pin.node);
                    return None;
                };

                flow_code(
                    InputId {
                        node: pin.node.0,
                        input: pin.input,
                    },
                    entity.reborrow(),
                    &inputs,
                    &mut outputs,
                )
            };

            for (output, slot) in outputs.into_iter().enumerate() {
                cache.put_output(
                    OutputId {
                        node: pin.node.0,
                        output,
                    },
                    slot,
                )
            }

            match continuation {
                Continuation::Continue(output) => Some(output),
                Continuation::Await(future) => {
                    let node = pin.node.0;
                    // spawn_block!(for entity -> {
                    //     let outflow = OutputId {
                    //         node,
                    //         output: future.await
                    //     };
                    //     run_code_after(entity.id(), outflow, &entity.world());
                    // });
                    None
                }
            }
        }
        _ => {
            tracing::error!("Node {:?} is not flow", pin.node);
            return None;
        }
    }
}

/// Execute code after specific outflow.
fn execute_code_after(
    mut entity: FlowEntity,
    mut outflow: OutPinId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    get_pure_code: &HashMap<CodeId, PureCode>,
    get_flow_code: &HashMap<CodeId, FlowCode>,
) {
    loop {
        let Some(code_node) = snarl.get_node(outflow.node) else {
            tracing::error!("Code node {:?} was not found", outflow.node);
            return;
        };

        match code_node.desc {
            CodeDesc::Pure { .. } => {
                tracing::error!("Node {:?} is not event or flow", outflow.node);
                return;
            }
            CodeDesc::Event { .. } => {
                if outflow.output > 0 {
                    tracing::error!("Events dont have outflow {:?}", outflow.output);
                    return;
                }
            }
            CodeDesc::Flow { outflows, .. } => {
                if outflow.output >= outflows {
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
            entity.reborrow(),
            inflow,
            snarl,
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

pub fn on_code_event(
    entity: EntityId,
    event: EventId,
    world: &mut World,
    codes: &HashMap<CodeId, Snarl<CodeNode>>,
    pure: &HashMap<CodeId, PureCode>,
    flow: &HashMap<CodeId, FlowCode>,
    cache: &mut HashMap<EntityId, OutputCache>,
) {
    let Ok(Some(Code { id, .. })) = world.try_get_cloned::<Code>(entity) else {
        tracing::error!("Entity {:?} was despawned", entity);
        return;
    };

    let Some(snarl) = codes.get(&id) else {
        tracing::error!("Code {id:?} is not found");
        return;
    };

    let node = snarl
        .node_ids()
        .find_map(|(node_id, node)| match node.desc {
            CodeDesc::Event { id, .. } if id == event => Some(node_id),
            _ => None,
        });

    let Some(node) = node else {
        tracing::error!("Event {event:?} is not found");
        return;
    };

    let cache = cache.entry(entity).or_insert_with(OutputCache::new);

    let guard = edict::tls::Guard::new(world);

    let Ok(entity) = guard.entity(entity) else {
        return;
    };

    execute_code_after(
        entity,
        OutPinId { node, output: 0 },
        snarl,
        cache,
        &pure,
        &flow,
    );
}

struct RunCodeAfterQueue {
    queue: Vec<(EntityId, OutPinId)>,
}

pub(crate) fn run_code_after(entity: EntityId, outflow: OutputId, world: &World) {
    let outflow = OutPinId {
        node: NodeId(outflow.node),
        output: outflow.output,
    };

    world
        .expect_resource_mut::<RunCodeAfterQueue>()
        .queue
        .push((entity, outflow));
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CodeGraph {
    name: Name,
    snarl: Snarl<CodeNode>,
    events: HashMap<(), OutPinId>,
}

struct CodeViewer;

impl SnarlViewer<CodeNode> for CodeViewer {
    fn title(&mut self, node: &CodeNode) -> String {
        node.name.to_string()
    }

    fn inputs(&mut self, node: &CodeNode) -> usize {
        match node.desc {
            CodeDesc::Event { .. } => 0,
            CodeDesc::Pure { ref inputs, .. } => inputs.len(),
            CodeDesc::Flow {
                inflows,
                ref inputs,
                ..
            } => inflows + inputs.len(),
        }
    }

    fn outputs(&mut self, node: &CodeNode) -> usize {
        match node.desc {
            CodeDesc::Event { ref outputs, .. } => 1 + outputs.len(),
            CodeDesc::Pure { ref outputs, .. } => outputs.len(),
            CodeDesc::Flow {
                outflows,
                ref outputs,
                ..
            } => outflows + outputs.len(),
        }
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
            CodeDesc::Event { .. } => {
                ui.label(node.name.to_string());
            }
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
            CodeDesc::Event { .. } => unreachable!(),
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
            CodeDesc::Event { ref outputs, .. } => {
                if pin.id.output == 0 {
                    flow_pin()
                } else {
                    let output = outputs[pin.id.output - 1];
                    PinInfo::square().with_fill(hue_hash(&output))
                }
            }
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

pub struct Codes {
    selected: Option<CodeId>,
}

impl Codes {
    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let mut codes = world.expect_resource_mut::<Codes>();
        let mut data = world.expect_resource_mut::<ProjectData>();

        let mut cbox = egui::ComboBox::from_id_source("selected-code");
        if let Some(selected) = codes.selected {
            if let Some(code) = data.codes.get(&selected) {
                cbox = cbox.selected_text(code.name.to_string());
            } else {
                codes.selected = None;
            }
        }

        ui.vertical(|ui| {
            cbox.show_ui(ui, |ui| {
                for (&id, code) in data.codes.iter() {
                    let r = ui.selectable_label(Some(id) == codes.selected, code.name.to_string());

                    if r.clicked_by(PointerButton::Primary) {
                        codes.selected = Some(id);
                        ui.close_menu();
                    }
                }
            });

            let Some(id) = codes.selected else {
                return;
            };

            let Some(code) = data.codes.get_mut(&id) else {
                return;
            };

            code.snarl
                .show(&mut CodeViewer, &SnarlStyle::default(), "code-viwer", ui);
        });
    }
}
