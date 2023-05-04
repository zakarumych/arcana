use std::sync::Arc;

use ash::vk;

use crate::generic::OutOfMemory;

use super::{device::WeakDevice, layout::PipelineLayout};

struct Inner {
    owner: WeakDevice,
    layout: PipelineLayout,
    idx: usize,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.owner.drop_pipeline(self.idx);
    }
}

#[derive(Clone)]
pub struct RenderPipeline {
    handle: vk::Pipeline,
    layout: vk::PipelineLayout,
    inner: Arc<Inner>,
}

impl RenderPipeline {
    pub(super) fn new(
        owner: WeakDevice,
        handle: vk::Pipeline,
        idx: usize,
        layout: PipelineLayout,
    ) -> Self {
        RenderPipeline {
            handle,
            layout: layout.handle(),
            inner: Arc::new(Inner { owner, layout, idx }),
        }
    }

    pub(super) fn handle(&self) -> vk::Pipeline {
        self.handle
    }

    pub(super) fn layout(&self) -> &PipelineLayout {
        &self.inner.layout
    }
}

#[derive(Debug)]
pub enum CreatePipelineErrorKind {
    OutOfMemory,
    InvalidShaderEntry,
}

impl From<OutOfMemory> for CreatePipelineErrorKind {
    #[inline(always)]
    fn from(_: OutOfMemory) -> Self {
        CreatePipelineErrorKind::OutOfMemory
    }
}
