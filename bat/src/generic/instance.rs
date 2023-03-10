use crate::backend::{CreateError, Device, LoadError};

use super::{feature::Features, queue::QueueFlags};

/// Capabilities of a queue family of specific device.
#[derive(Clone, Debug)]
pub struct FamilyCapabilities {
    /// Flags that describe the capabilities of the queue family.
    pub queue_flags: QueueFlags,

    /// Number of queues that can be created in the queue family.
    pub queue_count: usize,
}

/// Capabilities of the specific device.
#[derive(Clone, Debug)]
pub struct DeviceCapabilities {
    /// List of features that are supported by the device.
    pub features: Features,

    /// List of queue families capabilities.
    pub families: Vec<FamilyCapabilities>,
}

/// Capabilities of the devices.
#[derive(Clone, Debug)]
pub struct Capabilities {
    pub devices: Vec<DeviceCapabilities>,
}

/// Specifies how many queues of what family should be created.
pub struct QueuesCreateDesc {
    /// Index of the queue family.
    pub idx: u32,

    /// Number of queues to create.
    pub queue_count: usize,
}

/// Specifies how the device should be created.
pub struct DeviceDesc {
    /// Index of the device.
    pub idx: usize,

    /// List of queue infos.
    pub queue_infos: Vec<QueuesCreateDesc>,

    /// List of features that should be enabled.
    pub features: Features,
}

pub trait Instance {
    fn load() -> Result<Self, LoadError>
    where
        Self: Sized;

    fn capabilities(&self) -> &Capabilities;
    fn create(&self, info: DeviceDesc) -> Result<Device, CreateError>;
}
