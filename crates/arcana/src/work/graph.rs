use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    hash::Hash,
};

use arcana_names::Name;
use edict::world::World;
use hashbrown::{hash_map::Entry, HashMap, HashSet};
use slab::Slab;

use crate::{
    arena::Arena, id::SeqIdGen, model::Value, plugin::PluginsHub, work::job::invalid_output_pin,
    Stid,
};

use super::{
    job::{JobDesc, JobId},
    target::{Target, TargetHub, TargetId},
};

/// Index of a job in work graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct JobIdx(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HookId {
    pub hook: usize,
    pub pin: PinId,
}

pub struct WorkGraph {
    // Constant state
    plan: Vec<JobNode>,
    idx_to_order: HashMap<JobIdx, usize>,

    // Mutable state
    hub: TargetHub,
    idgen: SeqIdGen,
    sinks: HashMap<PinId, TargetId>,

    // Temporary state
    // Cleared after each run.
    selected_jobs: HashSet<JobIdx>,
    cbufs: Arena<mev::CommandEncoder>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PinId {
    pub job: JobIdx,
    pub pin: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: PinId,
    pub to: PinId,
}

#[derive(Debug)]
pub struct Cycle;

impl WorkGraph {
    /// Build work-graph from list of jobs and edges.
    pub fn new(
        mut jobs: HashMap<JobIdx, (JobId, JobDesc, HashMap<Name, Value>)>,
        edges: HashSet<Edge>,
    ) -> Result<Self, Cycle> {
        // Unfold graph into a queue.
        // This queue must have dependencies-first order.
        // If dependency cycle is detected, return error.

        let mut enqueued = HashSet::<JobIdx>::new();
        let mut pending = HashSet::<JobIdx>::new();
        let mut stack = jobs.keys().copied().collect::<Vec<_>>();
        let mut queue = Vec::new();

        while let Some(job_idx) = stack.pop() {
            if enqueued.contains(&job_idx) {
                continue;
            }

            let mut deferred = false;

            for edge in edges.iter().filter(|e| e.to.job == job_idx) {
                let dep_idx = edge.from.job;
                if enqueued.contains(&dep_idx) {
                    continue;
                }
                if pending.contains(&dep_idx) {
                    return Err(Cycle);
                }
                if !deferred {
                    pending.insert(job_idx);
                    stack.push(job_idx);
                    deferred = true;
                }
                stack.push(dep_idx);
            }

            if !deferred {
                enqueued.insert(job_idx);
                queue.push(job_idx);
            }
        }

        // Assign target ids to job pins.

        let mut idgen = SeqIdGen::new();

        let mut output_targets = HashMap::<PinId, TargetId>::new();
        let mut input_targets = HashMap::<PinId, TargetId>::new();

        // Iterate over jobs in reverse order.
        // Process inputs instead of outputs
        // so unused outputs are not assigned to targets.
        for &job_idx in queue.iter().rev() {
            let (_, job_desc, _) = &jobs[&job_idx];

            for edge in edges.iter().filter(|e| e.to.job == job_idx) {
                let to_pin = edge.to.pin;

                // Check if output is already assigned to a target.
                // This is possible if multiple inputs reads from the same output.
                if let Some(&target) = output_targets.get(&edge.from) {
                    // Simply assigned to the same target.
                    input_targets.insert(edge.to, target);
                    continue;
                }

                match job_desc
                    .update_idx(to_pin)
                    .and_then(|_| output_targets.get(&edge.to))
                {
                    Some(&target) => {
                        // Target already assigned to matching output of this update pin.
                        output_targets.insert(edge.from, target);
                        input_targets.insert(edge.to, target);
                    }
                    None => {
                        // Allocate new target to assign to this edge pins.
                        let target = TargetId::generate(&mut idgen);
                        output_targets.insert(edge.from, target);
                        input_targets.insert(edge.to, target);
                    }
                }
            }
        }

        // Construct execution plan.
        let mut idx_to_order = HashMap::new();
        let mut plan = Vec::new();

        for job_idx in queue {
            let (job_id, job_desc, job_cfg) = jobs.remove(&job_idx).unwrap();
            let mut job: JobNode = JobNode::new(job_idx, job_id, job_desc, job_cfg);

            for (idx, u) in job.updates.iter_mut().enumerate() {
                let pin = PinId {
                    job: job_idx,
                    pin: idx,
                };

                let out_id = output_targets.get(&pin).copied();
                let in_id = input_targets.get(&pin).copied();

                // Assign either input or output target id to update pin.
                match (out_id, in_id) {
                    (Some(out_id), Some(in_id)) => {
                        // When update is connected on both sides,
                        // it must be assigned to the same target.
                        assert_eq!(out_id, in_id);
                        u.id = Some(out_id);
                    }
                    (Some(out_id), None) => {
                        u.id = Some(out_id);
                    }
                    (None, Some(in_id)) => {
                        u.id = Some(in_id);
                    }
                    (None, None) => {}
                }
            }

            for (idx, c) in job.creates.iter_mut().enumerate() {
                let idx = idx + job.updates.len();
                c.id = output_targets
                    .get(&PinId {
                        job: job_idx,
                        pin: idx,
                    })
                    .copied();
            }

            for (idx, r) in job.reads.iter_mut().enumerate() {
                let idx = idx + job.updates.len() + job.creates.len();
                r.id = input_targets
                    .get(&PinId {
                        job: job_idx,
                        pin: idx,
                    })
                    .copied();
            }

            // Collect dependencies for each job.
            for edge in edges.iter().filter(|e| e.to.job == job_idx) {
                let to_pin = edge.to.pin;

                if to_pin < job.updates.len() {
                    let update = &mut job.updates[to_pin];
                    update.dep_idx = Some(edge.from.job);
                } else {
                    let read = &mut job.reads[to_pin - job.updates.len() - job.creates.len()];
                    read.dep_idx = Some(edge.from.job);
                }
            }

            idx_to_order.insert(job_idx, plan.len());
            plan.push(job);
        }

        Ok(WorkGraph {
            plan,
            idx_to_order,
            // edges,
            hub: TargetHub::new(),
            idgen,
            sinks: HashMap::new(),
            selected_jobs: HashSet::new(),
            cbufs: Arena::new(),
        })
    }

