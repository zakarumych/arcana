use std::alloc::Layout;

use ash::vk;
use gpu_alloc::MemoryBlock;

use crate::generic::BufferUsage;

use super::device::{DeviceOwned, WeakDevice};

pub struct Buffer {
    handle: vk::Buffer,
    owner: WeakDevice,
    layout: Layout,
    usage: BufferUsage,
    block: MemoryBlock<vk::DeviceMemory>,
    idx: usize,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.owner.drop_buffer(self.idx);
    }
}

impl DeviceOwned for Buffer {
    #[inline(always)]
    fn owner(&self) -> &WeakDevice {
        &self.owner
    }
}

impl Buffer {
    pub(super) fn new(
        owner: WeakDevice,
        handle: vk::Buffer,
        layout: Layout,
        usage: BufferUsage,
        block: MemoryBlock<vk::DeviceMemory>,
        idx: usize,
    ) -> Self {
        Buffer {
            owner,
            handle,
            layout,
            usage,
            block,
            idx,
        }
    }

    pub(super) fn handle(&self) -> vk::Buffer {
        self.handle
    }
}
