use std::{borrow::Cow, cell::RefCell, slice::Iter};

use arcana_project::{Ident, IdentBuf};
use hashbrown::{HashMap, HashSet};

use crate::{arena::Arena, make_id, Stid};

use super::{
    graph::Edge,
    target::{Target, TargetHub, TargetId},
};

/// Descroption of job creating a target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobCreateDesc {
    /// Target name.
    pub name: String,

    /// Target kind.
    pub kind: Stid,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobUpdateDesc {
    /// Target kind.
    pub kind: Stid,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobReadDesc {
    /// Target kind.
    pub kind: Stid,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobDesc {
    pub updates: Vec<JobUpdateDesc>,
    pub creates: Vec<JobCreateDesc>,
    pub reads: Vec<JobReadDesc>,
}

pub enum Input {
    Update,
    Read,
}

pub enum Output {
    Update,
    Create,
}

impl JobDesc {
    pub fn input_kind(&self, input: usize) -> Stid {
        if input < self.updates.len() {
            self.updates[input].kind
        } else {
            self.reads[input - self.updates.len()].kind
        }
    }

    pub fn output_kind(&self, output: usize) -> Stid {
        if output < self.updates.len() {
            self.updates[output].kind
        } else {
            self.creates[output - self.updates.len()].kind
        }
    }

    pub fn output_name(&self, output: usize) -> Option<&str> {
        if output < self.updates.len() {
            None
        } else {
            Some(&self.creates[output - self.updates.len()].name)
        }
    }
}

pub trait Job {
    /// First phase of a job is planning.
    ///
    /// This phase is responsible for:
    /// - Determining which jobs to run
    /// - Compute resource description for each job
    /// - Allocate resources
    ///
    /// This phase is executed for each frame, so considered hot path.
    /// It is important to keep it simple and fast,
    /// keep allocations to minimum and reuse as much as possible.
    fn plan(&mut self, planner: Planner<'_>);

    /// Second phase of a job is execution.
    ///
    /// This phase is responsible for recording commands.
    /// It fetches pre-allocated target resources and
    /// does anything necessary to record commands into command buffers:
    /// - Creating pipelines
    /// - Binding resources
    /// - Recording draw/dispatch calls
    fn exec(&mut self, runner: Exec<'_>);
}

pub struct JobCreateTarget {
    /// Target name.
    pub name: String,

    /// Target kind.
    pub kind: Stid,

    // Assigned target id.
    pub id: Option<TargetId>,
}

pub struct JobUpdateTarget {
    /// Target kind.
    pub kind: Stid,

    // Assigned target id.
    pub id: Option<TargetId>,

    pub dep_idx: Option<usize>,
}

pub struct JobReadTarget {
    /// Target kind.
    pub kind: Stid,

    // Assigned target id.
    pub id: Option<TargetId>,

    pub dep_idx: Option<usize>,
}

pub struct Planner<'a> {
    /// List of targets updates of the job correspond to.
    updates: Iter<'a, JobUpdateTarget>,

    /// List of targets creates of the job correspond to.
    creates: Iter<'a, JobCreateTarget>,

    /// List of targets reads of the job correspond to.
    reads: Iter<'a, JobReadTarget>,

    /// Where all targets live.
    hub: &'a mut TargetHub,

    /// Set of selected jobs.
    selected_jobs: &'a mut HashSet<usize>,

    device: mev::Device,
}

impl Planner<'_> {
    /// Fetcehs resource description for next update.
    pub fn update<T>(&mut self) -> Option<&T::Info>
    where
        T: Target,
    {
        let update = self.updates.next().expect("No more updates");
        assert_eq!(update.kind, Stid::of::<T>());
        let info = self.hub.plan_update::<T>(update.id?)?;

        if let Some(dep_idx) = update.dep_idx {
            self.selected_jobs.insert(dep_idx);
        }

        Some(info)
    }

    /// Provide resource description for next input.
    /// Allows merging resource description other readers.
    pub fn create<T>(&mut self) -> Option<&T::Info>
    where
        T: Target,
    {
        let create = self.creates.next().expect("No more creates");
        assert_eq!(create.kind, Stid::of::<T>());
        self.hub
            .plan_create::<T>(create.id?, &create.name, &self.device)
    }

    /// Provide resource description for next input.
    /// Allows merging resource description other readers.
    pub fn read<T>(&mut self, info: T::Info)
    where
        T: Target,
    {
        let read = self.reads.next().expect("No more reads");
        assert_eq!(read.kind, Stid::of::<T>());
        let Some(id) = read.id else {
            return;
        };
        self.hub.plan_read::<T>(id, info);

        if let Some(dep_idx) = read.dep_idx {
            self.selected_jobs.insert(dep_idx);
        }
    }
}

pub struct Exec<'a> {
    /// List of targets updates of the job correspond to.
    updates: Iter<'a, JobUpdateTarget>,

    /// List of targets creates of the job correspond to.
    creates: Iter<'a, JobCreateTarget>,

    /// List of targets reads of the job correspond to.
    reads: Iter<'a, JobReadTarget>,

    /// Where all targets live.
    hub: &'a mut TargetHub,

    device: mev::Device,
    queue: RefCell<&'a mut mev::Queue>,

    /// Arena for command buffers.
    /// This allows taking references to newly allocated command encoders
    /// And after job is done, collecting them in allocated order.
    cbufs: &'a Arena<mev::CommandEncoder>,
}

impl Exec<'_> {
    /// Fetches next resource to update.
    ///
    /// Returns none if not connected to next input.
    pub fn update<T>(&mut self) -> Option<&T>
    where
        T: Target,
    {
        let update = self.updates.next().expect("No more updates");
        self.hub.get::<T>(update.id?)
    }

    /// Fetches next resource to create.
    ///
    /// Returns none if not connected.
    pub fn create<T>(&mut self) -> Option<&T>
    where
        T: Target,
    {
        let create = self.creates.next().expect("No more creates");
        self.hub.get::<T>(create.id?)
    }

    /// Fetches next resource to read.
    ///
    /// Returns none if not connected.
    pub fn read<T>(&mut self) -> Option<&T>
    where
        T: Target,
    {
        let read = self.reads.next().expect("No more reads");
        self.hub.get::<T>(read.id?)
    }

    /// Allocates new command encoder.
    /// It will be automatically submitted to this job's queue.
    ///
    /// Returned reference is bound to this `Exec`'s borrow,
    /// so make sure to fetch target references before calling this.
    pub fn new_encoder(&self) -> &mut mev::CommandEncoder {
        let encoder = self.queue.borrow_mut().new_command_encoder().unwrap();
        self.cbufs.put(encoder)
    }

    /// Returns reference to device.
    pub fn device(&self) -> &mev::Device {
        &self.device
    }
}

