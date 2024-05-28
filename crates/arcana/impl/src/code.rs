//! Building blocks for visual programming.

use std::future::Future;

use edict::{flow::FlowEntity, spawn_block, Component, EntityId, World};
use hashbrown::{hash_map::Entry, HashMap};
use smallvec::SmallVec;

use crate::{
    make_id,
    stid::{Stid, WithStid},
    Slot,
};

make_id!(pub CodeId);

#[derive(Clone, Copy, Component)]
#[edict(name = "Code")]
pub struct Code {
    pub code_id: CodeId,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputId {
    pub node: usize,
    pub input: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId {
    pub node: usize,
    pub output: usize,
}

#[derive(Default)]
pub struct CodeValues {
    values: HashMap<ValueId, Slot>,
}

impl CodeValues {
    pub fn new() -> Self {
        CodeValues {
            values: HashMap::new(),
        }
    }

    pub fn get<T: 'static>(&self, id: ValueId) -> Option<&T> {
        self.values.get(&id).and_then(|s| s.get())
    }

    pub fn set<T>(&mut self, id: ValueId, value: T)
    where
        T: Send + 'static,
    {
        match self.values.entry(id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set(value);
            }
            Entry::Vacant(entry) => {
                entry.insert(Slot::with_value(value));
            }
        }
    }

    pub fn slot(&mut self, id: ValueId) -> &mut Slot {
        self.values.entry(id).or_default()
    }
}

/// Type of pure code function.
/// It takes list of inputs and outputs to produce.
/// Generally it should not have any visible side effects.
/// Its execution may occur at any point or not occur at all.
pub type PureCode =
    fn(entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], values: &mut CodeValues);

pub enum ContinuationProvider {}

pub struct Continuation<'a> {
    node: usize,
    code: CodeId,
    values: &'a mut Option<CodeValues>,
    next: &'a mut Option<usize>,
    outputs: &'a [ValueId],
}

impl<'a> Continuation<'a> {
    pub fn new(
        node: usize,
        code: CodeId,
        values: &'a mut Option<CodeValues>,
        next: &'a mut Option<usize>,
        outputs: &'a [ValueId],
    ) -> Self {
        Continuation {
            node,
            code,
            values,
            next,
            outputs,
        }
    }

    pub fn get<T: 'static>(&self, id: ValueId) -> Option<&T> {
        self.values.as_ref().unwrap().get(id)
    }

    pub fn set<T>(&mut self, id: ValueId, value: T)
    where
        T: Send + 'static,
    {
        self.values.as_mut().unwrap().set(id, value);
    }

    pub fn ready(self, outflow: usize) {
        *self.next = Some(outflow);
    }

    pub fn delay<T, F, Fut>(self, mut entity: FlowEntity<'_>, fut: Fut, f: F)
    where
        F: FnOnce(T, &[ValueId], &mut CodeValues) -> usize + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        let outputs = SmallVec::<[_; 8]>::from_slice(self.outputs);
        let mut values = self.values.take().unwrap();
        let node = self.node;
        let code = self.code;

        spawn_block!(for entity -> {
            let res = fut.await;
            let outflow = f(res, &outputs, &mut values);

            enque_async_continue(entity.id(), code, node, outflow, values, &entity.world())
        });
    }
}

/// Type of code function.
/// It takes list of inputs and outputs to produce.
/// It also takes index of input flow that triggered execution.
/// It returns output flow index to trigger next flow function.
pub type FlowCode = fn(
    inflow: usize,
    entity: FlowEntity,
    inputs: &[ValueId],
    outputs: &[ValueId],
    continuation: Continuation,
);

/// Code descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CodeDesc {
    /// Pure node gets executed every type its output is required.
    Pure {
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
    },

    /// Flow node that gets executed when triggered by connected inflow.
    Flow {
        inflows: usize,
        outflows: usize,
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
    },
}

pub trait IntoPureCode<I, O> {
    fn into_pure_code(self) -> (CodeDesc, PureCode);
}

