mod buffer;
mod feature;
mod format;
mod image;
mod instance;
mod queue;
mod render_pipeline;
mod shader;
mod surface;

pub use self::{
    buffer::{BufferDesc, BufferUsage, Memory},
    feature::Features,
    format::{PixelFormat, VertexFormat},
    image::{ImageDesc, ImageDimensions, ImageError, ImageUsage},
    instance::{
        Capabilities, CreateError, DeviceCapabilities, DeviceDesc, FamilyCapabilities, LoadError,
    },
    queue::{QueueError, QueueFlags},
    render_pipeline::{
        Blend, BlendDesc, BlendFactor, BlendOp, ColorTargetDesc, CompareFunction,
        CreatePipelineError, DepthStencilDesc, PrimitiveTopology, RasterDesc, RenderPipelineDesc,
        VertexAttributeDesc, VertexLayoutDesc, VertexStepMode, WriteMask,
    },
    shader::{
        CreateLibraryError, LibraryDesc, LibraryInput, Shader, ShaderLanguage, ShaderSource,
        ShaderStage,
    },
    surface::SurfaceError,
};

/// Error that can happen when device's memory is exhausted.
pub struct OutOfMemory;

pub(crate) use self::shader::{compile_shader, ShaderCompileError};
