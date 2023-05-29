use std::{
    mem::size_of,
    sync::{Arc, Weak},
};

use ash::vk;

use crate::generic::{ArgumentKind, Automatic, SamplerDesc};

use super::{
    arguments::ArgumentsField,
    device::{DeviceOwned, WeakDevice},
    refs::Refs,
};

struct Inner {
    owner: WeakDevice,
    desc: SamplerDesc,
}

#[derive(Clone)]
pub(super) struct WeakSampler {
    handle: vk::Sampler,
    inner: Weak<Inner>,
}

impl WeakSampler {
    #[inline]
    pub(super) fn upgrade(&self) -> Option<Sampler> {
        let inner = self.inner.upgrade()?;
        Some(Sampler {
            handle: self.handle,
            inner,
        })
    }

    #[inline]
    pub(super) fn unused(&self) -> bool {
        self.inner.strong_count() == 0
    }

    #[inline(always)]
    pub(super) fn handle(&self) -> vk::Sampler {
        self.handle
    }
}

#[derive(Clone)]
pub struct Sampler {
    handle: vk::Sampler,
    inner: Arc<Inner>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.owner.drop_sampler(self.desc);
    }
}

impl DeviceOwned for Sampler {
    #[inline(always)]
    fn owner(&self) -> &WeakDevice {
        &self.inner.owner
    }
}

impl Sampler {
    #[inline]
    pub(super) fn new(owner: WeakDevice, handle: vk::Sampler, desc: SamplerDesc) -> Self {
        Sampler {
            handle,
            inner: Arc::new(Inner { owner, desc }),
        }
    }

    #[inline]
    pub(super) fn downgrade(&self) -> WeakSampler {
        WeakSampler {
            handle: self.handle,
            inner: Arc::downgrade(&self.inner),
        }
    }

    #[inline(always)]
    pub(super) fn handle(&self) -> vk::Sampler {
        self.handle
    }
}

impl ArgumentsField<Automatic> for Sampler {
    const KIND: ArgumentKind = ArgumentKind::Sampler;
    const SIZE: usize = 1;
    const OFFSET: usize = 0;
    const STRIDE: usize = size_of::<vk::DescriptorImageInfo>();

    type Update = vk::DescriptorImageInfo;

    #[inline(always)]
    fn update(&self) -> vk::DescriptorImageInfo {
        vk::DescriptorImageInfo {
            sampler: self.handle,
            image_view: vk::ImageView::null(),
            image_layout: vk::ImageLayout::UNDEFINED,
        }
    }

    #[inline(always)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_sampler(self.clone());
    }
}
