//! Building blocks for visual programming.

use std::future::Future;

use arcana_intern::Name;
use edict::{component::Component, entity::EntityId, flow::FlowEntity, world::World};
use hashbrown::{hash_map::Entry, HashMap};

use arcana_id::{make_uid, Stid};
use arcana_tany::TAny;

make_uid! {
    /// ID of the code graph
    pub CodexId;
}

/// Codex descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CodexDesc {
    name: Name,
    signature: CodexSignature,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CodexSignature {
    /// Pure node gets executed every type its output is required.
    Pure {
        inputs: Vec<(Stid, Name)>,
        outputs: Vec<(Stid, Name)>,
    },

    /// Flow node that gets executed when triggered by connected inflow.
    Flow {
        inflows: usize,
        outflows: usize,
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
    },
}

pub trait IntoPureCodex<I, O> {
    fn into_pure_code(self) -> (CodexDesc, PureCodex);
}

macro_rules! into_pure_code {
    ($($a:ident)*, $($b:ident)*) => {
        impl<F $(,$b)* $(,$a)*> IntoPureCodex<($($a,)*), ($($b,)*)> for F
        where
            F: Fn(FlowEntity, $(&$a,)*) -> ($($b,)*) + Copy,
            $($a: HasStid,)*
            $($b: HasStid + Send + Sync,)*
        {
            fn into_pure_code(self) -> (CodexDesc, PureCodex) {
                #![allow(unused, non_snake_case)]

                const {
                    if ::core::mem::size_of::<F>() != 0 {
                        panic!("Code function must be zero-sized")
                    }
                }

                let desc = CodexDesc::Pure {
                    inputs: vec![$(<$a as HasStid>::stid(),)*],
                    outputs: vec![$(<$b as HasStid>::stid(),)*],
                };

                let code = |entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], values: &mut CodeValues| {
                    let f: F = unsafe {
                        core::mem::MaybeUninit::<F>::uninit().assume_init()
                    };

                    let mut idx = 0;
                    $(
                        let id = inputs[idx];
                        let Some($a) = values.get::<$a>(id) else {
                            return;
                        };
                        idx += 1;
                    )*

                    let ($($b,)*) = f(entity, $($a,)*);

                    let mut idx = 0;
                    $(
                        let id = outputs[idx];
                        values.set(id, $b);
                        idx += 1;
                    )*
                };

                (desc, code)
            }
        }
    };
}

for_tuple_2x!(into_pure_code);

pub trait IntoFlowCodex<I, O> {
    fn into_flow_code(self) -> (CodexDesc, FlowCodex);
}

macro_rules! into_flow_code {
    ($($a:ident)*, $($b:ident)*) => {
        impl<F $(,$b)* $(,$a)*> IntoFlowCodex<($($a,)*), ($($b,)*)> for F
        where
            F: Fn(FlowEntity, $(&$a,)*) -> ($($b,)*) + Copy,
            $($a: HasStid,)*
            $($b: HasStid + Send + Sync,)*
        {
            fn into_flow_code(self) -> (CodexDesc, FlowCodex) {
                #![allow(unused, non_snake_case)]

                const {
                    if ::core::mem::size_of::<F>() != 0 {
                        panic!("Code function must be zero-sized")
                    }
                }

                let desc = CodexDesc::Flow {
                    inflows: 1,
                    outflows: 1,
                    inputs: vec![$(<$a as HasStid>::stid(),)*],
                    outputs: vec![$(<$b as HasStid>::stid(),)*],
                };

                let code = |inflow: usize, entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], mut continuation: FlowContext| {
                    let f: F = unsafe {
                        core::mem::MaybeUninit::<F>::uninit().assume_init()
                    };

                    let mut idx = 0;
                    $(
                        let id = inputs[idx];
                        let Some($a) = continuation.get::<$a>(id) else {
                            return;
                        };
                        idx += 1;
                    )*

                    let ($($b,)*) = f(entity, $($a,)*);

                    let mut idx = 0;
                    $(
                        let id = outputs[idx];
                        continuation.set(id, $b);
                        idx += 1;
                    )*

                    continuation.ready(0);
                };

                (desc, code)
            }
        }
    };
}

for_tuple_2x!(into_flow_code);

pub trait IntoAsyncFlowCodexL<'a, I, O> {
    type Fut: Future<Output = O> + Send + 'a;

    fn run(&self, entity: FlowEntity, input: I) -> Self::Fut;
}

pub trait IntoAsyncFlowCodex<I, O>: for<'a> IntoAsyncFlowCodexL<'a, I, O> {
    fn into_flow_code(self) -> (CodexDesc, FlowCodex);
}