    pub fn set_sink<T>(&mut self, pin: PinId, target: T, info: T::Info)
    where
        T: Target,
    {
        let order = self.idx_to_order[&pin.job];
        let job = &mut self.plan[order];

        match self.sinks.entry(pin) {
            Entry::Occupied(entry) => {
                let target_id = *entry.get();

                match (job.update_idx(pin.pin), job.create_idx(pin.pin)) {
                    (Some(idx), None) => {
                        assert_eq!(job.updates[idx].id, Some(target_id));
                    }
                    (None, Some(idx)) => {
                        assert_eq!(job.creates[idx].id, Some(target_id));
                    }
                    _ => invalid_output_pin(pin.pin),
                }

                self.hub.external(target_id, target, info);
            }
            Entry::Vacant(entry) => {
                let mut target_id = TargetId::generate(&mut self.idgen);

                match (job.update_idx(pin.pin), job.create_idx(pin.pin)) {
                    (Some(idx), None) => match job.updates[idx].id {
                        None => {
                            job.updates[idx].id = Some(target_id);
                        }
                        Some(id) => target_id = id,
                    },
                    (None, Some(idx)) => match job.creates[idx].id {
                        None => {
                            job.creates[idx].id = Some(target_id);
                        }
                        Some(id) => target_id = id,
                    },
                    _ => {
                        invalid_output_pin(pin.pin);
                    }
                }

                entry.insert(target_id);
                self.hub.external(target_id, target, info);
            }
        }
    }

    pub fn unset_sink<T>(&mut self, pin: PinId)
    where
        T: Target,
    {
        let order = self.idx_to_order[&pin.job];
        let job = &mut self.plan[order];

        assert!(pin.pin < job.creates.len() + job.updates.len());

        if let Some(id) = self.sinks.remove(&pin) {
            self.hub.clear_external::<T>(id);

            match (job.update_idx(pin.pin), job.create_idx(pin.pin)) {
                (Some(idx), None) => {
                    assert_eq!(job.updates[idx].id, Some(id));
                    job.updates[idx].id = None;
                }
                (None, Some(idx)) => {
                    assert_eq!(job.creates[idx].id, Some(id));
                    job.creates[idx].id = None;
                }
                _ => invalid_output_pin(pin.pin),
            }
        }
    }

