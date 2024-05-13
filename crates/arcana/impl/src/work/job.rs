use edict::World;
use hashbrown::HashMap;

use crate::{make_id, model::Model, stid::WithStid, Stid};

use super::graph::{Exec, Planner};

make_id!(pub JobId);

/// Descroption of job creating a target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobCreateDesc {
    /// Target name.
    pub name: String,

    /// Target type.
    pub ty: Stid,
}

impl JobCreateDesc {
    pub fn new<T: WithStid>(name: impl Into<String>) -> Self {
        JobCreateDesc {
            name: name.into(),
            ty: T::stid(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobUpdateDesc {
    /// Target type.
    pub ty: Stid,
}

impl JobUpdateDesc {
    pub fn new<T: WithStid>() -> Self {
        JobUpdateDesc { ty: T::stid() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobReadDesc {
    /// Target type.
    pub ty: Stid,
}

impl JobReadDesc {
    pub fn new<T: WithStid>() -> Self {
        JobReadDesc { ty: T::stid() }
    }
}

/// Job description.
/// A set of targets a job creates, updates and reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JobDesc {
    /// Job configuration model.
    pub cfg: HashMap<String, Model>,

    /// List of targets job reads.
    /// They are inputs of the job.
    pub reads: Vec<JobReadDesc>,

    /// List of targets job updates.
    /// They are inputs and outputs of the job.
    pub updates: Vec<JobUpdateDesc>,

    /// List of targets job creates.
    /// They are outputs of the job.
    pub creates: Vec<JobCreateDesc>,
}

impl JobDesc {
    pub(super) fn input_update(&self, pin: usize) -> Option<usize> {
        if pin < self.updates.len() {
            Some(pin)
        } else {
            None
        }
    }

    pub(super) fn input_read(&self, pin: usize) -> Option<usize> {
        if pin >= self.updates.len() && pin < self.updates.len() + self.reads.len() {
            Some(pin - self.updates.len())
        } else {
            None
        }
    }

    pub(super) fn output_update(&self, pin: usize) -> Option<usize> {
        if pin < self.updates.len() {
            Some(pin)
        } else {
            None
        }
    }

    pub(super) fn output_create(&self, pin: usize) -> Option<usize> {
        if pin >= self.updates.len() && pin < self.creates.len() + self.updates.len() {
            Some(pin - self.updates.len())
        } else {
            None
        }
    }

    /// Returns input stable type ID by index.
    #[track_caller]
    pub fn input_type(&self, pin: usize) -> Stid {
        match (self.input_update(pin), self.input_read(pin)) {
            (Some(update), _) => self.updates[update].ty,
            (_, Some(read)) => self.reads[read].ty,
            _ => invalid_input_pin(pin),
        }
    }

    /// Returns output stable type ID by index.
    #[track_caller]
    pub fn output_type(&self, pin: usize) -> Stid {
        match (self.output_create(pin), self.output_update(pin)) {
            (Some(create), _) => self.creates[create].ty,
            (_, Some(update)) => self.updates[update].ty,
            _ => invalid_output_pin(pin),
        }
    }

    /// Returns output name by index.
    #[track_caller]
    pub fn output_name(&self, pin: usize) -> Option<&str> {
        match (self.output_create(pin), self.output_update(pin)) {
            (Some(create), _) => Some(&self.creates[create].name),
            (_, Some(_)) => None,
            _ => invalid_output_pin(pin),
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! add_job_desc {
    (($reads:ident, $updates:ident, $creates:ident)) => {};
    (($reads:ident, $updates:ident, $creates:ident) $ty:ty , $($rest:tt)*) => {
        $reads.push($crate::work::JobReadDesc::new::< $ty >());
        $crate::add_job_desc!(($reads, $updates, $creates) $($rest)*);
    };
    (($reads:ident, $updates:ident, $creates:ident) mut $ty:ty , $($rest:tt)*) => {
        $updates.push($crate::work::JobUpdateDesc::new::< $ty >());
        $crate::add_job_desc!(($reads, $updates, $creates) $($rest)*);
    };
    (($reads:ident, $updates:ident, $creates:ident) +$ty:ty => $name:expr , $($rest:tt)*) => {
        $creates.push($crate::work::JobCreateDesc::new::< $ty >($name));
        $crate::add_job_desc!(($reads, $updates, $creates) $($rest)*);
    };
}

#[macro_export]
macro_rules! job_desc {
    ($(@$model:expr;)? $($descs:tt)*) => {{
        let mut reads = Vec::new();
        let mut updates = Vec::new();
        let mut creates = Vec::new();
        $crate::add_job_desc!((reads, updates, creates) $($descs)*);
        $crate::work::JobDesc {
            cfg: {
                let mut cfg = $crate::hashbrown::HashMap::new();
                $(cfg.extend($model);)?
                cfg
            },
            reads,
            updates,
            creates,
        }
    }};
}

pub trait Job: 'static {
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
    fn plan(&mut self, planner: Planner<'_>, world: &mut World);

    /// Second phase of a job is execution.
    ///
    /// This phase is responsible for recording commands.
    /// It fetches pre-allocated target resources and
    /// does anything necessary to record commands into command buffers:
    /// - Creating pipelines
    /// - Binding resources
    /// - Recording draw/dispatch calls
    fn exec(&mut self, exec: Exec<'_>, world: &mut World);
}

#[track_caller]
pub(super) fn invalid_input_pin(pin: usize) -> ! {
    panic!("Invalid input pin index: {}", pin)
}

#[track_caller]
pub(super) fn invalid_output_pin(pin: usize) -> ! {
    panic!("Invalid output pin index: {}", pin)
}
