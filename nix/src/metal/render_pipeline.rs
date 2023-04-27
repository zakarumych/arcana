pub struct RenderPipeline {
    state: metal::RenderPipelineState,
    primitive: metal::MTLPrimitiveType,
}

unsafe impl Send for RenderPipeline {}
unsafe impl Sync for RenderPipeline {}

impl RenderPipeline {
    pub(super) fn new(
        state: metal::RenderPipelineState,
        primitive: metal::MTLPrimitiveType,
    ) -> Self {
        RenderPipeline { state, primitive }
    }

    pub(super) fn metal(&self) -> &metal::RenderPipelineState {
        &self.state
    }

    pub(super) fn primitive(&self) -> metal::MTLPrimitiveType {
        self.primitive
    }
}

#[derive(Debug)]
pub enum CreatePipelineErrorKind {
    InvalidShaderEntry,
    FailedToBuildPipeline(String),
}