macro_rules! into_async_flow_code {
    ($($a:ident)*, $($b:ident)*) => {
        impl<'a, F, Fut $(,$b)* $(,$a)*> IntoAsyncFlowCodexL<'a, ($($a,)*), ($($b,)*)> for F
        where
            F: Fn(FlowEntity, $($a,)*) -> Fut + Copy,
            Fut: Future<Output = ($($b,)*)> + Send + 'a,
        {
            type Fut = Fut;

            fn run(&self, entity: FlowEntity, input: ($($a,)*)) -> Fut {
                #![allow(unused, non_snake_case)]
                let ($($a,)*) = input;

                self(entity, $($a,)*)
            }
        }

        impl<F $(,$b)* $(,$a)*> IntoAsyncFlowCodex<($($a,)*), ($($b,)*)> for F
        where
            F: for<'a> IntoAsyncFlowCodexL<'a, ($($a,)*), ($($b,)*)> + Copy,
            $($a: HasStid + Clone,)*
            $($b: HasStid + Send + Sync,)*
        {
            fn into_flow_code(self) -> (CodexDesc, FlowCodex) {
                #![allow(unused, non_snake_case)]

                const {
                    if ::core::mem::size_of::<F>() != 0 {
                        panic!("Code function must be zero-sized")
                    }
                }

                let desc = CodexDesc::Flow {
                    inflows: 1,
                    outflows: 1,
                    inputs: vec![$(<$a as HasStid>::stid(),)*],
                    outputs: vec![$(<$b as HasStid>::stid(),)*],
                };

                let code = |inflow: usize, entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], mut continuation: FlowContext| {
                    let f: F = unsafe {
                        core::mem::MaybeUninit::<F>::uninit().assume_init()
                    };

                    let mut idx = 0;
                    $(
                        let id = inputs[idx];
                        let Some($a) = continuation.get::<$a>(id) else {
                            return;
                        };
                        idx += 1;
                    )*

                    let fut = f.run(entity, ($($a.clone(),)*));

                    continuation.delay(entity, fut, |($($b,)*), outputs, values| {
                        let mut idx = 0;
                        $(
                            let id = outputs[idx];
                            values.set(id, $b);
                            idx += 1;
                        )*

                        0
                    });
                };

                (desc, code)
            }
        }
    };
}

for_tuple_2x!(into_async_flow_code);

pub mod builtin {
    use edict::{component::Component, query::Entities, world::World};

    use crate::events::{Event, EventId, Events};

    use super::CodexId;

    /// Event emitted when entity gets `Code` component.
    pub const CODES_START: EventId = local_name_hash_id!(CODES_START => EventId);

    #[derive(Clone, Copy)]
    struct CodeStarted;

    impl Component for CodeStarted {
        fn name() -> &'static str {
            "CodeStarted"
        }
    }

    pub fn emit_code_start(world: &mut World) {
        let world = &*world.local();
        let mut events = world.expect_resource_mut::<Events>();
        let view = world
            .view::<Entities>()
            .with::<CodexId>()
            .without::<CodeStarted>();

        for entity in view {
            events.emit(Event::new(CODES_START, entity));
            world.insert_defer(entity, CodeStarted);
        }
    }
}

struct AsyncContinue {
    entity: EntityId,
    codes: CodexId,
    node: usize,
    outflow: usize,
    values: CodeValues,
}

struct AsyncContinueQueue {
    queue: Vec<AsyncContinue>,
}

impl AsyncContinueQueue {
    fn new() -> Self {
        AsyncContinueQueue { queue: Vec::new() }
    }

    fn extend(&mut self, other: &mut Self) {
        self.queue.extend(other.queue.drain(..));
    }

    fn drain(&mut self) -> impl Iterator<Item = AsyncContinue> + '_ {
        self.queue.drain(..)
    }

    fn clear(&mut self) {
        self.queue.clear();
    }
}

fn enqueue_async_continue(
    entity: EntityId,
    codes: CodexId,
    node: usize,
    outflow: usize,
    values: CodeValues,
    world: &World,
) {
    let mut codes_after_schedule = world.expect_resource_mut::<AsyncContinueQueue>();

    codes_after_schedule.queue.push(AsyncContinue {
        entity,
        codes,
        node,
        outflow,
        values,
    });
}

pub fn init_codex(world: &mut World) {
    world.insert_resource(AsyncContinueQueue::new());
}

struct CodexNode {
    desc: CodexDesc,
    kind: CodexNodeKind,
}

enum CodexNodeKind {
    Pure(PureCodex),
    Flow(FlowCodex),
}

/// Component that represent codex associated with an entity.
pub struct Codex {
    nodes: Vec<CodexNode>,
}

struct CodexTask {
    node: usize,
    inflow: usize,
}

pub struct CodexComponent {
    id: CodexId,
    tasks: Vec<CodexTask>,
}
