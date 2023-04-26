use std::ops::Deref;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    backend::{Buffer, Image, Sampler},
    generic::{
        BufferDesc, Capabilities, CreateError, CreateLibraryError, CreatePipelineError, DeviceDesc,
        ImageDesc, ImageError, LibraryDesc, OutOfMemory, QueueError, RenderPassDesc,
        RenderPipelineDesc, SamplerDesc, SurfaceError,
    },
};

pub trait Instance {
    // fn load() -> Result<Self, LoadError>
    // where
    //     Self: Sized;

    fn capabilities(&self) -> &Capabilities;
    fn create(&self, info: DeviceDesc) -> Result<crate::backend::Device, CreateError>;
}

pub trait Device {
    fn get_queue(&self, family: usize, idx: usize) -> crate::backend::Queue;
    fn new_shader_library(
        &self,
        desc: LibraryDesc,
    ) -> Result<crate::backend::Library, CreateLibraryError>;
    fn new_render_pipeline(
        &self,
        desc: RenderPipelineDesc,
    ) -> Result<crate::backend::RenderPipeline, CreatePipelineError>;
    fn new_buffer(&self, desc: BufferDesc) -> Result<Buffer, OutOfMemory>;
    fn new_image(&self, desc: ImageDesc) -> Result<Image, ImageError>;
    fn new_sampler(&self, desc: SamplerDesc) -> Result<Sampler, OutOfMemory>;
    fn new_surface(
        &self,
        window: &impl HasRawWindowHandle,
        display: &impl HasRawDisplayHandle,
    ) -> Result<crate::backend::Surface, SurfaceError>;
}

pub trait Queue {
    fn new_command_encoder(&mut self) -> Result<crate::backend::CommandEncoder, OutOfMemory>;

    fn submit<I>(&mut self, command_buffers: I) -> Result<(), QueueError>
    where
        I: IntoIterator<Item = crate::backend::CommandBuffer>;

    fn present(&mut self, surface: crate::backend::SurfaceImage) -> Result<(), QueueError>;
}

pub trait CommandEncoder {
    fn barrier(&mut self);
    fn render(&mut self, desc: RenderPassDesc) -> crate::backend::RenderCommandEncoder<'_>;
    fn finish(self) -> Result<crate::backend::CommandBuffer, OutOfMemory>;
}

pub trait RenderCommandEncoder {
    // fn set_pipeline(&mut self, pipeline: &crate::backend::RenderPipeline);
}

pub trait Surface {
    fn next_image(
        &mut self,
        queue: &mut crate::backend::Queue,
    ) -> Result<crate::backend::SurfaceImage, SurfaceError>;
}

pub trait SurfaceImage: Deref<Target = Image> {
    fn image(&self) -> &Image;
}