macro_rules! into_pure_code {
    ($($a:ident)*, $($b:ident)*) => {
        impl<F $(,$b)* $(,$a)*> IntoPureCode<($($a,)*), ($($b,)*)> for F
        where
            F: Fn(FlowEntity, $(&$a,)*) -> ($($b,)*) + Copy,
            $($a: WithStid,)*
            $($b: WithStid + Send,)*
        {
            fn into_pure_code(self) -> (CodeDesc, PureCode) {
                #![allow(unused, non_snake_case)]

                const {
                    if ::core::mem::size_of::<F>() != 0 {
                        panic!("Code function must be zero-sized")
                    }
                }

                let desc = CodeDesc::Pure {
                    inputs: vec![$(<$a as WithStid>::stid(),)*],
                    outputs: vec![$(<$b as WithStid>::stid(),)*],
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

pub trait IntoFlowCode<I, O> {
    fn into_flow_code(self) -> (CodeDesc, FlowCode);
}

macro_rules! into_flow_code {
    ($($a:ident)*, $($b:ident)*) => {
        impl<F $(,$b)* $(,$a)*> IntoFlowCode<($($a,)*), ($($b,)*)> for F
        where
            F: Fn(FlowEntity, $(&$a,)*) -> ($($b,)*) + Copy,
            $($a: WithStid,)*
            $($b: WithStid + Send,)*
        {
            fn into_flow_code(self) -> (CodeDesc, FlowCode) {
                #![allow(unused, non_snake_case)]

                const {
                    if ::core::mem::size_of::<F>() != 0 {
                        panic!("Code function must be zero-sized")
                    }
                }

                let desc = CodeDesc::Flow {
                    inflows: 1,
                    outflows: 1,
                    inputs: vec![$(<$a as WithStid>::stid(),)*],
                    outputs: vec![$(<$b as WithStid>::stid(),)*],
                };

                let code = |inflow: usize, entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], mut continuation: Continuation| {
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

pub trait IntoAsyncFlowCodeL<'a, I, O> {
    type Fut: Future<Output = O> + Send + 'a;

    fn run(&self, entity: FlowEntity<'a>, input: I) -> Self::Fut;
}

pub trait IntoAsyncFlowCode<I, O>: for<'a> IntoAsyncFlowCodeL<'a, I, O> {
    fn into_flow_code(self) -> (CodeDesc, FlowCode);
}

macro_rules! into_async_flow_code {
    ($($a:ident)*, $($b:ident)*) => {
        impl<'a, F, Fut $(,$b)* $(,$a)*> IntoAsyncFlowCodeL<'a, ($($a,)*), ($($b,)*)> for F
        where
            F: Fn(FlowEntity<'a>, $($a,)*) -> Fut + Copy,
            Fut: Future<Output = ($($b,)*)> + Send + 'a,
        {
            type Fut = Fut;

            fn run(&self, entity: FlowEntity<'a>, input: ($($a,)*)) -> Fut {
                #![allow(unused, non_snake_case)]
                let ($($a,)*) = input;

                self(entity, $($a,)*)
            }
        }

        impl<F $(,$b)* $(,$a)*> IntoAsyncFlowCode<($($a,)*), ($($b,)*)> for F
        where
            F: for<'a> IntoAsyncFlowCodeL<'a, ($($a,)*), ($($b,)*)> + Copy,
            $($a: WithStid + Clone,)*
            $($b: WithStid + Send,)*
        {
            fn into_flow_code(self) -> (CodeDesc, FlowCode) {
                #![allow(unused, non_snake_case)]

                const {
                    if ::core::mem::size_of::<F>() != 0 {
                        panic!("Code function must be zero-sized")
                    }
                }

                let desc = CodeDesc::Flow {
                    inflows: 1,
                    outflows: 1,
                    inputs: vec![$(<$a as WithStid>::stid(),)*],
                    outputs: vec![$(<$b as WithStid>::stid(),)*],
                };

                let code = |inflow: usize, entity: FlowEntity, inputs: &[ValueId], outputs: &[ValueId], mut continuation: Continuation| {
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

                    let fut = f.run(unsafe { FlowEntity::make(entity.id()) }, ($($a.clone(),)*));

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
    use edict::{Component, Entities, World};

    use crate::{
        events::{Event, EventId, Events},
        local_name_hash_id,
    };

    use super::Code;

    /// Event emitted when entity gets `Code` component.
    pub const CODE_START: EventId = local_name_hash_id!(CODE_START => EventId);

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
            .with::<Code>()
            .without::<CodeStarted>();

        for entity in view {
            events.emit(Event::new(CODE_START, entity, ()));
            world.insert_defer(entity, CodeStarted);
        }
    }
}

pub struct AsyncContinue {
    pub entity: EntityId,
    pub code: CodeId,
    pub node: usize,
    pub outflow: usize,
    pub values: CodeValues,
}

pub struct AsyncContinueQueue {
    queue: Vec<AsyncContinue>,
}

impl AsyncContinueQueue {
    pub fn new() -> Self {
        AsyncContinueQueue { queue: Vec::new() }
    }

    pub fn extend(&mut self, other: &mut Self) {
        self.queue.extend(other.queue.drain(..));
    }

    pub fn drain(&mut self) -> impl Iterator<Item = AsyncContinue> + '_ {
        self.queue.drain(..)
    }
}

fn enque_async_continue(
    entity: EntityId,
    code: CodeId,
    node: usize,
    outflow: usize,
    values: CodeValues,
    world: &World,
) {
    let mut code_after_schedule = world.expect_resource_mut::<AsyncContinueQueue>();

    code_after_schedule.queue.push(AsyncContinue {
        entity,
        code,
        node,
        outflow,
        values,
    });
}

pub fn init_codes(world: &mut World) {
    world.insert_resource(AsyncContinueQueue::new());
}
