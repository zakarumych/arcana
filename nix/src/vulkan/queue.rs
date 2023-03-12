use std::fmt;

use ash::vk;

use crate::generic::QueueFlags;

use super::device::Device;

#[derive(Clone)]
pub struct Queue {
    device: Device,
    queue: vk::Queue,
    flags: QueueFlags,
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Queue({:p}@{:?})", self.queue, self.device)
    }
}

impl Queue {
    pub(super) fn new(device: Device, queue: vk::Queue, flags: QueueFlags) -> Self {
        Queue {
            device,
            queue,
            flags,
        }
    }
}

pub(super) struct Family {
    pub queues: Vec<vk::Queue>,
    pub flags: QueueFlags,
}
