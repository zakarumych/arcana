//! Building blocks for visual programming.

use std::{any::Any, marker::PhantomData};

use edict::{world::WorldLocal, Component, EntityId, World};
use hashbrown::HashMap;

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
pub type PureCode = fn(inputs: &[Input], outputs: &mut [Output], world: &WorldLocal);

/// Type of code function.
/// It takes list of inputs and outputs to produce.
/// It also takes index of input flow that triggered execution.
/// It returns output flow index to trigger next flow function.
pub type FlowCode = fn(
    id: EntityId,
    idx: usize,
    flow_in: usize,
    inputs: &[Input],
    outputs: &mut [Output],
    world: &WorldLocal,
) -> Option<usize>;

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

pub struct CodeSchedule {
    queue: Vec<(EntityId, OutputId)>,
}

impl CodeSchedule {
    fn new() -> Self {
        CodeSchedule { queue: Vec::new() }
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (EntityId, OutputId)> + '_ {
        self.queue.drain(..)
    }
}

pub fn schedule_code_flow(entity: EntityId, outflow: OutputId, world: &mut World) {
    world
        .with_resource(CodeSchedule::new)
        .queue
        .push((entity, outflow));
}

/// Predefined code flow events.
pub mod events {

    use crate::local_name_hash_id;

    use super::CodeId;

    /// Event that occurs when flow graph is started.
    /// Either at the beginning of the game or when flow graph is created during the game.
    pub const START: CodeId = local_name_hash_id!(START => CodeId);
}
