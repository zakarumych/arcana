use std::sync::{Arc, Weak};

use crate::generic::Argument;

use super::device::WeakDevice;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct DescriptorSetLayoutDesc {
    pub arguments: Vec<Argument>,
}

struct DescriptorSetLayoutInner {
    owner: WeakDevice,
    desc: DescriptorSetLayoutDesc,
}

impl Drop for DescriptorSetLayoutInner {
    fn drop(&mut self) {
        self.owner.drop_descriptor_set_layout(std::mem::replace(
            &mut self.desc,
            DescriptorSetLayoutDesc {
                arguments: Vec::new(),
            },
        ));
    }
}

#[derive(Clone)]
pub(super) struct WeakDescriptorSetLayout {
    handle: ash::vk::DescriptorSetLayout,
    inner: Weak<DescriptorSetLayoutInner>,
}

impl WeakDescriptorSetLayout {
    pub fn unused(&self) -> bool {
        self.inner.strong_count() == 0
    }

    pub fn upgrade(&self) -> Option<DescriptorSetLayout> {
        let inner = self.inner.upgrade()?;
        Some(DescriptorSetLayout {
            handle: self.handle,
            inner,
        })
    }

    pub fn handle(&self) -> ash::vk::DescriptorSetLayout {
        self.handle
    }
}

#[derive(Clone)]
pub(super) struct DescriptorSetLayout {
    handle: ash::vk::DescriptorSetLayout,
    inner: Arc<DescriptorSetLayoutInner>,
}

impl DescriptorSetLayout {
    pub fn new(
        owner: WeakDevice,
        handle: ash::vk::DescriptorSetLayout,
        desc: DescriptorSetLayoutDesc,
    ) -> Self {
        DescriptorSetLayout {
            handle,
            inner: Arc::new(DescriptorSetLayoutInner { owner, desc }),
        }
    }

    pub fn downgrade(&self) -> WeakDescriptorSetLayout {
        WeakDescriptorSetLayout {
            handle: self.handle,
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn handle(&self) -> ash::vk::DescriptorSetLayout {
        self.handle
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct PipelineLayoutDesc {
    pub groups: Vec<Vec<Argument>>,
}

struct PipelineLayoutInner {
    owner: WeakDevice,
    desc: PipelineLayoutDesc,
}

impl Drop for PipelineLayoutInner {
    fn drop(&mut self) {
        self.owner.drop_pipeline_layout(std::mem::replace(
            &mut self.desc,
            PipelineLayoutDesc { groups: Vec::new() },
        ));
    }
}

#[derive(Clone)]
pub(super) struct WeakPipelineLayout {
    handle: ash::vk::PipelineLayout,
    inner: Weak<PipelineLayoutInner>,
}

impl WeakPipelineLayout {
    pub fn unused(&self) -> bool {
        self.inner.strong_count() == 0
    }

    pub fn upgrade(&self) -> Option<PipelineLayout> {
        let inner = self.inner.upgrade()?;
        Some(PipelineLayout {
            handle: self.handle,
            inner,
        })
    }

    pub fn handle(&self) -> ash::vk::PipelineLayout {
        self.handle
    }
}

#[derive(Clone)]
pub(super) struct PipelineLayout {
    handle: ash::vk::PipelineLayout,
    inner: Arc<PipelineLayoutInner>,
}

impl PipelineLayout {
    pub fn new(
        owner: WeakDevice,
        handle: ash::vk::PipelineLayout,
        desc: PipelineLayoutDesc,
    ) -> Self {
        PipelineLayout {
            handle,
            inner: Arc::new(PipelineLayoutInner { owner, desc }),
        }
    }

    pub fn downgrade(&self) -> WeakPipelineLayout {
        WeakPipelineLayout {
            handle: self.handle,
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn handle(&self) -> ash::vk::PipelineLayout {
        self.handle
    }
}
