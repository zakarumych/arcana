use std::ops::Range;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::generic::{
    Arguments, BufferDesc, BufferInitDesc, Capabilities, Constants, CreateError,
    CreateLibraryError, CreatePipelineError, DeviceDesc, Extent2, Extent3, ImageDesc,
    ImageDimensions, ImageError, LibraryDesc, Offset2, Offset3, OutOfMemory, PipelineStages,
    PixelFormat, QueueError, RenderPassDesc, RenderPipelineDesc, SamplerDesc, SurfaceError,
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

    /// Create a new sampler.
    fn new_sampler(&self, desc: SamplerDesc) -> Result<crate::backend::Sampler, OutOfMemory>;

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
    ///
    /// If `check_point` is `true`, inserts a checkpoint into queue and check previous checkpoints.
    /// Checkpoints are required to synchronize resource use and reclamation.
    fn submit<I>(&mut self, command_buffers: I, check_point: bool) -> Result<(), QueueError>
    where
        I: IntoIterator<Item = crate::backend::CommandBuffer>;
}

pub trait CommandEncoder {
    /// Synchronizes the access to the resources.
    /// Commands in `before` stages of subsequent commands will be
    /// executed only after commands in `after` stages of previous commands
    /// are finished.
    fn barrier(&mut self, after: PipelineStages, before: PipelineStages);

    /// Synchronizes the access to the image.
    /// Commands in `before` stages of subsequent commands will be
    /// executed only after commands in `after` stages of previous commands
    /// are finished.
    /// Image content is discarded.
    fn init_image(
        &mut self,
        after: PipelineStages,
        before: PipelineStages,
        image: &crate::backend::Image,
    );

    /// Returns encoder for copy commands.
    fn copy(&mut self) -> crate::backend::CopyCommandEncoder<'_>;

    /// Starts rendering and returns encoder for render commands.
    fn render(&mut self, desc: RenderPassDesc) -> crate::backend::RenderCommandEncoder<'_>;

    /// Presents the frame to the surface.
    fn present(&mut self, frame: crate::backend::Frame, after: PipelineStages);

    /// Finishes encoding and returns the command buffer.
    fn finish(self) -> Result<crate::backend::CommandBuffer, OutOfMemory>;
}

pub trait CopyCommandEncoder {
    /// Synchronizes the access to the resources.
    /// Commands in `before` stages of subsequent commands will be
    /// executed only after commands in `after` stages of previous commands
    /// are finished.
    fn barrier(&mut self, after: PipelineStages, before: PipelineStages);

    /// Writes data to the buffer.
    fn write_buffer(&mut self, buffer: &crate::backend::Buffer, offset: u64, data: &[u8]);
}

pub trait RenderCommandEncoder {
    /// Sets the current render pipeline.
    fn with_pipeline(&mut self, pipeline: &crate::backend::RenderPipeline);

    fn with_viewport(&mut self, offset: Offset3<f32>, extent: Extent3<f32>);

    fn with_scissor(&mut self, offset: Offset2<i32>, extent: Extent2<u32>);

    /// Sets arguments group for the current pipeline.
    fn with_arguments(&mut self, group: u32, arguments: &impl Arguments);

    /// Sets constants for the current pipeline.
    fn with_constants(&mut self, constants: &impl Constants);

    /// Draws primitives.
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);
}

pub trait Surface {
    /// Acquires next frame from the surface.
    fn next_frame(
        &mut self,
        queue: &mut crate::backend::Queue,
        before: PipelineStages,
    ) -> Result<crate::backend::Frame, SurfaceError>;
}

pub trait Frame {
    fn image(&self) -> &crate::backend::Image;
}

pub trait Image {
    /// Returns the pixel format of the image.
    fn format(&self) -> PixelFormat;

    /// Returns the dimensions of the image.
    fn dimensions(&self) -> ImageDimensions;

    /// Returns the number of layers in the image.
    fn layers(&self) -> u32;

    /// Returns the number of mip levels in the image.
    fn levels(&self) -> u32;

    /// Returns `true` if the buffer is not shared,
    /// meaning that there are no other references to the buffer
    /// including references that tracks that GPU may be using the buffer.
    ///
    /// If this method returns `true` then it is safe to write to the buffer
    /// from host and use in any way.
    ///
    /// If old content is not needed then no synchronization is required.
    /// Otherwise memory barrier with is required.
    fn detached(&self) -> bool;
}

pub trait Buffer {
    /// Returns the size of the buffer in bytes.
    fn size(&self) -> usize;

    /// Returns `true` if the buffer is not shared,
    /// meaning that there are no other references to the buffer
    /// including references that tracks that GPU may be using the buffer.
    ///
    /// If this method returns `true` then it is safe to write to the buffer
    /// from host and use in any way.
    ///
    /// If old content is not needed then no synchronization is required.
    /// Otherwise memory barrier with is required.
    fn detached(&self) -> bool;

    /// Write data to the buffer.
    ///
    /// # Safety
    ///
    /// Calling this function is unsafe because
    /// Other threads or GPU may access the same buffer region.
    ///
    /// Use [`CommandEncoder::write_buffer`] to update
    /// buffer in safer way.
    unsafe fn write_unchecked(&mut self, offset: u64, data: &[u8]);
}
