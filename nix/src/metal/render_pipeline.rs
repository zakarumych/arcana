pub struct RenderPipeline {
    state: metal::RenderPipelineState,
}

impl RenderPipeline {
    pub(super) fn new(state: metal::RenderPipelineState) -> Self {
        RenderPipeline { state }
    }
}

pub enum CreatePipelineErrorKind {
    InvalidShaderEntry,
    FailedToBuildPipeline(String),
}