    pub fn add_hook<T>(
        &mut self,
        pin: PinId,
        mut hook: impl FnMut(&T, &mev::Device, &CommandStream) + 'static,
    ) -> HookId
    where
        T: Target,
    {
        let order = self.idx_to_order[&pin.job];
        let job = &mut self.plan[order];

        let target_id = match (job.update_idx(pin.pin), job.create_idx(pin.pin)) {
            (Some(idx), None) => job.updates[idx].id,
            (None, Some(idx)) => job.creates[idx].id,
            _ => invalid_output_pin(pin.pin),
        };

        let idx = job.hooks.insert(Box::new(move |hub, device, commands| {
            if let Some(target_id) = target_id {
                if let Some(target) = hub.get(target_id) {
                    hook(target, device, commands);
                }
            }
        }));
        HookId { hook: idx, pin }
    }

    pub fn has_hook(&self, id: HookId) -> bool {
        let order = self.idx_to_order[&id.pin.job];
        let job = &self.plan[order];
        job.hooks.contains(id.hook)
    }

    pub fn remove_hook(&mut self, id: HookId) {
        let order = self.idx_to_order[&id.pin.job];
        let job = &mut self.plan[order];
        let _ = job.hooks.try_remove(id.hook);
    }

    pub fn run(
        &mut self,
        queue: &mut mev::Queue,
        world: &mut World,
        hub: &mut PluginsHub,
    ) -> Result<(), mev::DeviceError> {
        self.selected_jobs.clear();

        for (&PinId { job, .. }, _) in &self.sinks {
            self.selected_jobs.insert(job);
        }

        // Plan in reverse order.
        // This allows to collect all target descriptors before creating them.
        // And select dependencies for execution before planning loop considers them.
        for job in self.plan.iter_mut().rev() {
            if !self.selected_jobs.contains(&job.idx) {
                continue;
            }
            job.plan(
                &mut self.hub,
                &mut self.selected_jobs,
                queue.device().clone(),
                world,
                hub,
            );
        }

        for job in self.plan.iter_mut() {
            if !self.selected_jobs.contains(&job.idx) {
                continue;
            }
            job.exec(&mut self.hub, queue, &self.cbufs, world, hub);
        }

        queue.submit(self.cbufs.drain().filter_map(|e| e.finish().ok()), true)
    }
}

pub struct Planner<'a> {
    /// List of targets updates of the job correspond to.
    updates: std::slice::Iter<'a, TargetUpdate>,

    /// List of targets creates of the job correspond to.
    creates: std::slice::Iter<'a, TargetCreate>,

    /// List of targets reads of the job correspond to.
    reads: std::slice::Iter<'a, TargetRead>,

    /// Where all targets live.
    hub: &'a mut TargetHub,

    /// Set of selected jobs.
    selected_jobs: &'a mut HashSet<JobIdx>,

    device: mev::Device,

    idx: JobIdx,

    params: &'a HashMap<Name, Value>,
}

