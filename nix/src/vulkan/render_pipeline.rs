use ash::vk;

use crate::generic::OutOfMemory;

pub struct RenderPipeline {
    pipeline: vk::Pipeline,
}

impl RenderPipeline {
    pub(super) fn new(pipeline: vk::Pipeline) -> Self {
        RenderPipeline { pipeline }
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
