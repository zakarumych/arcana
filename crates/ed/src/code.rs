//! This module UI to generate flows.

use std::{collections::BTreeMap, hash::Hash, ops::Range};

use arcana::{
    code::{
        AsyncContinueQueue, Code, CodeDesc, CodeId, CodeValues, Continuation, FlowCode, PureCode,
        ValueId,
    },
    edict::{
        self,
        flow::FlowEntity,
        world::{World, WorldLocal},
    },
    events::{EventId, Events},
    hash_id,
    plugin::{CodeInfo, PluginsHub},
    project::Project,
    Ident, Name,
};
use egui::{epaint::PathShape, Color32, Painter, PointerButton, Rect, Shape, Stroke, Ui};
use egui_snarl::{
    ui::{CustomPinShape, PinInfo, PinShape, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

use crate::{container::Container, data::ProjectData, hue_hash};

#[derive(Default)]
struct OutputCacheEntry {
    queue: Vec<CodeValues>,
}

#[derive(Default)]
pub struct OutputCache {
    map: HashMap<CodeId, OutputCacheEntry>,
}

impl OutputCache {
    pub fn new() -> Self {
        OutputCache {
            map: HashMap::new(),
        }
    }

    pub fn grab(&mut self, code: CodeId) -> CodeValues {
        let entry = self.map.entry(code).or_default();
        entry.queue.pop().unwrap_or_default()
    }

    pub fn cache(&mut self, code: CodeId, values: CodeValues) {
        let entry = self.map.entry(code).or_default();
        entry.queue.push(values);
    }
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
    values: &mut CodeValues,
    pures: &HashMap<CodeId, PureCode>,
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
            let pure_code = pures[&code_node.id];

            let mut outputs = (0..outputs.len())
                .map(|output| ValueId {
                    node: node.0,
                    output,
                })
                .collect::<SmallVec<[_; 8]>>();

            let inputs = (0..inputs.len())
                .map(|input| {
                    let in_pin = snarl.in_pin(InPinId { node, input });
                    assert!(in_pin.remotes.len() <= 1);
                    let out_pin_id = in_pin.remotes[0];

                    ValueId {
                        node: out_pin_id.node.0,
                        output: out_pin_id.output,
                    }
                })
                .collect::<SmallVec<[_; 8]>>();

            pure_code(entity, &inputs, &mut outputs, values);
        }
        _ => {
            tracing::error!("Node {node:?} is not pure");
            return;
        }
    }
}

/// Execute specific flow code.
fn execute_flow(
    code: CodeId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
    mut entity: FlowEntity,
    pin: InPinId,
    values: &mut Option<CodeValues>,
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
            let flow_code = flows[&code_node.id];

            // Schedule pure deps.
            let schedule = schedule_pure_inputs(pin.node, inflows..inflows + inputs.len(), snarl);

            // Execute pure deps.
            for node in schedule {
                execute_pure(
                    entity.reborrow(),
                    node,
                    snarl,
                    values.as_mut().unwrap(),
                    pures,
                );
            }

            // Collect outputs.
            let outputs = (0..outputs.len())
                .map(|output| ValueId {
                    node: pin.node.0,
                    output,
                })
                .collect::<SmallVec<[_; 8]>>();

            // Collect inputs.
            let inputs = (inflows..inflows + inputs.len())
                .map(|input| {
                    let in_pin = snarl.in_pin(InPinId {
                        node: pin.node,
                        input,
                    });
                    assert!(in_pin.remotes.len() <= 1);
                    let out_pin_id = in_pin.remotes[0];

                    ValueId {
                        node: out_pin_id.node.0,
                        output: out_pin_id.output,
                    }
                })
                .collect::<SmallVec<[_; 8]>>();

            let mut next = None;
            values.get_or_insert_with(|| cache.grab(code));
            let continuation = Continuation::new(pin.node.0, code, values, &mut next, &outputs);

            flow_code(
                pin.input,
                entity.reborrow(),
                &inputs,
                &outputs,
                continuation,
            );

            next
        }
        _ => {
            tracing::error!("Node {:?} is not flow", pin.node);
            None
        }
    }
}