impl Planner<'_> {
    /// Provide resource description for next input.
    /// Allows merging resource description other readers.
    pub fn create<T>(&mut self) -> Option<&T::Info>
    where
        T: Target,
    {
        let create = self.creates.next().expect("No more creates");
        assert_eq!(create.ty, Stid::of::<T>());
        self.hub
            .plan_create::<T>(create.id?, &create.name, &self.device)
    }

    /// Fetcehs resource description for next update.
    pub fn update<T>(&mut self) -> Option<&T::Info>
    where
        T: Target,
    {
        let update = self.updates.next().expect("No more updates");
        assert_eq!(update.ty, Stid::of::<T>());
        let info = self.hub.plan_update::<T>(update.id?)?;

        if let Some(dep_idx) = update.dep_idx {
            self.selected_jobs.insert(dep_idx);
        }

        Some(info)
    }

    /// Provide resource description for next input.
    /// Allows merging resource description other readers.
    pub fn read<T>(&mut self, info: T::Info)
    where
        T: Target,
    {
        let read = self.reads.next().expect("No more reads");
        assert_eq!(read.ty, Stid::of::<T>());
        let Some(id) = read.id else {
            return;
        };
        self.hub.plan_read::<T>(id, info);

        if let Some(dep_idx) = read.dep_idx {
            self.selected_jobs.insert(dep_idx);
        }
    }

    pub fn idx(&self) -> JobIdx {
        self.idx
    }

    pub fn param<Q>(&self, name: &Q) -> &Value
    where
        Name: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        &self.params[name]
    }
}

pub struct CommandStream<'a> {
    queue: RefCell<&'a mut mev::Queue>,
    cbufs: &'a Arena<mev::CommandEncoder>,
}

impl CommandStream<'_> {
    /// Allocates new command encoder.
    /// It will be automatically submitted to this job's queue.
    ///
    /// Returned reference is bound to this `Exec`'s borrow,
    /// so make sure to fetch target references before calling this.
    pub fn new_encoder(&self) -> &mut mev::CommandEncoder {
        let encoder = self.queue.borrow_mut().new_command_encoder().unwrap();
        self.cbufs.put(encoder)
    }
}

pub struct Exec<'a> {
    /// List of targets updates of the job correspond to.
    updates: &'a [TargetUpdate],
    next_update: Cell<usize>,

    /// List of targets creates of the job correspond to.
    creates: &'a [TargetCreate],
    next_create: Cell<usize>,

    /// List of targets reads of the job correspond to.
    reads: &'a [TargetRead],
    next_read: Cell<usize>,

    /// Where all targets live.
    hub: &'a mut TargetHub,

    device: mev::Device,

    /// Arena for command buffers.
    /// This allows taking references to newly allocated command encoders
    /// And after job is done, collecting them in allocated order.
    commands: CommandStream<'a>,

    idx: JobIdx,

    params: &'a HashMap<Name, Value>,
}

impl Exec<'_> {
    /// Fetches next resource to update.
    ///
    /// Returns none if not connected to next input.
    pub fn update<T>(&self) -> Option<&T>
    where
        T: Target,
    {
        let idx = self.next_update.get();
        let update = self.updates.get(idx).expect("No more updates");
        self.next_update.set(idx + 1);
        self.hub.get::<T>(update.id?)
    }

    /// Fetches next resource to create.
    ///
    /// Returns none if not connected.
    pub fn create<T>(&self) -> Option<&T>
    where
        T: Target,
    {
        let idx = self.next_create.get();
        let create = self.creates.get(idx).expect("No more creates");
        self.next_create.set(idx + 1);
        self.hub.get::<T>(create.id?)
    }

    /// Fetches next resource to read.
    ///
    /// Returns none if not connected.
    pub fn read<T>(&self) -> Option<&T>
    where
        T: Target,
    {
        let idx = self.next_read.get();
        let read = self.reads.get(idx).expect("No more reads");
        self.next_read.set(idx + 1);
        self.hub.get::<T>(read.id?)
    }

    /// Allocates new command encoder.
    /// It will be automatically submitted to this job's queue.
    ///
    /// Returned reference is bound to this `Exec`'s borrow,
    /// so make sure to fetch target references before calling this.
    pub fn new_encoder(&self) -> &mut mev::CommandEncoder {
        self.commands.new_encoder()
    }

    /// Returns reference to device.
    pub fn device(&self) -> &mev::Device {
        &self.device
    }

    pub fn idx(&self) -> JobIdx {
        self.idx
    }

    pub fn param<Q>(&self, name: &Q) -> &Value
    where
        Name: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        &self.params[name]
    }
}

#[derive(Debug)]
struct TargetCreate {
    /// Target name.
    name: Name,

    /// Target type.
    ty: Stid,

    /// Assigned target id.
    id: Option<TargetId>,
}

