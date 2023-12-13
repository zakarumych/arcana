use std::{
    any::{Any, TypeId},
    cell::RefCell,
    marker::PhantomData,
    mem::ManuallyDrop,
};

use blink_alloc::Blink;
use edict::{world::WorldLocal, EntityId};
use hashbrown::{
    hash_map::{Entry, HashMap},
    HashSet,
};
use slab::Slab;

use crate::{
    arena::Arena,
    id::{Id, IdGen},
    work::job::Access,
};

use super::{
    job::{Input, Job, JobId, JobNode, Output, Planner, Runner, Setup},
    target::{InputId, OutputId, Target, TargetHub, TargetId},
    Image2D,
};

pub enum Pin {}
pub type PinId = Id<Pin>;

/// Graph of work nodes connected through their inputs and outputs.
pub struct WorkGraph {
    nodes: HashMap<JobId, JobNode>,
    pins: HashMap<PinId, JobId>,
    edges: HashMap<InputId, OutputId>,
    hub: TargetHub,
    idgen: IdGen,
    merge_info: HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
    sinks: HashSet<OutputId>,
    presents: HashMap<EntityId, OutputId>,
    main_present: Option<OutputId>,

    queue: Vec<JobId>,
    selected: HashSet<JobId>,
    targets: HashMap<PinId, TargetId>,

    cbufs: Arena<mev::CommandEncoder>,
}

impl WorkGraph {
    pub fn new() -> Self {
        WorkGraph {
            nodes: HashMap::new(),
            pins: HashMap::new(),
            edges: HashMap::new(),
            hub: TargetHub::new(),
            idgen: IdGen::new(),
            merge_info: HashMap::new(),
            sinks: HashSet::new(),
            presents: HashMap::new(),
            main_present: None,

            queue: Vec::new(),
            selected: HashSet::new(),
            targets: HashMap::new(),

            cbufs: Arena::new(),
        }
    }

    pub fn insert(&mut self, job: impl Job) -> JobId {
        let node = job.setup(Setup::new(&mut self.idgen, &mut self.merge_info));
        let job = self.idgen.next();
        for (id, _) in node.inputs() {
            self.pins.insert(id.cast(), job);
        }
        for (id, _) in node.outputs() {
            self.pins.insert(id.cast(), job);
        }
        self.nodes.insert(job, node);
        self.prepare();
        job
    }

    pub fn remove(&mut self, job: JobId) {
        let Some(node) = self.nodes.remove(&job) else {
            return;
        };
        for (input, _) in node.inputs() {
            self.pins.remove(&input.cast());
            self.edges.remove(&input);
        }
        for (output, _) in node.outputs() {
            self.pins.remove(&output.cast());
            self.edges.retain(|_, from| *from != output);
        }
        self.prepare();
    }

    pub fn name(&self, job: JobId) -> &str {
        self.nodes[&job].name()
    }

