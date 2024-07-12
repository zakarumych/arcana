use arcana_names::Name;
use edict::world::World;
use hashbrown::HashMap;

use crate::{
    make_id,
    model::{Model, Value},
    stid::WithStid,
    Stid,
};

use super::graph::{Exec, Planner};

make_id! {
    /// ID of the render job.
    pub JobId;
}

/// Descroption of job creating a target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetCreateDesc {
    /// Create name.
    pub name: Name,

    /// Target type.
    pub ty: Stid,
}

impl TargetCreateDesc {
    pub fn new<T: WithStid>(name: Name) -> Self {
        TargetCreateDesc {
            name,
            ty: T::stid(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetUpdateDesc {
    /// Update name.
    pub name: Name,

    /// Target type.
    pub ty: Stid,
}

impl TargetUpdateDesc {
    pub fn new<T: WithStid>(name: Name) -> Self {
        TargetUpdateDesc {
            name,
            ty: T::stid(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetReadDesc {
    /// Read name.
    pub name: Name,

    /// Target type.
    pub ty: Stid,
}

impl TargetReadDesc {
    pub fn new<T: WithStid>(name: Name) -> Self {
        TargetReadDesc {
            name,
            ty: T::stid(),
        }
    }
}

/// Job description.
/// A set of targets a job creates, updates and reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JobDesc {
    /// Job parameters.
    pub params: Vec<(Name, Model)>,

    /// List of targets job reads.
    /// They are inputs of the job.
    pub reads: Vec<TargetReadDesc>,

    /// List of targets job updates.
    /// They are inputs and outputs of the job.
    pub updates: Vec<TargetUpdateDesc>,

    /// List of targets job creates.
    /// They are outputs of the job.
    pub creates: Vec<TargetCreateDesc>,
}

impl JobDesc {
    pub fn output_count(&self) -> usize {
        self.updates.len() + self.creates.len()
    }

    pub fn input_count(&self) -> usize {
        self.updates.len() + self.reads.len() + self.params.len()
    }

    pub fn update_idx(&self, pin: usize) -> Option<usize> {
        if pin < self.updates.len() {
            Some(pin)
        } else {
            None
        }
    }

    pub fn read_idx(&self, pin: usize) -> Option<usize> {
        if pin >= self.updates.len() && pin < self.updates.len() + self.reads.len() {
            Some(pin - self.updates.len())
        } else {
            None
        }
    }

    pub fn param_idx(&self, pin: usize) -> Option<usize> {
        if pin >= self.updates.len() + self.reads.len()
            && pin < self.updates.len() + self.reads.len() + self.params.len()
        {
            Some(pin - self.updates.len() - self.reads.len())
        } else {
            None
        }
    }

    pub fn create_idx(&self, pin: usize) -> Option<usize> {
        if pin >= self.updates.len() && pin < self.creates.len() + self.updates.len() {
            Some(pin - self.updates.len())
        } else {
            None
        }
    }

    /// Returns input stable type ID by index.
    #[track_caller]
    pub fn input_type(&self, pin: usize) -> Stid {
        match (self.update_idx(pin), self.read_idx(pin)) {
            (Some(update), _) => self.updates[update].ty,
            (_, Some(read)) => self.reads[read].ty,
            _ => invalid_input_pin(pin),
        }
    }

    /// Returns output stable type ID by index.
    #[track_caller]
    pub fn output_type(&self, pin: usize) -> Stid {
        match (self.create_idx(pin), self.update_idx(pin)) {
            (Some(create), _) => self.creates[create].ty,
            (_, Some(update)) => self.updates[update].ty,
            _ => invalid_output_pin(pin),
        }
    }

    /// Returns output name by index.
    #[track_caller]
    pub fn output_name(&self, pin: usize) -> Option<&str> {
        match (self.create_idx(pin), self.update_idx(pin)) {
            (Some(create), _) => Some(&self.creates[create].name),
            (_, Some(_)) => None,
            _ => invalid_output_pin(pin),
        }
    }

    pub fn default_params(&self) -> HashMap<Name, Value> {
        self.params
            .iter()
            .map(|(k, m)| (*k, m.default_value()))
            .collect()
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! add_job_desc {
    (($params:ident, $reads:ident, $updates:ident, $creates:ident)) => {};
    (($params:ident, $reads:ident, $updates:ident, $creates:ident) $name:ident: $ty:ty , $($rest:tt)*) => {
        $reads.push($crate::work::TargetReadDesc::new::< $ty >($crate::ident!($name).into()));
        $crate::add_job_desc!(($params, $reads, $updates, $creates) $($rest)*);
    };
    (($params:ident, $reads:ident, $updates:ident, $creates:ident) $name:ident: mut $ty:ty, $($rest:tt)*) => {
        $updates.push($crate::work::TargetUpdateDesc::new::< $ty >($crate::ident!($name).into()));
        $crate::add_job_desc!(($params, $reads, $updates, $creates) $($rest)*);
    };
    (($params:ident, $reads:ident, $updates:ident, $creates:ident) $name:ident: +$ty:ty , $($rest:tt)*) => {
        $creates.push($crate::work::TargetCreateDesc::new::< $ty >($crate::ident!($name).into()));
        $crate::add_job_desc!(($params, $reads, $updates, $creates) $($rest)*);
    };
    (($params:ident, $reads:ident, $updates:ident, $creates:ident) $name:ident: in $model:expr , $($rest:tt)*) => {
        $params.push(($crate::ident!($name).into(), $model));
        $crate::add_job_desc!(($params, $reads, $updates, $creates) $($rest)*);
    };
}

#[macro_export]
macro_rules! job_desc {
    ($(@$model:expr,)? $($descs:tt)*) => {{
        let mut params = std::vec::Vec::new();
        let mut reads = std::vec::Vec::new();
        let mut updates = std::vec::Vec::new();
        let mut creates = std::vec::Vec::new();
        $crate::add_job_desc!((params, reads, updates, creates) $($descs)*);
        $crate::work::JobDesc {
            params,
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
