mod buffer;
mod feature;
mod format;
mod image;
mod instance;
mod queue;
mod render;
mod render_pipeline;
mod shader;
mod surface;

use std::{error::Error, fmt};

pub use self::{
    buffer::{BufferDesc, BufferUsage, Memory},
    feature::Features,
    format::{PixelFormat, VertexFormat},
    image::{ImageDesc, ImageDimensions, ImageError, ImageUsage},
    instance::{
        Capabilities, CreateError, DeviceCapabilities, DeviceDesc, FamilyCapabilities, LoadError,
        QueuesCreateDesc,
    },
    queue::{QueueError, QueueFlags},
    render::{AttachmentDesc, ClearColor, ClearDepthStencil, LoadOp, RenderPassDesc, StoreOp},
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
#[derive(Debug)]
pub struct OutOfMemory;

impl fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "out of memory")
    }
}

impl Error for OutOfMemory {}

pub(crate) use self::shader::{compile_shader, ShaderCompileError};
