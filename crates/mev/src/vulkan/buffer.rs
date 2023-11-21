use core::fmt;
use std::{
    mem::{size_of, ManuallyDrop},
    sync::Arc,
};

use ash::vk;
use gpu_alloc::MemoryBlock;

use crate::generic::{ArgumentKind, Automatic, BufferUsage, Storage, Uniform};

use super::{
    arguments::ArgumentsField,
    device::{DeviceOwned, WeakDevice},
    refs::Refs,
};

struct Inner {
    owner: WeakDevice,
    size: usize,
    usage: BufferUsage,
    block: ManuallyDrop<MemoryBlock<(vk::DeviceMemory, usize)>>,
    idx: usize,
}

#[derive(Clone)]
pub struct Buffer {
    handle: vk::Buffer,
    inner: Arc<Inner>,
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("handle", &self.handle)
            .finish()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let block = unsafe { ManuallyDrop::take(&mut self.block) };
        self.owner.drop_buffer(self.idx, block);
    }
}

impl DeviceOwned for Buffer {
    #[inline(never)]
    fn owner(&self) -> &WeakDevice {
        &self.inner.owner
    }
}

impl Buffer {
    pub(super) fn new(
        owner: WeakDevice,
        handle: vk::Buffer,
        size: usize,
        usage: BufferUsage,
        block: MemoryBlock<(vk::DeviceMemory, usize)>,
        idx: usize,
    ) -> Self {
        Buffer {
            handle,
            inner: Arc::new(Inner {
                owner,
                size,
                usage,
                block: ManuallyDrop::new(block),
                idx,
            }),
        }
    }

    #[inline(never)]
    pub fn handle(&self) -> vk::Buffer {
        self.handle
    }
}

#[hidden_trait::expose]
impl crate::traits::Buffer for Buffer {
    #[inline(never)]
    fn size(&self) -> usize {
        self.inner.size
    }

    #[inline(never)]
    fn detached(&self) -> bool {
        debug_assert_eq!(Arc::weak_count(&self.inner), 0, "No weak refs allowed");
        Arc::strong_count(&self.inner) == 1
    }

    #[inline(never)]
    unsafe fn write_unchecked(&mut self, offset: usize, data: &[u8]) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        if let Some(device) = inner.owner.upgrade() {
            unsafe {
                let ptr = inner
                    .block
                    .map(device.inner(), offset as u64, data.len())
                    .unwrap();
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.as_ptr(), data.len());
            }
        }
    }
}

impl ArgumentsField<Automatic> for Buffer {
    const KIND: ArgumentKind = <Self as ArgumentsField<Uniform>>::KIND;
    const SIZE: usize = <Self as ArgumentsField<Uniform>>::SIZE;
    const OFFSET: usize = <Self as ArgumentsField<Uniform>>::OFFSET;
    const STRIDE: usize = <Self as ArgumentsField<Uniform>>::STRIDE;

    type Update = <Self as ArgumentsField<Uniform>>::Update;

    #[inline(never)]
    fn update(&self) -> <Self as ArgumentsField<Uniform>>::Update {
        <Self as ArgumentsField<Uniform>>::update(self)
    }

    #[inline(never)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_buffer(self.clone());
    }
}

impl ArgumentsField<Uniform> for Buffer {
    const KIND: ArgumentKind = ArgumentKind::UniformBuffer;
    const SIZE: usize = 1;
    const OFFSET: usize = 0;
    const STRIDE: usize = size_of::<vk::DescriptorBufferInfo>();

    type Update = vk::DescriptorBufferInfo;

    #[inline(never)]
    fn update(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.handle,
            offset: 0,
            range: self.inner.size as u64,
        }
    }

    #[inline(never)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_buffer(self.clone());
    }
}

impl ArgumentsField<Storage> for Buffer {
    const KIND: ArgumentKind = ArgumentKind::StorageBuffer;
    const SIZE: usize = 1;
    const OFFSET: usize = 0;
    const STRIDE: usize = size_of::<vk::DescriptorBufferInfo>();

    type Update = vk::DescriptorBufferInfo;

    #[inline(never)]
    fn update(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.handle,
            offset: 0,
            range: self.inner.size as u64,
        }
    }

    #[inline(never)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_buffer(self.clone());
    }
}
