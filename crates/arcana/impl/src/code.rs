//! Building blocks for visual programming.

use std::{any::Any, marker::PhantomData};

use edict::World;
use hashbrown::{
    hash_map::{DefaultHashBuilder, Entry},
    HashMap,
};

use crate::{make_id, stid::Stid};

make_id!(pub CodeId);

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
    value: Option<&'a Box<dyn Any>>,
}

impl Input<'_> {
    pub fn get<T: 'static>(&self) -> Option<&T> {
        let boxed = self.value?;
        boxed.downcast_ref()
    }
}

pub struct OutputCache {
    values: HashMap<OutputId, Box<dyn Any>>,
}

impl OutputCache {
    pub fn input(&self, id: OutputId) -> Input {
        Input {
            value: self.values.get(&id),
        }
    }

    pub fn output(&mut self, id: OutputId) -> Output<'static> {
        Output {
            boxed: self.values.remove(&id),
            marker: PhantomData,
        }
    }

    pub fn set_output(&mut self, id: OutputId, output: Output<'_>) {
        if let Some(boxed) = output.boxed {
            self.values.insert(id, boxed);
        }
    }
}

/// Type of pure code function.
/// It takes list of inputs and outputs to produce.
/// Generally it should not have any visible side effects.
/// Its execution may occur at any point or not occur at all.
pub type PureFn = fn(inputs: &[Input], outputs: &mut [Output], world: &mut World);

/// Type of code function.
/// It takes list of inputs and outputs to produce.
/// It also takes index of input flow that triggered execution.
/// It returns output flow index to trigger next flow function.
pub type FlowFn = fn(
    flow_in: usize,
    inputs: &[Input],
    outputs: &mut [Output],
    world: &mut World,
) -> Option<usize>;

pub enum CodeFn {
    Pure(PureFn),
    Flow(FlowFn),
}

impl CodeFn {
    pub fn run_pure(&self, inputs: &[Input], outputs: &mut [Output], world: &mut World) {
        match self {
            CodeFn::Pure(f) => f(inputs, outputs, world),
            CodeFn::Flow(_) => panic!("expected flow function"),
        }
    }

    pub fn run_flow(
        &self,
        inflow: usize,
        inputs: &[Input],
        outputs: &mut [Output],
        world: &mut World,
    ) -> Option<usize> {
        match self {
            CodeFn::Pure(_) => panic!("expected pure function"),
            CodeFn::Flow(f) => f(inflow, inputs, outputs, world),
        }
    }
}

/// Code descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CodeDesc {
    /// Pure node gets executed every type its output is required.
    Pure {
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
        code: CodeId,
    },

    /// Flow node that gets executed when triggered by connected inflow.
    Flow {
        inflows: usize,
        outflows: usize,
        inputs: Vec<Stid>,
        outputs: Vec<Stid>,
        code: CodeId,
    },
}
