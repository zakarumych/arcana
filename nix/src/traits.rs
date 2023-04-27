use std::ops::Range;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    backend::{Buffer, Image, Sampler},
    generic::{
        ArgumentKind, ArgumentLayout, Arguments, BufferDesc, BufferDesc, BufferInitDesc,
        Capabilities, Capabilities, CreateError, CreateError, CreateLibraryError,
        CreateLibraryError, CreatePipelineError, CreatePipelineError, DeviceDesc, DeviceDesc,
        ImageDesc, ImageDesc, ImageError, ImageError, LibraryDesc, LibraryDesc, OutOfMemory,
        OutOfMemory, QueueError, QueueError, RenderPassDesc, RenderPassDesc, RenderPipelineDesc,
        RenderPipelineDesc, RenderStages, SamplerDesc, SurfaceError, SurfaceError,
    },
};

pub trait Instance {
    fn capabilities(&self) -> &Capabilities;
    fn create(
        &self,
        info: DeviceDesc,
    ) -> Result<(crate::backend::Device, Vec<crate::backend::Queue>), CreateError>;
}

pub trait Device {
    /// Create a new shader library.
    fn new_shader_library(
        &self,
        desc: LibraryDesc,
    ) -> Result<crate::backend::Library, CreateLibraryError>;

    /// Create a new render pipeline.
    fn new_render_pipeline(
        &self,
        desc: RenderPipelineDesc,
    ) -> Result<crate::backend::RenderPipeline, CreatePipelineError>;

    /// Create a new buffer with uninitialized contents.
    fn new_buffer(&self, desc: BufferDesc) -> Result<crate::backend::Buffer, OutOfMemory>;

    /// Create a new buffer and initialize it with the given data.
    fn new_buffer_init(&self, desc: BufferInitDesc) -> Result<crate::backend::Buffer, OutOfMemory>;

    /// Create a new image.
    fn new_image(&self, desc: ImageDesc) -> Result<crate::backend::Image, ImageError>;

    /// Create a new surface associated with given window.
    fn new_surface(
        &self,
        window: &impl HasRawWindowHandle,
        display: &impl HasRawDisplayHandle,
    ) -> Result<crate::backend::Surface, SurfaceError>;
}

pub trait Queue {
    /// Get the queue family index.
    fn family(&self) -> u32;

    /// Create a new command encoder associated with this queue.
    /// The encoder must be submitted to the queue it was created from.
    fn new_command_encoder(&mut self) -> Result<crate::backend::CommandEncoder, OutOfMemory>;

    /// Submit command buffers to the queue.
    fn submit<I>(&mut self, command_buffers: I) -> Result<(), QueueError>
    where
        I: IntoIterator<Item = crate::backend::CommandBuffer>;
}

pub trait CommandEncoder {
    /// Returns encoder for copy commands.
    fn copy(&mut self) -> crate::backend::CopyCommandEncoder<'_>;

    /// Starts rendering and returns encoder for render commands.
    fn render(&mut self, desc: RenderPassDesc) -> crate::backend::RenderCommandEncoder<'_>;

    /// Presents the frame to the surface.
    fn present(&mut self, frame: crate::backend::Frame);

    /// Finishes encoding and returns the command buffer.
    fn finish(self) -> Result<crate::backend::CommandBuffer, OutOfMemory>;
}

pub trait CopyCommandEncoder {
    /// Writes data to the buffer.
    fn write_buffer(&mut self, buffer: &crate::backend::Buffer, offset: u64, data: &[u8]);
}

pub trait RenderCommandEncoder {
    /// Synchronizes the access to the resources.
    /// Commands in `before` stages of subsequent commands will be
    /// executed only after commands in `after` stages of previous commands
    /// are finished.
    fn barrier(&mut self, after: RenderStages, before: RenderStages);

    /// Sets the current render pipeline.
    fn with_pipeline(&mut self, pipeline: &crate::backend::RenderPipeline);

    /// Sets arguments for the current pipeline.
    fn with_arguments(&mut self, arguments: &mut impl Arguments, index: u32);

    /// Draws primitives.
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);
}

pub trait Surface {
    /// Acquires next frame from the surface.
    fn next_frame(
        &mut self,
        queue: &mut crate::backend::Queue,
    ) -> Result<crate::backend::Frame, SurfaceError>;
}

pub trait Frame {
    fn image(&self) -> &crate::backend::Image;
}

pub trait Image {
    fn id(&self) -> ImageId;
}

pub trait Buffer {
    /// Write data to the buffer.
    ///
    /// # Safety
    ///
    /// Calling this function is unsafe because
    /// Other threads or GPU may access the same buffer region.
    ///
    /// Use [`CommandEncoder::write_buffer`] to update
    /// buffer in safer way.
    unsafe fn write_unchecked(&self, offset: u64, data: &[u8]);
}

/// A single shader argument with statically known layout.
pub trait Argument: crate::private::Sealed + 'static {
    /// Returns layout of the argument.
    const KIND: ArgumentKind;
}