fn run_code(
    code: CodeId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
    mut entity: FlowEntity,
    mut outflow: OutPinId,
    mut values: Option<CodeValues>,
) {
    loop {
        let Some(code_node) = snarl.get_node(outflow.node) else {
            tracing::error!("Code node {:?} was not found", outflow.node);
            break;
        };

        match code_node.desc {
            CodeDesc::Pure { .. } => {
                tracing::error!("Node {:?} is not event or flow", outflow.node);
                break;
            }
            CodeDesc::Event { .. } => {
                if outflow.output > 0 {
                    tracing::error!("Events dont have outflow {:?}", outflow.output);
                    break;
                }
            }
            CodeDesc::Flow { outflows, .. } => {
                if outflow.output >= outflows {
                    tracing::error!(
                        "Flow {:?} doesn't have outflow {:?}",
                        outflow.node,
                        outflow.output
                    );
                    break;
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

        values.get_or_insert_with(|| cache.grab(code));

        let next = execute_flow(
            code,
            snarl,
            cache,
            pures,
            flows,
            entity.reborrow(),
            inflow,
            &mut values,
        );

        match next {
            Some(output) => {
                outflow = OutPinId {
                    node: inflow.node,
                    output,
                };
            }
            None => break,
        }
    }

    if let Some(values) = values {
        cache.cache(code, values);
    }
}

/// Run scheduled [`CodeAfter`]
fn run_async_continations(
    world: &mut World,
    queue: &mut AsyncContinueQueue,
    cache: &mut OutputCache,
    codes: &HashMap<CodeId, CodeGraph>,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
) {
    queue.extend(&mut world.expect_resource_mut::<AsyncContinueQueue>());

    let guard = edict::tls::Guard::new(world);

    for c in queue.drain() {
        let Ok(entity) = guard.entity(c.entity) else {
            continue;
        };

        let Some(graph) = codes.get(&c.code) else {
            continue;
        };

        run_code(
            c.code,
            &graph.snarl,
            cache,
            pures,
            flows,
            entity,
            OutPinId {
                node: NodeId(c.node),
                output: c.outflow,
            },
            Some(c.values),
        )
    }
}

pub fn handle_code_events(
    world: &mut World,
    cache: &mut OutputCache,
    codes: &HashMap<CodeId, CodeGraph>,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
    start: &mut u64,
) {
    let world = world.local();

    'outer: loop {
        let events = world.expect_resource::<Events>();

        while let Some(event) = events.next(start) {
            let Ok(Some(Code { code_id, .. })) = world.try_get_cloned::<Code>(event.entity) else {
                tracing::error!("Entity {:?} was despawned", event.entity);
                continue;
            };

            let Some(graph) = codes.get(&code_id) else {
                tracing::error!("Code {code_id:?} is not found");
                continue;
            };

            let Some((node, outputs)) =
                graph
                    .snarl
                    .node_ids()
                    .find_map(|(node_id, node)| match node.desc {
                        CodeDesc::Event { id, ref outputs } if id == event.id => {
                            Some((node_id, outputs))
                        }
                        _ => None,
                    })
            else {
                continue;
            };

            if outputs.len() > event.payload.len() {
                tracing::error!(
                    "Event node {:?} with event id {:?} requires {} outputs, but event provides {} values",
                    node,
                    event.id,
                    outputs.len(),
                    event.payload.len()
                );
                continue;
            }

            let mut values = cache.grab(code_id);

            // Collect outputs.
            for idx in 0..outputs.len() {
                let slot = values.slot(ValueId {
                    node: node.0,
                    output: idx,
                });

                event.payload.clone_to(idx, slot);
            }

            let entity = event.entity;

            drop(events);

            let guard = edict::tls::Guard::new(world);

            let Ok(entity) = guard.entity(entity) else {
                return;
            };

            let outflow = OutPinId { node, output: 0 };

            run_code(
                code_id,
                &graph.snarl,
                cache,
                &pures,
                &flows,
                entity,
                outflow,
                None,
            );

            continue 'outer;
        }

        return;
    }
}

pub struct CodeContext {
    queue: AsyncContinueQueue,
    cache: OutputCache,
    next_event: u64,
}

impl CodeContext {
    pub fn new() -> Self {
        CodeContext {
            queue: AsyncContinueQueue::new(),
            cache: OutputCache::new(),
            next_event: 0,
        }
    }

    pub fn execute(&mut self, hub: &PluginsHub, data: &ProjectData, world: &mut World) {
        run_async_continations(
            world,
            &mut self.queue,
            &mut self.cache,
            &data.codes,
            &hub.pure_fns,
            &hub.flow_fns,
        );

        handle_code_events(
            world,
            &mut self.cache,
            &data.codes,
            &hub.pure_fns,
            &hub.flow_fns,
            &mut self.next_event,
        );
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CodeGraph {
    name: Name,
    snarl: Snarl<CodeNode>,
    events: HashMap<EventId, OutPinId>,
}

struct CodeViewer<'a> {
    available: &'a BTreeMap<Ident, Vec<CodeInfo>>,
}

impl SnarlViewer<CodeNode> for CodeViewer<'_> {
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

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<CodeNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        scale: f32,
        snarl: &mut Snarl<CodeNode>,
    ) {
        ui.label("Add code");

        if self.available.is_empty() {
            ui.separator();
            ui.weak("No available codes");
        }

        for (&plugin, codes) in self.available.iter() {
            if codes.is_empty() {
                continue;
            }

            ui.separator();
            ui.weak(plugin.as_str());

            for code in codes {
                if ui.button(code.name.as_str()).clicked() {
                    snarl.insert_node(
                        pos,
                        CodeNode {
                            id: code.id,
                            name: code.name,
                            desc: code.desc.clone(),
                        },
                    );

                    ui.close_menu();
                    return;
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
    new_code_name: String,
    available: BTreeMap<Ident, Vec<CodeInfo>>,
}

impl Codes {
    pub fn new() -> Self {
        Codes {
            selected: None,
            new_code_name: String::new(),
            available: BTreeMap::new(),
        }
    }

    pub fn update_plugins(&mut self, _data: &mut ProjectData, container: &Container) {
        self.available.clear();

        for (name, plugin) in container.plugins() {
            let codes = self.available.entry(name).or_insert(plugin.codes());

            codes.sort_by_key(|node| node.name);
        }
    }

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
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut codes.new_code_name);

                let r = ui.small_button(egui_phosphor::regular::PLUS);
                if r.clicked_by(PointerButton::Primary) {
                    match Name::from_str(&codes.new_code_name) {
                        Ok(name) => {
                            codes.new_code_name.clear();

                            let new_code = CodeGraph {
                                name,
                                snarl: Snarl::new(),
                                events: HashMap::new(),
                            };

                            let id = hash_id!(name);
                            data.codes.insert(id, new_code);
                            codes.selected = Some(id);
                        }
                        Err(_) => {}
                    }
                }

                cbox.show_ui(ui, |ui| {
                    for (&id, code) in data.codes.iter() {
                        let r =
                            ui.selectable_label(Some(id) == codes.selected, code.name.to_string());

                        if r.clicked_by(PointerButton::Primary) {
                            codes.selected = Some(id);
                            ui.close_menu();
                        }
                    }
                });
            });

            let Some(id) = codes.selected else {
                return;
            };

            let Some(code) = data.codes.get_mut(&id) else {
                return;
            };

            code.snarl.show(
                &mut CodeViewer {
                    available: &codes.available,
                },
                &SnarlStyle::default(),
                "code-viwer",
                ui,
            );
        });

        let project = world.expect_resource::<Project>();
        try_log_err!(data.sync(&project));
    }
}