    pub fn inputs(&self, job: JobId) -> impl ExactSizeIterator<Item = (InputId, Input)> + '_ {
        self.nodes[&job].inputs()
    }

    pub fn outputs(&self, job: JobId) -> impl ExactSizeIterator<Item = (OutputId, Output)> + '_ {
        self.nodes[&job].outputs()
    }

    pub fn input_target(&self, id: InputId) -> Option<TargetId> {
        self.targets.get(&id.cast()).copied()
    }

    pub fn output_target(&self, id: OutputId) -> Option<TargetId> {
        self.targets.get(&id.cast()).copied()
    }

    pub fn connect(&mut self, from: OutputId, to: InputId) {
        assert!(!self.sinks.contains(&from));

        let to_job = self.pins[&to.cast()];
        let input = self.nodes[&to_job]
            .inputs()
            .find(|(id, _)| *id == to)
            .unwrap()
            .1;

        if input.access == Access::Exclusive {
            assert!(self.edges.iter().find(|(_, id)| **id == from).is_none());
        }

        self.edges.insert(to, from);
        self.prepare();
    }

    pub fn disconnect(&mut self, from: OutputId, to: InputId) {
        match self.edges.entry(to) {
            Entry::Occupied(mut entry) => {
                if *entry.get() == from {
                    entry.remove();
                }
            }
            Entry::Vacant(_) => {}
        }
        self.prepare();
    }

    pub fn sink<T: Target>(&mut self, id: OutputId, instance: T, info: T::Info) {
        self.sinks.insert(id);

        assert!(self.edges.iter().find(|(_, from)| **from == id).is_none());
        self.prepare();

        self.hub.external(id.cast(), instance, info);
    }

    pub fn present<T: Target>(&mut self, id: OutputId, viewport: EntityId) {
        self.presents.insert(viewport, id);
    }

    pub fn main_present<T: Target>(&mut self, id: OutputId) {
        self.main_present = Some(id);
    }

    /// Walk the graph and build the work queue and targets.
    fn prepare(&mut self) {
        // Clear queue and targets.
        self.queue.clear();
        self.targets.clear();
        self.hub.clear();

        // Build job queue, ensuring that all dependencies are enqueued before their dependents.
        let mut enqueued = HashSet::<JobId>::new();
        let mut stack = self.nodes.keys().copied().collect::<Vec<_>>();

        while let Some(job) = stack.pop() {
            if enqueued.contains(&job) {
                continue;
            }

            let mut deferred = false;
            for (input, _) in self.nodes[&job].inputs() {
                let output = self.edges[&input];
                let dep = self.pins[&output.cast()];

                if enqueued.contains(&dep) {
                    continue;
                }

                if !deferred {
                    stack.push(job);
                    deferred = true;
                }

                stack.push(dep);
            }

            if !deferred {
                self.queue.push(job);
                enqueued.insert(job);
            }
        }

        // Build targets.
        // Connect outputs and inputs and propagate through updates.

        // Start with sinks
        for sink in &self.sinks {
            let target = self.idgen.next();
            self.targets.insert(sink.cast(), target);
        }

        // Start with last jobs in queue.
        for &job in self.queue.iter().rev() {
            let node = &self.nodes[&job];

            for (to, _) in node.inputs() {
                // Connected input must be assigned to a target.
                let Some(&from) = self.edges.get(&to.cast()) else {
                    continue;
                };

                // Check if output is already assigned to a target.
                // This is possible if multiple inputs reads from the same output.
                if let Some(&target) = self.targets.get(&from.cast()) {
                    // Simply assigned to the same target.
                    self.targets.insert(to.cast(), target);
                    continue;
                }

                match self.targets.get(&to.cast()) {
                    None => {
                        // Allocate new target.
                        let target = self.idgen.next();

                        // Assign output and input to target.
                        self.targets.insert(to.cast(), target);
                        self.targets.insert(from.cast(), target);
                    }
                    Some(&target) => {
                        // If already assigned - it is update pin with matching output that was assigned.

                        // Assign output to target.
                        self.targets.insert(from.cast(), target);
                    }
                }
            }
        }
    }

    pub fn run(
        &mut self,
        world: &mut WorldLocal,
        blink: &Blink,
        device: mev::Device,
        queue: &mut mev::Queue,
    ) {
        // Select jobs that produce targets that are used by sinks.
        self.selected.clear();
        for &sink in &self.sinks {
            let job = self.pins[&sink.cast()];
            self.selected.insert(job);
        }

        // Plan in reverse order.
        for &job in self.queue.iter().rev() {
            if !self.selected.contains(&job) {
                continue;
            }

            let node = &mut self.nodes.get_mut(&job).unwrap();
            let mut planner = Planner::new(
                &mut self.hub,
                &self.targets,
                &self.pins,
                &self.edges,
                &self.merge_info,
                &mut self.selected,
                device.clone(),
            );

            node.plan(planner);
        }

        for &job in self.queue.iter() {
            if !self.selected.contains(&job) {
                continue;
            }

            let node = &mut self.nodes.get_mut(&job).unwrap();
            let mut runner = Runner::new(
                &mut self.hub,
                &self.targets,
                device.clone(),
                RefCell::new(&mut *queue),
                &mut self.cbufs,
            );

            node.run(runner);
        }
    }
}
