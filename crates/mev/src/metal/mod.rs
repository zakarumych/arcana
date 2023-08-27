mod arguments;
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

const MAX_VERTEX_BUFFERS: u32 = 31;

pub mod for_macro {
    pub use crate::generic::Constants;

    pub use super::arguments::{Arguments, ArgumentsField};
    pub use bytemuck::{Pod, Zeroable};
    pub use std::{
        mem::{align_of, size_of, MaybeUninit},
        ptr::addr_of,
    };

    pub const fn pad_for<T: Constants>(end: usize) -> usize {
        let align = align_of::<T>();
        pad_align(end, align)
    }

    pub const fn pad_align(end: usize, align: usize) -> usize {
        ((end + (align - 1)) & !(align - 1)) - end
    }
}
