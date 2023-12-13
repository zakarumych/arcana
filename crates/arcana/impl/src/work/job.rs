//! This module defines Job API for work-graph.
//!
//! `Job` is a computational unit of work-graph.
//! It may be arbitrary complex but cannot be divided into smaller units by user.
//!
//! This trait defines Job API.
//!
//! There's three stages:
//! - setup
//! - plan
//! - execution
//!
//! Setup stage happens for each job when it is added to the graph.
//! At this stage job must declare targets it will use and targets it will produce.
//!
//! Plan stage happens each frame.
//! At this stage relations between jobs are processed to build execution plan.
//! Resources are allocated and prepared for execution.
//!
//! Execution stage happens each frame. At this stage jobs are traversed
//! in planned order to find out which jobs needs to run.
//! Then jobs are executed, possibly in parallel to fill command buffers which then
//! are submitted in order.
//!
//! Jobs communicate data between each other through resources,
//! referred by jobs as targets.
//! Job may fetch only target ids that was declared as accessed during setup.    

use std::{
    alloc::Layout,
    any::{Any, TypeId},
    cell::RefCell,
    mem::ManuallyDrop,
    num::NonZeroU64,
    ptr::NonNull,
};

use blink_alloc::{Blink, BlinkAlloc};
use hashbrown::{HashMap, HashSet};

use crate::{
    arena::Arena,
    id::{Id, IdGen},
};

use super::{
    graph::PinId,
    target::{InputId, OutputId, Target, TargetHub, TargetId, TargetInfoMerge, UpdateId},
};

pub type JobId = Id<dyn Job>;

/// Entry to the Job API.
/// Setups the job to be added to the graph.
pub trait Job {
    /// Setup the job.
    ///
    /// This method is called once when the job is added to the graph.
    /// It must declare targets it will use and targets it will produce.
    fn setup(self, setup: Setup<'_>) -> JobNode;
}

impl<F> Job for F
where
    F: FnOnce(Setup<'_>) -> JobNode,
{
    fn setup(self, setup: Setup<'_>) -> JobNode {
        self(setup)
    }
}

pub struct Setup<'a> {
    inputs: HashMap<InputId, Input>,
    outputs: HashMap<OutputId, Output>,
    idgen: &'a mut IdGen,
    merge_info: &'a mut HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
}

impl<'a> Setup<'a> {
    pub(super) fn new(
        idgen: &'a mut IdGen,
        merge_info: &'a mut HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
    ) -> Self {
        Setup {
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            idgen,
            merge_info,
        }
    }
}

impl Setup<'_> {
    /// Declare that job will produce a target.
    /// Job will have exclusive access to the target resource while it runs.
    /// Creates an output pin for the job node.
    pub fn produce<T>(&mut self) -> OutputId<T>
    where
        T: Target,
    {
        let id = self.idgen.next();
        self.outputs.insert(
            id.cast(),
            Output {
                id: TypeId::of::<T>(),
                name: T::name(),
            },
        );
        id
    }

    /// Declare that job will consume a target.
    /// Creates an input pin for the job node.
    pub fn consume<T>(&mut self) -> InputId<T>
    where
        T: Target,
    {
        let id = self.idgen.next();
        self.inputs.insert(
            id.cast(),
            Input {
                id: TypeId::of::<T>(),
                name: T::name(),
                access: Access::Exclusive,
            },
        );
        id
    }

    /// Declare that job will update a target.
    /// Creates an input and output pin for the job node.
    pub fn update<T>(&mut self) -> UpdateId<T>
    where
        T: Target,
    {
        let id = self.idgen.next();
        self.inputs.insert(
            id.cast(),
            Input {
                id: TypeId::of::<T>(),
                name: T::name(),
                access: Access::Exclusive,
            },
        );
        self.outputs.insert(
            id.cast(),
            Output {
                id: TypeId::of::<T>(),
                name: T::name(),
            },
        );
        id
    }

    /// Declare that job will read a target.
    /// Creates an input pin for the job node.
    ///
    /// This is only available for target types with mergeable info.
    pub fn read<T>(&mut self) -> InputId<T>
    where
        T: TargetInfoMerge,
    {
        let id = self.idgen.next();
        self.inputs.insert(
            id.cast(),
            Input {
                id: TypeId::of::<T>(),
                name: T::name(),
                access: Access::Shared,
            },
        );

        self.merge_info.insert(
            TypeId::of::<T>(),
            |info: &mut dyn Any, other: &dyn Any| unsafe {
                T::merge_info(
                    info.downcast_mut().unwrap_unchecked(),
                    other.downcast_ref().unwrap_unchecked(),
                )
            },
        );

        id
    }

    /// Called by the job setup to build the job node.
    /// This must be called exactly once.
    pub fn build(self, name: String, plan: impl PlanJob, run: impl RunJob) -> JobNode {
        JobNode {
            name,
            plan: Box::new(plan),
            run: Box::new(run),
            inputs: self.inputs,
            outputs: self.outputs,
        }
    }
}

/// Part of the Job API.
///
/// Implementation of this trait as inserted into the graph execution plan
/// and executed when the graph is executed.
pub trait PlanJob: 'static {
    /// Called each frame at planning stage.
    /// Jobs provide requirements and definitions of targets they will use.
    /// Job may provide definition only if it creates target.
    /// If definition don't have `Default` impl, job must provide definition.
    ///
    /// Job may provide requirements for targets it will use either way.
    fn plan(&mut self, planner: Planner<'_>);
}

