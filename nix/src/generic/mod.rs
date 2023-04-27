mod _arguments;
mod arguments;
mod buffer;
mod feature;
mod format;
mod image;
mod instance;
mod queue;
mod render;
mod render_pipeline;
mod sampler;
mod shader;
mod stages;
mod surface;

use std::{error::Error, fmt};

pub use self::{
    _arguments::{data_types::*, ArgumentKind, ArgumentLayout, Arguments},
    arguments::{
        Argument, ArgumentGroup, ArgumentKind, Arguments, WriteArgument, WriteArgumentGroup,
    },
    buffer::{BufferDesc, BufferInitDesc, BufferUsage, Memory},
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
    sampler::{AddressMode, Filter, MipMapMode, SamplerDesc},
    shader::{
        CreateLibraryError, LibraryDesc, LibraryInput, Shader, ShaderLanguage, ShaderSource,
        ShaderStage, ShaderStages,
    },
    stages::{RenderStage, RenderStages},
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
