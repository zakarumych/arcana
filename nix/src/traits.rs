use std::ops::Deref;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    backend::{Buffer, Image},
    generic::{
        BufferDesc, Capabilities, CreateError, CreateLibraryError, CreatePipelineError, DeviceDesc,
        ImageDesc, ImageError, LibraryDesc, OutOfMemory, QueueError, RenderPassDesc,
        RenderPipelineDesc, SurfaceError,
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
    fn new_surface(
        &self,
        window: &impl HasRawWindowHandle,
        display: &impl HasRawDisplayHandle,
    ) -> crate::backend::Surface;
}

pub trait Queue {
    fn new_command_encoder(&mut self) -> Result<crate::backend::CommandEncoder, OutOfMemory>;

    fn submit<I>(&mut self, command_buffers: I) -> Result<(), QueueError>
    where
        I: IntoIterator<Item = crate::backend::CommandBuffer>;

    fn present(&mut self, surface: crate::backend::SurfaceImage) -> Result<(), QueueError>;
}

pub trait CommandEncoder {
    fn render(&mut self, desc: RenderPassDesc) -> crate::backend::RenderCommandEncoder<'_>;
    fn finish(self) -> crate::backend::CommandBuffer;
}

pub trait RenderCommandEncoder {
    // fn set_pipeline(&mut self, pipeline: &crate::backend::RenderPipeline);
}

pub trait Surface {
    fn next_image(&mut self) -> Result<crate::backend::SurfaceImage, SurfaceError>;
}

pub trait SurfaceImage: Deref<Target = Image> {
    fn image(&self) -> &Image;
}
