pub struct ComputePipeline {
    state: metal::ComputePipelineState,
}

unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}

impl ComputePipeline {
    pub(super) fn new(state: metal::ComputePipelineState) -> Self {
        ComputePipeline { state }
    }

    pub(super) fn metal(&self) -> &metal::ComputePipelineState {
        &self.state
    }
}