#[derive(Debug)]
struct TargetUpdate {
    /// Target type.
    ty: Stid,

    /// Assigned target id.
    id: Option<TargetId>,

    /// Job index that outputs this target.
    dep_idx: Option<JobIdx>,
}

#[derive(Debug)]
struct TargetRead {
    /// Target type.
    ty: Stid,

    /// Assigned target id.
    id: Option<TargetId>,

    /// Job index that outputs this target.
    dep_idx: Option<JobIdx>,
}

// #[derive(Debug)]
struct JobNode {
    idx: JobIdx,
    id: JobId,
    params: HashMap<Name, Value>,
    updates: Vec<TargetUpdate>,
    creates: Vec<TargetCreate>,
    reads: Vec<TargetRead>,
    hooks: Slab<Box<dyn FnMut(&TargetHub, &mev::Device, &CommandStream)>>,
}

impl JobNode {
    /// Construct new job node from description and job instance.
    fn new(idx: JobIdx, id: JobId, desc: JobDesc, params: HashMap<Name, Value>) -> Self {
        JobNode {
            idx,
            id,
            params,
            updates: desc
                .updates
                .into_iter()
                .map(|u| TargetUpdate {
                    ty: u.ty,
                    id: None,
                    dep_idx: None,
                })
                .collect(),
            creates: desc
                .creates
                .into_iter()
                .map(|c| TargetCreate {
                    ty: c.ty,
                    name: c.name,
                    id: None,
                })
                .collect(),
            reads: desc
                .reads
                .into_iter()
                .map(|c| TargetRead {
                    ty: c.ty,
                    id: None,
                    dep_idx: None,
                })
                .collect(),
            hooks: Slab::new(),
        }
    }

    fn plan(
        &mut self,
        hub: &mut TargetHub,
        selected_jobs: &mut HashSet<JobIdx>,
        device: mev::Device,
        world: &mut World,
        plugins: &mut PluginsHub,
    ) {
        let planner = Planner {
            updates: self.updates.iter(),
            creates: self.creates.iter(),
            reads: self.reads.iter(),
            hub,
            selected_jobs,
            device,
            idx: self.idx,
            params: &self.params,
        };

        if let Some(job) = plugins.jobs.get_mut(&self.id) {
            job.plan(planner, world);
        }
    }

    fn exec(
        &mut self,
        hub: &mut TargetHub,
        queue: &mut mev::Queue,
        cbufs: &Arena<mev::CommandEncoder>,
        world: &mut World,
        plugins: &mut PluginsHub,
    ) {
        let device = queue.device().clone();

        let commands = CommandStream {
            queue: RefCell::new(queue),
            cbufs,
        };

        let exec = Exec {
            updates: &self.updates,
            next_update: Cell::new(0),
            creates: &self.creates,
            next_create: Cell::new(0),
            reads: &self.reads,
            next_read: Cell::new(0),
            hub,
            device: device.clone(),
            commands,
            idx: self.idx,
            params: &self.params,
        };

        if let Some(job) = plugins.jobs.get_mut(&self.id) {
            job.exec(exec, world);
        }

        let commands = CommandStream {
            queue: RefCell::new(queue),
            cbufs,
        };

        for (_, hook) in self.hooks.iter_mut() {
            hook(hub, &device, &commands);
        }
    }

    // fn update_idx(&self, pin: usize) -> Option<usize> {
    //     if pin < self.updates.len() {
    //         Some(pin)
    //     } else {
    //         None
    //     }
    // }

    // fn read_idx(&self, pin: usize) -> Option<usize> {
    //     if pin >= self.updates.len() && pin < self.updates.len() + self.reads.len() {
    //         Some(pin - self.updates.len())
    //     } else {
    //         None
    //     }
    // }

    fn update_idx(&self, pin: usize) -> Option<usize> {
        if pin < self.updates.len() {
            Some(pin)
        } else {
            None
        }
    }

    fn create_idx(&self, pin: usize) -> Option<usize> {
        if pin >= self.updates.len() && pin < self.creates.len() + self.updates.len() {
            Some(pin - self.updates.len())
        } else {
            None
        }
    }
}
