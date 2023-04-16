use std::{alloc::Layout, mem::ManuallyDrop, sync::Arc};

use ash::vk;
use gpu_alloc::MemoryBlock;

use crate::generic::BufferUsage;

use super::device::{DeviceOwned, WeakDevice};

struct Inner {
    owner: WeakDevice,
    layout: Layout,
    usage: BufferUsage,
    block: ManuallyDrop<MemoryBlock<vk::DeviceMemory>>,
    idx: usize,
}

#[derive(Clone)]
pub struct Buffer {
    handle: vk::Buffer,
    inner: Arc<Inner>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let block = unsafe { ManuallyDrop::take(&mut self.block) };
        self.owner.drop_buffer(self.idx, block);
    }
}

impl DeviceOwned for Buffer {
    #[inline(always)]
    fn owner(&self) -> &WeakDevice {
        &self.inner.owner
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
            handle,
            inner: Arc::new(Inner {
                owner,
                layout,
                usage,
                block: ManuallyDrop::new(block),
                idx,
            }),
        }
    }

    pub(super) fn handle(&self) -> vk::Buffer {
        self.handle
    }
}
