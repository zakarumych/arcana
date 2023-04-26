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

pub struct RenderPipeline {
    handle: vk::Pipeline,
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
            inner: Arc::new(Inner { owner, layout, idx }),
        }
    }
}

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
