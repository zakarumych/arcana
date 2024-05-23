//! Building blocks for visual programming.

use std::{any::Any, future::Future, marker::PhantomData, pin::Pin};

use edict::{flow::FlowEntity, Component};
use hashbrown::HashMap;

use crate::{
    events::EventId,
    make_id,
    stid::{Stid, WithStid},
};

make_id!(pub CodeId);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputId {
    pub node: usize,
    pub input: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId {
    pub node: usize,
    pub output: usize,
}

pub struct Output<'a> {
    boxed: Option<Box<dyn Any>>,
    marker: PhantomData<&'a ()>,
}

impl Output<'_> {
    pub fn set<T: 'static>(&mut self, value: T) {
        if let Some(boxed) = &mut self.boxed {
            if let Some(slot) = boxed.downcast_mut::<T>() {
                *slot = value;
                return;
            }
        }
        self.boxed = Some(Box::new(value));
    }
}

pub struct Input<'a> {
    value: &'a dyn Any,
}

impl Input<'_> {
    pub fn get<T: 'static>(&self) -> &T {
        self.value.downcast_ref().unwrap()
    }
}

#[derive(Component)]
pub struct OutputCache {
    values: HashMap<OutputId, Box<dyn Any>>,
}

impl OutputCache {
    pub fn new() -> Self {
        OutputCache {
            values: HashMap::new(),
        }
    }

    pub fn input(&self, id: OutputId) -> Option<Input> {
        let boxed = self.values.get(&id)?;
        Some(Input { value: &**boxed })
    }

    pub fn take_output(&mut self, id: OutputId) -> Output<'static> {
        Output {
            boxed: self.values.remove(&id),
            marker: PhantomData,
        }
    }

    pub fn put_output(&mut self, id: OutputId, output: Output<'_>) {
        if let Some(boxed) = output.boxed {
            self.values.insert(id, boxed);
        }
    }
}

/// Type of pure code function.
/// It takes list of inputs and outputs to produce.
/// Generally it should not have any visible side effects.
/// Its execution may occur at any point or not occur at all.
pub type PureCode = fn(entity: FlowEntity, inputs: &[Input], outputs: &mut [Output]);

pub enum Continuation {
    /// Continue execution with given output flow.
    Continue(usize),

    /// Continue execution with given output flow when given future resolves.
    Await(Pin<Box<dyn Future<Output = usize> + Send>>),
}

/// Type of code function.
/// It takes list of inputs and outputs to produce.
/// It also takes index of input flow that triggered execution.
/// It returns output flow index to trigger next flow function.
pub type FlowCode = fn(
    input: InputId,
    entity: FlowEntity,
    inputs: &[Input],
    outputs: &mut [Output],
) -> Continuation;

/// Code descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CodeDesc {
    /// Event node is never executed,
    /// instead it is triggered externally and starts execution of code flow.
    /// It always has exactly one outflow and number of output values.
    /// But no inflows or input values.
    Event { id: EventId, outputs: Vec<Stid> },

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
            F: Fn(FlowEntity, $($a,)*) -> ($($b,)*) + Copy,
            $($a: WithStid + Clone,)*
            $($b: WithStid,)*
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

                let code = |entity: FlowEntity, inputs: &[Input], outputs: &mut [Output]| {
                    let f: F = unsafe {
                        core::mem::MaybeUninit::<F>::uninit().assume_init()
                    };

                    let mut idx = 0;
                    $(
                        let $a: $a = inputs[idx].get::<$a>().clone();
                        idx += 1;
                    )*

                    let ($($b,)*) = f(entity, $($a,)*);

                    let mut idx = 0;
                    $(
                        outputs[idx].set($b);
                        idx += 1;
                    )*
                };

                (desc, code)
            }
        }
    };
}

for_tuple_2x!(into_pure_code);

#[test]
fn foo() {
    fn foo(_: FlowEntity) {}

    assert_eq!(
        IntoPureCode::<(), ()>::into_pure_code(foo).0,
        CodeDesc::Pure {
            inputs: vec![],
            outputs: vec![]
        }
    );
}

/// Predefined code events.
pub mod builtin {
    use crate::{events::EventId, local_name_hash_id};

    /// Event emitted when `Code` is added to the entity, including entity spawning.
    pub const START_EVENT: EventId = local_name_hash_id!(START_EVENT => EventId);
}
