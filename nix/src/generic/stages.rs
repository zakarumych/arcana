/// Stages in the rendering pipeline.
#[repr(u16)]
pub enum RenderStage {
    /// Stage at which indirect draw commands are read from buffer.
    DrawIndirect,

    /// Stage at which vertex data is read from buffer.
    VertexInput,

    /// Stage at which vertex shader is executed.
    VertexShader,

    /// Stage at which early fragment tests are performed.
    EarlyFragmentTest,

    /// Stage at which fragment shader is executed.
    FragmentShader,

    /// Stage at which late fragment tests are performed.
    LateFragmentTest,

    /// Stage at which color data is written to buffer.
    ColorOutput,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct RenderStages: u32 {
        /// Bit for [`DrawIndirect`](RenderStage::DrawIndirect) stage.
        const DRAW_INDIRECT = 1 << RenderStage::DrawIndirect as u32;
        /// Bit for [`VertexInput`](RenderStage::VertexInput) stage.
        const VERTEX_INPUT = 1 << RenderStage::VertexInput as u32;
        /// Bit for [`VertexShader`](RenderStage::VertexShader) stage.
        const VERTEX_SHADER = 1 << RenderStage::VertexShader as u32;
        /// Bit for [`EarlyFragmentTest`](RenderStage::EarlyFragmentTest) stage.
        const EARLY_FRAGMENT_TEST = 1 << RenderStage::EarlyFragmentTest as u32;
        /// Bit for [`FragmentShader`](RenderStage::FragmentShader) stage.
        const FRAGMENT_SHADER = 1 << RenderStage::FragmentShader as u32;
        /// Bit for [`LateFragmentTest`](RenderStage::LateFragmentTest) stage.
        const LATE_FRAGMENT_TEST = 1 << RenderStage::LateFragmentTest as u32;
        /// Bit for [`ColorOutput`](RenderStage::ColorOutput) stage.
        const COLOR_OUTPUT = 1 << RenderStage::ColorOutput as u32;
    }
}