impl<F> PlanJob for F
where
    F: FnMut(Planner<'_>) + 'static,
{
    fn plan(&mut self, planner: Planner<'_>) {
        self(planner)
    }
}

pub struct Planner<'a> {
    hub: &'a mut TargetHub,
    targets: &'a HashMap<PinId, TargetId>,
    pins: &'a HashMap<PinId, JobId>,
    edges: &'a HashMap<InputId, OutputId>,
    merge_info: &'a HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
    selected: &'a mut HashSet<JobId>,
    device: mev::Device,
}

impl<'a> Planner<'a> {
    pub(super) fn new(
        hub: &'a mut TargetHub,
        targets: &'a HashMap<PinId, TargetId>,
        pins: &'a HashMap<PinId, JobId>,
        edges: &'a HashMap<InputId, OutputId>,
        merge_info: &'a HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
        selected: &'a mut HashSet<JobId>,
        device: mev::Device,
    ) -> Self {
        Planner {
            hub,
            targets,
            pins,
            edges,
            merge_info,
            selected,
            device,
        }
    }
}

impl Planner<'_> {
    /// See the output target instance.
    ///
    /// If there's more than one reader, info is merged.
    ///
    /// If not required, returns `None`.
    pub fn output<T>(&mut self, id: OutputId<T>, name: &str) -> Option<&T::Info>
    where
        T: Target,
    {
        let target = *self.targets.get(&id.cast())?;
        self.hub.plan_output::<T>(target, name, &self.device)
    }

    /// Tell dependencies what this job needs.
    ///
    /// If there's more than one readers, info is merged.
    ///
    /// Does nothing if target is not connected.
    pub fn input<T>(&mut self, id: InputId<T>, info: T::Info)
    where
        T: Target,
    {
        let Some(&target) = self.targets.get(&id.cast()) else {
            return;
        };
        if let Some(&output) = self.edges.get(&id.cast()) {
            self.hub.plan_input::<T>(target, info, self.merge_info);
            let dep = self.pins[&output.cast()];
            self.selected.insert(dep);
        }
    }

    /// Specify that job's output resource is the same as input resource.
    /// In setup stage job must declare that it will consume the `from` target.
    pub fn update<T>(&mut self, id: UpdateId<T>)
    where
        T: Target,
    {
        if let Some(&output) = self.edges.get(&id.cast()) {
            let dep = self.pins[&id.cast()];
            self.selected.insert(dep);
        }
    }
}

/// Part of the Job API.
///
/// Implementation of this trait as inserted into the graph execution plan
/// and executed when the graph is executed.
pub trait RunJob: 'static {
    /// Called when graph is executed and targets of the job needs to be produced.
    fn run(&mut self, runner: Runner<'_>);
}

impl<F> RunJob for F
where
    F: FnMut(Runner<'_>) + 'static,
{
    fn run(&mut self, runner: Runner<'_>) {
        self(runner)
    }
}

pub struct Runner<'a> {
    hub: &'a mut TargetHub,
    targets: &'a HashMap<PinId, TargetId>,

    device: mev::Device,
    queue: RefCell<&'a mut mev::Queue>,

    /// Arena for command buffers.
    /// This allows taking references to newly allocated command encoders
    /// And after job is done, collecting them in allocated order.
    cbufs: &'a Arena<mev::CommandEncoder>,
}

impl<'a> Runner<'a> {
    pub(super) fn new(
        hub: &'a mut TargetHub,
        targets: &'a HashMap<PinId, TargetId>,
        device: mev::Device,
        queue: RefCell<&'a mut mev::Queue>,
        cbufs: &'a Arena<mev::CommandEncoder>,
    ) -> Self {
        Runner {
            targets,
            hub,
            device,
            queue,
            cbufs,
        }
    }
}

impl Runner<'_> {
    /// Get a target resource.
    pub fn input<T>(&self, id: InputId<T>, stages: mev::PipelineStages) -> Option<&T>
    where
        T: Target,
    {
        let target = *self.targets.get(&id.cast())?;
        self.hub.get(target)
    }

    /// Get a target resource.
    pub fn output<T>(&self, id: OutputId<T>, stages: mev::PipelineStages) -> Option<&T>
    where
        T: Target,
    {
        let target = *self.targets.get(&id.cast())?;
        self.hub.get(target)
    }

    /// Get a target resource.
    pub fn update<T>(&self, id: UpdateId<T>, stages: mev::PipelineStages) -> Option<&T>
    where
        T: Target,
    {
        let target = *self.targets.get(&id.cast())?;
        self.hub.get(target)
    }

    pub fn new_encoder(&self) -> &mut mev::CommandEncoder {
        let encoder = self.queue.borrow_mut().new_command_encoder().unwrap();
        self.cbufs.put(encoder)
    }

    pub fn device(&self) -> &mev::Device {
        &self.device
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy)]
pub struct Input {
    pub id: TypeId,
    pub name: &'static str,
    pub access: Access,
}

#[derive(Clone, Copy)]
pub struct Output {
    pub id: TypeId,
    pub name: &'static str,
}

pub struct JobNode {
    name: String,
    plan: Box<dyn PlanJob>,
    run: Box<dyn RunJob>,
    inputs: HashMap<InputId, Input>,
    outputs: HashMap<OutputId, Output>,
}

impl JobNode {
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn inputs(&self) -> impl ExactSizeIterator<Item = (InputId, Input)> + '_ {
        self.inputs.iter().map(|(&id, &input)| (id, input))
    }

    pub(super) fn outputs(&self) -> impl ExactSizeIterator<Item = (OutputId, Output)> + '_ {
        self.outputs.iter().map(|(&id, &output)| (id, output))
    }

    pub(super) fn plan(&mut self, planner: Planner<'_>) {
        self.plan.plan(planner)
    }

    pub(super) fn run(&mut self, runner: Runner<'_>) {
        self.run.run(runner)
    }
}
