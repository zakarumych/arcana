use edict::{world::WorldLocal, Res};
use hashbrown::{hash_map::Entry, HashMap, HashSet};

use crate::{arena::Arena, id::IdGen, plugin::PluginsHub};

use super::{
    job::{JobId, JobNode},
    target::{Target, TargetHub, TargetId},
};

pub struct WorkGraph {
    // Constant state
    plan: Vec<(JobId, JobNode)>,
    job_order: HashMap<JobId, usize>,
    // edges: HashSet<Edge>,

    // Mutable state
    hub: TargetHub,
    idgen: IdGen,
    sinks: HashMap<PinId, TargetId>,

    // Temporary state
    selected_jobs: HashSet<JobId>,

    cbufs: Arena<mev::CommandEncoder>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PinId {
    pub job: JobId,
    pub idx: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: PinId,
    pub to: PinId,
}

#[derive(Debug)]
pub struct Cycle;

impl WorkGraph {
    /// Build work-graph from list of jobs descs and edges.
    pub fn new<'a>(mut jobs: HashMap<JobId, JobNode>, edges: HashSet<Edge>) -> Result<Self, Cycle> {
        // Unfold graph into a queue.
        // This queue must have dependencies-first order.
        // If dependency cycle is detected, return error.

        let mut enqueued = HashSet::<JobId>::new();
        let mut pending = HashSet::<JobId>::new();
        let mut stack = jobs.keys().copied().collect::<Vec<_>>();
        let mut queue = Vec::new();

        while let Some(job_id) = stack.pop() {
            if enqueued.contains(&job_id) {
                continue;
            }

            let mut deferred = false;

            for edge in edges.iter().filter(|e| e.to.job == job_id) {
                let dep_idx = edge.from.job;
                if enqueued.contains(&dep_idx) {
                    continue;
                }
                if pending.contains(&dep_idx) {
                    panic!("Cyclic dependency detected");
                }
                if !deferred {
                    pending.insert(job_id);
                    stack.push(job_id);
                    deferred = true;
                }
                stack.push(dep_idx);
            }

            if !deferred {
                enqueued.insert(job_id);
                queue.push(job_id);
            }
        }

        // Assign target ids to job pins.

        let mut idgen = IdGen::new();

        let mut output_targets = HashMap::<PinId, TargetId>::new();
        let mut input_targets = HashMap::<PinId, TargetId>::new();

        for &job_id in queue.iter().rev() {
            let job = jobs.get_mut(&job_id).unwrap();

            for edge in edges.iter().filter(|e| e.to.job == job_id) {
                let to_pin = edge.to.idx;

                // Check if output is already assigned to a target.
                // This is possible if multiple inputs reads from the same output.
                if let Some(&target) = output_targets.get(&edge.from) {
                    // Simply assigned to the same target.
                    input_targets.insert(edge.to, target);
                    continue;
                }

                match (to_pin < job.updates.len())
                    .then(|| output_targets.get(&edge.to))
                    .flatten()
                {
                    Some(&target) => {
                        // Target already assigned to matching output of this update pin.
                        output_targets.insert(edge.from, target);
                        input_targets.insert(edge.to, target);
                    }
                    None => {
                        // Allocate new target to assign to this edge pins.
                        let target = idgen.next();
                        output_targets.insert(edge.from, target);
                        input_targets.insert(edge.to, target);
                    }
                }
            }
        }

        let mut hub = TargetHub::new();
        let mut plan = Vec::new();

        let mut job_order = HashMap::<JobId, usize>::new();

        for job_id in queue {
            let job_idx = plan.len();
            let mut job = jobs.remove(&job_id).unwrap();

            for (idx, u) in job.updates.iter_mut().enumerate() {
                let pin = PinId { job: job_id, idx };

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
                c.id = output_targets.get(&PinId { job: job_id, idx }).copied();
            }

            for (idx, r) in job.reads.iter_mut().enumerate() {
                let idx = idx + job.updates.len() + job.creates.len();
                r.id = input_targets.get(&PinId { job: job_id, idx }).copied();
            }

            // Collect dependencies for each job.
            for edge in edges.iter().filter(|e| e.to.job == job_id) {
                let to_pin = edge.to.idx;

                if to_pin < job.updates.len() {
                    let update = &mut job.updates[to_pin];
                    update.dep_id = Some(edge.from.job);
                } else {
                    let read = &mut job.reads[to_pin - job.updates.len() - job.creates.len()];
                    read.dep_id = Some(edge.from.job);
                }
            }

            job_order.insert(job_id, job_idx);
            plan.push((job_id, job));
        }

        Ok(WorkGraph {
            plan,
            job_order,
            // edges,
            hub,
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
        let job_idx = self.job_order[&pin.job];
        let job = &mut self.plan[job_idx].1;

        assert!(pin.idx < job.creates.len() + job.updates.len());

        match self.sinks.entry(pin) {
            Entry::Occupied(entry) => {
                let id = *entry.get();

                if pin.idx < job.creates.len() {
                    assert_eq!(job.creates[pin.idx].id, Some(id));
                } else {
                    assert_eq!(job.updates[pin.idx - job.creates.len()].id, Some(id));
                }

                self.hub.external(id, target, info);
            }
            Entry::Vacant(entry) => {
                let id = self.idgen.next();

                if pin.idx < job.creates.len() {
                    assert_eq!(job.creates[pin.idx].id, None);
                    job.creates[pin.idx].id = Some(id);
                } else {
                    assert_eq!(job.updates[pin.idx - job.creates.len()].id, None);
                    job.updates[pin.idx - job.creates.len()].id = Some(id);
                }

                entry.insert(id);
                self.hub.external(id, target, info);
            }
        }
    }

    pub fn unset_sink<T>(&mut self, pin: PinId)
    where
        T: Target,
    {
        let job_idx = self.job_order[&pin.job];
        let job = &mut self.plan[job_idx].1;

        assert!(pin.idx < job.creates.len() + job.updates.len());

        if let Some(id) = self.sinks.remove(&pin) {
            self.hub.clear_external::<T>(id);

            if pin.idx < job.creates.len() {
                assert_eq!(job.creates[pin.idx].id, Some(id));
                job.creates[pin.idx].id = None;
            } else {
                assert_eq!(job.updates[pin.idx - job.creates.len()].id, Some(id));
                job.updates[pin.idx - job.creates.len()].id = None;
            }
        }
    }

    pub fn run(
        &mut self,
        device: mev::Device,
        queue: &mut mev::Queue,
        world: &mut WorldLocal,
        hub: &mut PluginsHub,
    ) -> Result<(), mev::DeviceError> {
        self.selected_jobs.clear();

        for (&PinId { job, .. }, _) in &self.sinks {
            self.selected_jobs.insert(job);
        }

        // Plan in reverse order.
        // This allows to collect all target descriptors before creating them.
        // And select dependencies for execution before planning loop considers them.
        for (job_id, job) in self.plan.iter_mut().rev() {
            if !self.selected_jobs.contains(job_id) {
                continue;
            }
            job.plan(
                &mut self.hub,
                &mut self.selected_jobs,
                device.clone(),
                world,
                hub,
            );
        }

        for (job_id, job) in self.plan.iter_mut() {
            if !self.selected_jobs.contains(job_id) {
                continue;
            }
            job.exec(
                &mut self.hub,
                device.clone(),
                queue,
                &self.cbufs,
                world,
                hub,
            );
        }

        queue.submit(self.cbufs.drain().filter_map(|e| e.finish().ok()), true)
    }
}
