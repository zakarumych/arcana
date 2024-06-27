//! This module UI to generate flows.

use std::{collections::BTreeMap, hash::Hash, ops::Range};

use arcana::{
    code::{
        AsyncContinueQueue, CodeDesc, CodeId, CodeValues, Codes, CodesId, Continuation, FlowCode,
        PureCode, ValueId,
    },
    edict::{self, flow::Entity, world::World},
    events::{EventId, Events},
    hash_id,
    plugin::{CodeInfo, EventInfo, PluginsHub},
    project::Project,
    Ident, Name, NameError, Stid,
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
    map: HashMap<CodesId, OutputCacheEntry>,
}

impl OutputCache {
    pub fn new() -> Self {
        OutputCache {
            map: HashMap::new(),
        }
    }

    pub fn grab(&mut self, codes: CodesId) -> CodeValues {
        let entry = self.map.entry(codes).or_default();
        entry.queue.pop().unwrap_or_default()
    }

    pub fn cache(&mut self, codes: CodesId, values: CodeValues) {
        let entry = self.map.entry(codes).or_default();
        entry.queue.push(values);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CodeNode {
    Event {
        id: EventId,
        name: Name,
        outputs: Vec<Stid>,
    },
    /// Pure node gets executed every type its output is required.
    Pure {
        id: CodeId,
        name: Name,
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
    },

    /// Flow node that gets executed when triggered by connected inflow.
    Flow {
        id: CodeId,
        name: Name,
        inflows: usize,
        outflows: usize,
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
    },
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
            Some(CodeNode::Pure { inputs, .. }) => {
                for input in 0..inputs.len() {
                    let inpin = snarl.in_pin(InPinId { node, input });
                    assert!(inpin.remotes.len() <= 1);

                    if !inpin.remotes.is_empty() {
                        let producer = inpin.remotes[0];
                        if !scheduled.contains(&producer.node) {
                            match snarl.get_node(producer.node) {
                                Some(CodeNode::Pure { .. }) => {
                                    if !delay {
                                        queue.push(node);
                                    }
                                    delay = true;
                                    queue.push(producer.node);
                                }
                                _ => {
                                    scheduled.insert(producer.node);
                                }
                            }
                        }
                    }
                }
            }
            _ => continue,
        }

        if !delay {
            scheduled.insert(node);
            schedule.push(node);
        }
    }

    schedule
}

