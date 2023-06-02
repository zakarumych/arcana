mod argument;
mod buffer;
mod command;
mod compute_pipeline;
mod device;
mod from;
mod image;
mod instance;
mod queue;
mod render_pipeline;
mod sampler;
mod shader;
mod surface;

pub use self::{
    buffer::Buffer,
    command::{CommandBuffer, CommandEncoder, CopyCommandEncoder, RenderCommandEncoder},
    compute_pipeline::ComputePipeline,
    device::Device,
    image::Image,
    instance::Instance,
    queue::Queue,
    render_pipeline::RenderPipeline,
    sampler::Sampler,
    shader::Library,
    surface::{Frame, Surface},
};

pub(crate) use self::{
    instance::{CreateErrorKind, LoadErrorKind},
    render_pipeline::CreatePipelineErrorKind,
    shader::CreateLibraryErrorKind,
    surface::SurfaceErrorKind,
};

// Minimize functions size by offloading panic to a separate function.
#[cold]
#[inline(never)]
fn out_of_bounds() -> ! {
    panic!("offset + data.len() > buffer.length()");
}

pub mod for_macro {}
