use blink_alloc::Blink;
use edict::world::WorldLocal;
use hashbrown::{hash_map::Entry, HashMap, HashSet};

use crate::id::IdGen;

use super::{
    job::{
        JobCreateTarget, JobDesc, JobNode, JobReadTarget, JobUpdateDesc, JobUpdateTarget, Planner,
    },
    target::{Target, TargetHub, TargetId},
};

pub struct WorkGraph {
    // Constant state
    plan: Vec<JobNode>,
    job_order: HashMap<usize, usize>,
    // edges: HashSet<Edge>,

    // Mutable state
    hub: TargetHub,
    idgen: IdGen,
    sinks: HashSet<[usize; 2], TargetId>,

    // Temporary state
    selected_jobs: HashSet<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: [usize; 2],
    pub to: [usize; 2],
}

pub struct Cycle;

impl WorkGraph {
    /// Build work-graph from list of jobs descs and edges.
    pub fn new<'a>(jobs: HashMap<usize, JobNode>, edges: HashSet<Edge>) -> Result<Self, Cycle> {
        // Unfold graph into a queue.
        // This queue must have dependencies-first order.
        // If dependency cycle is detected, return error.

        let mut enqueued = HashSet::<usize>::new();
        let mut pending = HashSet::<usize>::new();
        let mut stack = jobs.keys().copied().collect::<Vec<_>>();
        let mut queue = Vec::new();

        while let Some(job_id) = stack.pop() {
            if enqueued.contains(&job_id) {
                continue;
            }

            let job = &jobs[&job_id];

            let mut deferred = false;

            for edge in edges.iter().filter(|e| e.to[0] == job_id) {
                let dep = edge.from[0];
                if enqueued.contains(&dep) {
                    continue;
                }
                if pending.contains(&job_id) {
                    panic!("Cyclic dependency detected");
                }
                if !deferred {
                    stack.push(job_id);
                    deferred = true;
                }
                stack.push(dep);
            }

            if !deferred {
                enqueued.insert(job_id);
                queue.push(job_id);
            }
        }

        // Assign target ids to job pins.

        let mut idgen = IdGen::new();

        let mut output_targets = HashMap::<[usize; 2], TargetId>::new();
        let mut input_targets = HashMap::<[usize; 2], TargetId>::new();

        for &job_id in queue.iter().rev() {
            let job = &mut jobs[&job_id];

            for edge in edges.iter().filter(|e| e.to[0] == job_id) {
                let to_pin = edge.to[1];

                // Check if output is already assigned to a target.
                // This is possible if multiple inputs reads from the same output.
                if let Some(&target) = output_targets.get(&edge.from) {
                    // Simply assigned to the same target.
                    input_targets.insert(edge.to, target);
                    continue;
                }

                match (to_pin < job.updates.len()).then(|| output_targets.get(&edge.to)) {
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
        let plan = Vec::new();

        let job_order = HashMap::<usize, usize>::new();

        for job_id in queue {
            let mut job = jobs.remove(&job_id).unwrap();

            for (idx, u) in job.updates.iter_mut().enumerate() {
                u.id = output_targets.get(&[job_id, idx]);
                assert_eq!(u.id, input_targets.get(&[job_id, idx]));
            }

            for (idx, c) in job.creates.iter_mut().enumerate() {
                let idx = idx + job.updates.len();
                c.id = output_targets.get(&[job_id, idx]);
            }

            for (idx, r) in job.reads.iter_mut().enumerate() {
                let idx = idx + job.updates.len() + job.creates.len();
                r.id = input_targets.get(&[job_id, idx]);
            }

            for edge in edges.iter().filter(|e| e.to[0] == job_id) {
                let to_pin = edge.to[1];

                if to_pin < job.updates.len() {
                    let update = &mut job.updates[to_pin];
                    update.dep = Some(edge.from[0]);
                } else {
                    let read = &mut job.reads[to_pin - job.updates.len() - job.creates.len()];
                    read.dep = Some(edge.from[0]);
                }
            }

            job_order.insert(job_id, plan.len());
            plan.push(job);
        }

        Ok(WorkGraph {
            plan,
            job_order,
            // edges,
            hub,
            idgen,
            sinks: HashSet::new(),
            selected_jobs: HashSet::new(),
        })
    }

    pub fn set_sink<T>(&mut self, job_id: usize, output: usize, target: T, info: T::Info)
    where
        T: Target,
    {
        let job = &mut self.plan[self.job_order[&job_id]];

        match self.sinks.entry([job_id, output]) {
            Entry::Occupied(entry) => {
                let id = *entry.get();

                if output < job.updates.len() {
                    assert_eq!(job.updates[output].id, Some(id));
                } else if output < job.updates.len() {
                    assert_eq!(job.creates[output - job.updates.len()].id, Some(id));
                }

                self.hub.external(id, target, info);
            }
            Entry::Vacant(entry) => {
                let id = self.idgen.next();

                if output < job.updates.len() {
                    assert_eq!(job.updates[output].id, None);
                    job.updates[output].id = Some(id);
                } else if output < job.updates.len() {
                    assert_eq!(job.creates[output - job.updates.len()].id, None);
                    job.creates[output - job.updates.len()].id = Some(id);
                }

                entry.insert(id);
                self.hub.external(id, target, info);
            }
        }
    }

    pub fn unset_sink<T>(&mut self, job_id: usize, output: usize)
    where
        T: Target,
    {
        let job = &mut self.plan[self.job_order[&job_id]];

        if let Some(id) = self.sinks.remove([job_id, output]) {
            self.hub.clear_external(id);

            if output < job.updates.len() {
                assert_eq!(job.updates[output].id, Some(id));
                job.updates[output].id = None;
            } else if output < job.updates.len() {
                assert_eq!(job.creates[output - job.updates.len()].id, Some(id));
                job.creates[output - job.updates.len()].id = None;
            }
        }
    }

    pub fn run(&mut self, world: &mut WorldLocal, device: mev::Device, queue: &mut mev::Queue) {
        self.selected_jobs.clear();

        for &[job_id, _] in &self.sinks {
            self.selected_jobs.insert(job_id);
        }

        // Plan in reverse order.
        for (job_id, job) in self.jobs.iter_mut().enumerate().rev() {
            if !self.selected_jobs.contains(&job_id) {
                continue;
            }

            let mut planner = Planner::new(
                &job.updates,
                &job.creates,
                &job.reads,
                &mut self.hub,
                &mut self.selected_jobs,
                &device,
            );

            job.plan(planner);
        }
    }
}
