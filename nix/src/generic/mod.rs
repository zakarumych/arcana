// mod _arguments;
mod arguments;
mod buffer;
mod constants;
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
    arguments::{
        ArgumentGroupLayout, ArgumentKind, ArgumentLayout, Arguments, ArgumentsField, Automatic,
        /*Constant,*/ Sampled, Storage, Uniform,
    },
    buffer::{BufferDesc, BufferInitDesc, BufferUsage, Memory},
    constants::*,
    feature::Features,
    format::{PixelFormat, VertexFormat},
    image::{ImageDesc, ImageDimensions, ImageError, ImageUsage},
    instance::{
        Capabilities, CreateError, DeviceCapabilities, DeviceDesc, FamilyCapabilities, LoadError,
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
    stages::{PipelineStage, PipelineStages},
    surface::SurfaceError,
};

pub(super) use self::arguments::ArgumentsSealed;

/// Error that can happen when device's memory is exhausted.
#[derive(Debug)]
pub struct OutOfMemory;

impl fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "out of memory")
    }
}

impl Error for OutOfMemory {}

pub trait Zero {
    const ZERO: Self;
}

impl Zero for u32 {
    const ZERO: Self = 0;
}

impl Zero for i32 {
    const ZERO: Self = 0;
}

impl Zero for f32 {
    const ZERO: Self = 0.0;
}

#[repr(transparent)]
pub struct Offset<T, const D: usize>(pub [T; D]);

impl<T, const D: usize> Offset<T, D>
where
    T: Zero,
{
    pub const ZERO: Self = Self([T::ZERO; D]);
}

pub type Offset1<T = i32> = Offset<T, 1>;
pub type Offset2<T = i32> = Offset<T, 2>;
pub type Offset3<T = i32> = Offset<T, 3>;

impl<T> Offset1<T> {
    pub const fn new(x: T) -> Self {
        Self([x])
    }

    pub const fn x(&self) -> T
    where
        T: Copy,
    {
        self.0[0]
    }
}

impl<T> Offset2<T> {
    pub const fn new(x: T, y: T) -> Self {
        Self([x, y])
    }

    pub const fn x(&self) -> T
    where
        T: Copy,
    {
        self.0[0]
    }

    pub const fn y(&self) -> T
    where
        T: Copy,
    {
        self.0[1]
    }
}

impl<T> Offset3<T> {
    pub const fn new(x: T, y: T, z: T) -> Self {
        Self([x, y, z])
    }

    pub const fn x(&self) -> T
    where
        T: Copy,
    {
        self.0[0]
    }

    pub const fn y(&self) -> T
    where
        T: Copy,
    {
        self.0[1]
    }

    pub const fn z(&self) -> T
    where
        T: Copy,
    {
        self.0[2]
    }
}

#[repr(transparent)]
pub struct Extent<T, const D: usize>(pub [T; D]);

impl<T, const D: usize> Extent<T, D>
where
    T: Zero,
{
    pub const ZERO: Self = Self([T::ZERO; D]);
}

impl<const D: usize> Extent<f32, D> {
    pub const ONE: Self = Self([1.0; D]);
}

pub type Extent1<T = u32> = Extent<T, 1>;
pub type Extent2<T = u32> = Extent<T, 2>;
pub type Extent3<T = u32> = Extent<T, 3>;

impl<T> Extent1<T> {
    pub const fn new(width: T) -> Self {
        Self([width])
    }

    pub const fn width(&self) -> T
    where
        T: Copy,
    {
        self.0[0]
    }
}

impl<T> Extent2<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self([width, height])
    }

    pub const fn width(&self) -> T
    where
        T: Copy,
    {
        self.0[0]
    }

    pub const fn height(&self) -> T
    where
        T: Copy,
    {
        self.0[1]
    }
}

impl<T> Extent3<T> {
    pub const fn new(width: T, height: T, depth: T) -> Self {
        Self([width, height, depth])
    }

    pub const fn width(&self) -> T
    where
        T: Copy,
    {
        self.0[0]
    }

    pub const fn height(&self) -> T
    where
        T: Copy,
    {
        self.0[1]
    }

    pub const fn depth(&self) -> T
    where
        T: Copy,
    {
        self.0[2]
    }
}

pub(crate) use self::shader::{compile_shader, ShaderCompileError};