pub struct JobNode {
    pub(super) job: Box<dyn Job>,
    pub(super) updates: Vec<JobUpdateTarget>,
    pub(super) creates: Vec<JobCreateTarget>,
    pub(super) reads: Vec<JobReadTarget>,
}

impl JobNode {
    pub fn new(desc: JobDesc, job: Box<dyn Job>) -> Self {
        JobNode {
            job,
            updates: desc
                .updates
                .into_iter()
                .map(|u| JobUpdateTarget {
                    kind: u.kind,
                    id: None,
                    dep_idx: None,
                })
                .collect(),
            creates: desc
                .creates
                .into_iter()
                .map(|c| JobCreateTarget {
                    kind: c.kind,
                    name: c.name,
                    id: None,
                })
                .collect(),
            reads: desc
                .reads
                .into_iter()
                .map(|c| JobReadTarget {
                    kind: c.kind,
                    id: None,
                    dep_idx: None,
                })
                .collect(),
        }
    }

    pub(super) fn plan(
        &mut self,
        hub: &mut TargetHub,
        selected_jobs: &mut HashSet<usize>,
        device: mev::Device,
    ) {
        let planner = Planner {
            updates: self.updates.iter(),
            creates: self.creates.iter(),
            reads: self.reads.iter(),
            hub,
            selected_jobs,
            device,
        };
        self.job.plan(planner);
    }

    pub(super) fn exec(
        &mut self,
        hub: &mut TargetHub,
        device: mev::Device,
        queue: &mut mev::Queue,
        cbufs: &Arena<mev::CommandEncoder>,
    ) {
        let exec = Exec {
            updates: self.updates.iter(),
            creates: self.creates.iter(),
            reads: self.reads.iter(),
            hub,
            device,
            queue: RefCell::new(queue),
            cbufs,
        };

        self.job.exec(exec);
    }
}