/// Execute specific pure code.
fn execute_pure(
    entity: Entity,
    node: NodeId,
    snarl: &Snarl<CodeNode>,
    values: &mut CodeValues,
    pures: &HashMap<CodeId, PureCode>,
) {
    let Some(code_node) = snarl.get_node(node) else {
        tracing::error!("Pure node {node:?} was not found");
        return;
    };

    match *code_node {
        CodeNode::Pure {
            id,
            ref inputs,
            ref outputs,
            ..
        } => {
            let pure_code = pures[&id];

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
    codes: CodesId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
    mut entity: Entity,
    pin: InPinId,
    values: &mut Option<CodeValues>,
) -> Option<usize> {
    let Some(code_node) = snarl.get_node(pin.node) else {
        tracing::error!("Pure node {:?} was not found", pin.node);
        return None;
    };

    match *code_node {
        CodeNode::Flow {
            inflows,
            ref inputs,
            ref outputs,
            id,
            ..
        } => {
            // Check flow connection.
            if pin.input >= inflows {
                tracing::error!("Flow {:?} doesn't have inflow {}", pin.node, pin.input);
                return None;
            }

            // Grab code function.
            let flow_code = flows[&id];

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
            values.get_or_insert_with(|| cache.grab(codes));
            let continuation = Continuation::new(pin.node.0, codes, values, &mut next, &outputs);

            flow_code(
                pin.input,
                entity.reborrow(),
                &inputs,
                &outputs,
                continuation,
            );

            tracing::debug!("Next is {:?}", next);

            assert_ne!(next.is_some(), values.is_some());

            next
        }
        _ => {
            tracing::error!("Node {:?} is not flow", pin.node);
            None
        }
    }
}

fn run_codes(
    codes: CodesId,
    snarl: &Snarl<CodeNode>,
    cache: &mut OutputCache,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
    mut entity: Entity,
    mut outflow: OutPinId,
    mut values: Option<CodeValues>,
) {
    loop {
        let Some(code_node) = snarl.get_node(outflow.node) else {
            tracing::error!("Code node {:?} was not found", outflow.node);
            break;
        };

        match *code_node {
            CodeNode::Event { .. } => {
                if outflow.output > 0 {
                    tracing::error!("Events dont have outflow {:?}", outflow.output);
                    break;
                }
            }
            CodeNode::Pure { .. } => {
                tracing::error!("Node {:?} is not event or flow", outflow.node);
                break;
            }

            CodeNode::Flow { outflows, .. } => {
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

        values.get_or_insert_with(|| cache.grab(codes));

        let next = execute_flow(
            codes,
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
        cache.cache(codes, values);
    }
}

/// Run scheduled [`CodeAfter`]
fn run_async_continations(
    world: &mut World,
    queue: &mut AsyncContinueQueue,
    cache: &mut OutputCache,
    codes: &HashMap<CodesId, CodeGraph>,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
) {
    queue.extend(&mut world.expect_resource_mut::<AsyncContinueQueue>());

    let _guard = edict::tls::Guard::new(world.local());

    // Safety: Safe to do once under tls guard.
    let world = unsafe { arcana::flow::World::make_mut() };

    for c in queue.drain() {
        let Ok(entity) = world.entity(c.entity) else {
            continue;
        };

        let Some(graph) = codes.get(&c.codes) else {
            continue;
        };

        run_codes(
            c.codes,
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
    codes: &HashMap<CodesId, CodeGraph>,
    pures: &HashMap<CodeId, PureCode>,
    flows: &HashMap<CodeId, FlowCode>,
    start: &mut u64,
) {
    let world = world.local();

    'outer: loop {
        let events = world.expect_resource::<Events>();

        while let Some(event) = events.next(start) {
            let Ok(Some(Codes { codes_id, .. })) = world.try_get_cloned(event.entity) else {
                tracing::debug!("Entity {} was despawned", event.entity);
                continue;
            };

            let Some(graph) = codes.get(&codes_id) else {
                tracing::debug!("Code {codes_id} is not found");
                continue;
            };

            let Some((node, outputs)) =
                graph
                    .snarl
                    .node_ids()
                    .find_map(|(node_id, node)| match *node {
                        CodeNode::Event {
                            id, ref outputs, ..
                        } if id == event.id => Some((node_id, outputs)),
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

            let mut values = cache.grab(codes_id);

            // Collect outputs.
            for idx in 0..outputs.len() {
                let slot = values.slot(ValueId {
                    node: node.0,
                    output: idx,
                });

                event.payload.clone_to(idx, slot);
            }

            let entity = event.entity;

            tracing::debug!("Running code {:?} for event {:?}", codes_id, event.id);

            drop(events);

            let _guard = edict::tls::Guard::new(world);
            let world = unsafe { arcana::flow::World::make_mut() };

            let Ok(entity) = world.entity(entity) else {
                return;
            };

            let outflow = OutPinId { node, output: 0 };

            run_codes(
                codes_id,
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

    pub fn reset(&mut self) {
        self.queue.clear();
        self.cache.map.clear();
        self.next_event = 0;
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
    available_events: &'a BTreeMap<Ident, Vec<EventInfo>>,
    available_codes: &'a BTreeMap<Ident, Vec<CodeInfo>>,
}

impl SnarlViewer<CodeNode> for CodeViewer<'_> {
    fn title(&mut self, node: &CodeNode) -> String {
        match *node {
            CodeNode::Event { name, .. } => name.to_string(),
            CodeNode::Flow { name, .. } => name.to_string(),
            CodeNode::Pure { name, .. } => name.to_string(),
        }
    }

    fn inputs(&mut self, node: &CodeNode) -> usize {
        match *node {
            CodeNode::Event { .. } => 0,
            CodeNode::Pure { ref inputs, .. } => inputs.len(),
            CodeNode::Flow {
                inflows,
                ref inputs,
                ..
            } => inflows + inputs.len(),
        }
    }

    fn outputs(&mut self, node: &CodeNode) -> usize {
        match *node {
            CodeNode::Event { ref outputs, .. } => 1 + outputs.len(),
            CodeNode::Pure { ref outputs, .. } => outputs.len(),
            CodeNode::Flow {
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

        match *node {
            CodeNode::Event { name, .. } => {
                ui.label(name.to_string());
            }
            CodeNode::Pure { name, .. } => {
                ui.label(name.to_string());
            }
            CodeNode::Flow { name, .. } => {
                ui.label(name.to_string());
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

        match *node {
            CodeNode::Event { .. } => unreachable!(),
            CodeNode::Pure { ref inputs, .. } => {
                let input = inputs[pin.id.input];
                PinInfo::square().with_fill(hue_hash(&input))
            }
            CodeNode::Flow {
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

        match *node {
            CodeNode::Event { ref outputs, .. } => {
                if pin.id.output == 0 {
                    flow_pin()
                } else {
                    let output = outputs[pin.id.output - 1];
                    PinInfo::square().with_fill(hue_hash(&output))
                }
            }
            CodeNode::Pure { ref outputs, .. } => {
                let output = outputs[pin.id.output];
                PinInfo::square().with_fill(hue_hash(&output))
            }
            CodeNode::Flow {
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
        _scale: f32,
        snarl: &mut Snarl<CodeNode>,
    ) {
        if !self.available_events.is_empty() {
            ui.label("Add event");
            for (&plugin, events) in self.available_events.iter() {
                if events.is_empty() {
                    continue;
                }

                ui.separator();
                ui.weak(plugin.as_str());

                for event in events {
                    if ui.button(event.name.as_str()).clicked() {
                        snarl.insert_node(
                            pos,
                            CodeNode::Event {
                                id: event.id,
                                name: event.name,
                                outputs: event.values.clone(),
                            },
                        );

                        ui.close_menu();
                        return;
                    }
                }
            }
        }
        if !self.available_codes.is_empty() {
            ui.label("Add code");
            for (&plugin, codes) in self.available_codes.iter() {
                if codes.is_empty() {
                    continue;
                }

                ui.separator();
                ui.weak(plugin.as_str());

                for code in codes {
                    if ui.button(code.name.as_str()).clicked() {
                        snarl.insert_node(
                            pos,
                            match code.desc {
                                CodeDesc::Pure {
                                    ref inputs,
                                    ref outputs,
                                } => CodeNode::Pure {
                                    id: code.id,
                                    name: code.name,
                                    inputs: inputs.clone(),
                                    outputs: outputs.clone(),
                                },
                                CodeDesc::Flow {
                                    inflows,
                                    outflows,
                                    ref inputs,
                                    ref outputs,
                                } => CodeNode::Flow {
                                    id: code.id,
                                    name: code.name,
                                    inflows,
                                    outflows,
                                    inputs: inputs.clone(),
                                    outputs: outputs.clone(),
                                },
                            },
                        );

                        ui.close_menu();
                        return;
                    }
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
        stroke: Stroke::new(1.0, Color32::WHITE),
    }));
}

fn flow_pin() -> PinInfo {
    PinInfo::default().with_shape(PinShape::Custom(CustomPinShape::new(
        |painter, rect, _, _| draw_flow_pin(painter, rect),
    )))
}

pub struct CodeTool {
    selected: Option<CodesId>,
    new_code_name: String,
    available_events: BTreeMap<Ident, Vec<EventInfo>>,
    available_codes: BTreeMap<Ident, Vec<CodeInfo>>,
}

impl CodeTool {
    pub fn new() -> Self {
        CodeTool {
            selected: None,
            new_code_name: String::new(),
            available_events: BTreeMap::new(),
            available_codes: BTreeMap::new(),
        }
    }

    pub fn update_plugins(&mut self, _data: &mut ProjectData, new: &Container) {
        self.available_codes.clear();

        for (name, plugin) in new.plugins() {
            let codes = self.available_codes.entry(name).or_insert(plugin.codes());

            codes.sort_by_key(|node| node.name);
        }

        self.available_events.clear();

        for (name, plugin) in new.plugins() {
            let events = self.available_events.entry(name).or_insert(plugin.events());
            events.sort_by_key(|node| node.name);
        }
    }

    pub fn show(&mut self, project: &Project, data: &mut ProjectData, ui: &mut Ui) {
        let mut cbox = egui::ComboBox::from_id_source("selected-code");
        if let Some(selected) = self.selected {
            if let Some(code) = data.codes.get(&selected) {
                cbox = cbox.selected_text(code.name.to_string());
            } else {
                self.selected = None;
            }
        }

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_code_name);

                let r = ui.small_button(egui_phosphor::regular::PLUS);
                if r.clicked_by(PointerButton::Primary) {
                    match Name::from_str(&self.new_code_name) {
                        Ok(name) => {
                            self.new_code_name.clear();

                            let new_code = CodeGraph {
                                name,
                                snarl: Snarl::new(),
                                events: HashMap::new(),
                            };

                            let id = hash_id!(name);
                            data.codes.insert(id, new_code);
                            self.selected = Some(id);
                        }
                        Err(NameError::Empty) => {
                            tracing::error!("Failed to create code with empty name");
                        }
                        Err(NameError::Bad(c)) => {
                            tracing::error!(
                                "Failed to create code with name \"{}\". Bad character '{}'",
                                self.new_code_name,
                                c,
                            );
                        }
                    }
                }

                cbox.show_ui(ui, |ui| {
                    for (&id, code) in data.codes.iter() {
                        let r =
                            ui.selectable_label(Some(id) == self.selected, code.name.to_string());

                        if r.clicked_by(PointerButton::Primary) {
                            self.selected = Some(id);
                            ui.close_menu();
                        }
                    }
                });
            });

            let Some(id) = self.selected else {
                return;
            };

            let Some(code) = data.codes.get_mut(&id) else {
                return;
            };

            code.snarl.show(
                &mut CodeViewer {
                    available_events: &self.available_events,
                    available_codes: &self.available_codes,
                },
                &SnarlStyle::default(),
                "code-viwer",
                ui,
            );
        });

        try_log_err!(data.sync(&project));
    }
}
