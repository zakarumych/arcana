use std::ops::Range;

use ash::vk;
use smallvec::SmallVec;

use crate::generic::{
    Arguments, ClearColor, ClearDepthStencil, DeviceRepr, Extent2, Extent3, LoadOp, Offset2,
    Offset3, OutOfMemory, PipelineStages, RenderPassDesc, StoreOp,
};

use super::{
    access::access_for_stages, format_aspect, from::IntoAsh, handle_host_oom,
    layout::PipelineLayout, refs::Refs, unexpected_error, Buffer, Device, Frame, Image,
    RenderPipeline,
};

pub struct CommandBuffer {
    pub(super) handle: vk::CommandBuffer,
    pub(super) pool: vk::CommandPool,
    pub(super) present: SmallVec<[Frame; 2]>,
    pub(super) refs: Refs,
}

pub struct CommandEncoder {
    device: Device,
    handle: vk::CommandBuffer,
    pool: vk::CommandPool,
    present: SmallVec<[Frame; 2]>,
    refs: Refs,
}

impl CommandEncoder {
    pub(super) fn new(
        device: Device,
        handle: vk::CommandBuffer,
        pool: vk::CommandPool,
        refs: Refs,
    ) -> Self {
        CommandEncoder {
            device,
            handle,
            pool,
            present: SmallVec::new(),
            refs,
        }
    }
}

#[hidden_trait::expose]
impl crate::traits::CommandEncoder for CommandEncoder {
    #[inline(never)]
    fn barrier(&mut self, after: PipelineStages, before: PipelineStages) {
        barrier(&self.device, self.handle, after, before);
    }

    #[inline(never)]
    fn init_image(&mut self, after: PipelineStages, before: PipelineStages, image: &Image) {
        image_barrier(&self.device, self.handle, after, before, image);
        self.refs.add_image(image.clone());
    }

    #[inline(never)]
    fn copy(&mut self) -> CopyCommandEncoder<'_> {
        CopyCommandEncoder {
            device: self.device.clone(),
            handle: self.handle,
            refs: &mut self.refs,
        }
    }

    fn render(&mut self, desc: RenderPassDesc) -> RenderCommandEncoder<'_> {
        let mut extent = vk::Extent2D {
            width: u32::MAX,
            height: u32::MAX,
        };

        let mut color_attachments = Vec::with_capacity(desc.color_attachments.len());
        for color in desc.color_attachments.iter() {
            let format = color.image.format();
            debug_assert!(format.is_color());

            let color_extent: ash::vk::Extent2D = color.image.dimensions().to_2d().into_ash();
            extent.width = extent.width.min(color_extent.width);
            extent.height = extent.height.min(color_extent.height);

            let mut attachment = vk::RenderingAttachmentInfo::builder();

            self.refs.add_image(color.image.clone());

            attachment.image_view = color.image.view_handle();
            attachment.image_layout = vk::ImageLayout::GENERAL;
            attachment.load_op = match color.load {
                LoadOp::Load => vk::AttachmentLoadOp::LOAD,
                LoadOp::Clear(ClearColor(r, g, b, a)) => {
                    attachment.clear_value = vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [r, g, b, a],
                        },
                    };
                    vk::AttachmentLoadOp::CLEAR
                }
                LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
            };
            attachment.store_op = match color.store {
                StoreOp::Store => vk::AttachmentStoreOp::STORE,
                StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
            };
            color_attachments.push(attachment.build());
        }

        let mut info = vk::RenderingInfo::builder().color_attachments(&color_attachments);

        let depth_attachment;
        let stencil_attachment;

        if let Some(depth) = desc.depth_stencil_attachment {
            let format = depth.image.format();
            debug_assert!(format.is_depth() || format.is_stencil());

            let depth_extent: ash::vk::Extent2D = depth.image.dimensions().to_2d().into_ash();
            extent.width = extent.width.min(depth_extent.width);
            extent.height = extent.height.min(depth_extent.height);

            if format.is_depth() {
                let mut attachment = vk::RenderingAttachmentInfo::builder();

                self.refs.add_image(depth.image.clone());

                attachment.image_view = depth.image.view_handle();
                attachment.image_layout = vk::ImageLayout::GENERAL;
                attachment.load_op = match depth.load {
                    LoadOp::Load => vk::AttachmentLoadOp::LOAD,
                    LoadOp::Clear(ClearDepthStencil { depth, stencil }) => {
                        attachment.clear_value = vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
                        };
                        vk::AttachmentLoadOp::CLEAR
                    }
                    LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
                };
                attachment.store_op = match depth.store {
                    StoreOp::Store => vk::AttachmentStoreOp::STORE,
                    StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
                };
                depth_attachment = attachment;
                info.p_depth_attachment = &*depth_attachment;
            }
            if format.is_stencil() {
                let mut attachment = vk::RenderingAttachmentInfo::builder();

                self.refs.add_image(depth.image.clone());

                attachment.image_view = depth.image.view_handle();
                attachment.load_op = match depth.load {
                    LoadOp::Load => vk::AttachmentLoadOp::LOAD,
                    LoadOp::Clear(ClearDepthStencil { depth, stencil }) => {
                        attachment.clear_value = vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
                        };
                        vk::AttachmentLoadOp::CLEAR
                    }
                    LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
                };
                attachment.store_op = match depth.store {
                    StoreOp::Store => vk::AttachmentStoreOp::STORE,
                    StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
                };
                stencil_attachment = attachment;
                info.p_stencil_attachment = &*stencil_attachment;
            }
        }

        unsafe {
            self.device.ash().cmd_begin_rendering(
                self.handle,
                &info
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent,
                    })
                    .layer_count(1),
            )
        }

        RenderCommandEncoder {
            device: self.device.clone(),
            handle: self.handle,
            current_layout: None,
            refs: &mut self.refs,
        }
    }

    #[inline(never)]
    fn present(&mut self, frame: Frame, after: PipelineStages) {
        unsafe {
            self.device.ash().cmd_pipeline_barrier(
                self.handle,
                ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE | after.into_ash(),
                ash::vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[ash::vk::ImageMemoryBarrier::builder()
                    .src_access_mask(access_for_stages(after))
                    .dst_access_mask(ash::vk::AccessFlags::empty())
                    .old_layout(ash::vk::ImageLayout::GENERAL)
                    .new_layout(ash::vk::ImageLayout::PRESENT_SRC_KHR)
                    .image(frame.image().handle())
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .build()],
            )
        }

        self.refs.add_image(frame.image().clone());
        self.present.push(frame);
    }

    #[inline(never)]
    fn finish(self) -> Result<CommandBuffer, OutOfMemory> {
        let result = unsafe { self.device.ash().end_command_buffer(self.handle) };
        result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
            _ => unexpected_error(err),
        })?;

        Ok(CommandBuffer {
            handle: self.handle,
            pool: self.pool,
            present: self.present,
            refs: self.refs,
        })
    }
}

pub struct RenderCommandEncoder<'a> {
    device: Device,
    handle: vk::CommandBuffer,
    refs: &'a mut Refs,
    current_layout: Option<PipelineLayout>,
}

impl RenderCommandEncoder<'_> {
    #[inline(never)]
    pub(super) fn handle(&self) -> vk::CommandBuffer {
        self.handle
    }

    #[inline(never)]
    pub(super) fn device(&self) -> &Device {
        &self.device
    }

    #[inline(never)]
    pub(super) fn current_layout(&self) -> Option<&PipelineLayout> {
        self.current_layout.as_ref()
    }

    #[inline(never)]
    pub(super) fn refs_mut(&mut self) -> &mut Refs {
        &mut self.refs
    }
}

impl Drop for RenderCommandEncoder<'_> {
    #[inline(never)]
    fn drop(&mut self) {
        unsafe { self.device.ash().cmd_end_rendering(self.handle) }
    }
}

#[hidden_trait::expose]
impl crate::traits::RenderCommandEncoder for RenderCommandEncoder<'_> {
    #[inline(never)]
    fn with_pipeline(&mut self, pipeline: &RenderPipeline) {
        unsafe {
            self.device.ash().cmd_bind_pipeline(
                self.handle,
                ash::vk::PipelineBindPoint::GRAPHICS,
                pipeline.handle(),
            );
        }
        self.current_layout = Some(pipeline.layout().clone());
        self.refs.add_render_pipeline(pipeline.clone());
    }

    #[inline(never)]
    fn with_viewport(&mut self, offset: Offset3<f32>, extent: Extent3<f32>) {
        unsafe {
            self.device.ash().cmd_set_viewport(
                self.handle,
                0,
                &[ash::vk::Viewport::builder()
                    .x(offset.x())
                    .y(offset.y())
                    .width(extent.width())
                    .height(extent.height())
                    .min_depth(offset.z())
                    .max_depth(extent.depth())
                    .build()],
            );
        }
    }

    #[inline(never)]
    fn with_scissor(&mut self, offset: Offset2<i32>, extent: Extent2<u32>) {
        unsafe {
            self.device.ash().cmd_set_scissor(
                self.handle,
                0,
                &[ash::vk::Rect2D::builder()
                    .offset(ash::vk::Offset2D {
                        x: offset.x(),
                        y: offset.y(),
                    })
                    .extent(ash::vk::Extent2D {
                        width: extent.width(),
                        height: extent.height(),
                    })
                    .build()],
            );
        }
    }

    #[inline(never)]
    fn with_arguments(&mut self, group: u32, arguments: &impl Arguments) {
        arguments.bind_render(group, self);
    }

    #[inline(never)]
    fn with_constants(&mut self, constants: &impl DeviceRepr) {
        let Some(layout) = self.current_layout.as_ref() else {
            panic!("Constants binding requires a pipeline to be bound to the encoder");
        };

        let data = constants.as_repr();

        unsafe {
            self.device.ash().cmd_push_constants(
                self.handle,
                layout.handle(),
                ash::vk::ShaderStageFlags::ALL,
                0,
                bytemuck::bytes_of(&data),
            )
        }
    }

    #[inline(never)]
    fn bind_vertex_buffers(&mut self, start: u32, buffers: &[(&crate::backend::Buffer, usize)]) {
        let mut handles = smallvec::SmallVec::<[_; 8]>::with_capacity(buffers.len());
        let mut offsets = smallvec::SmallVec::<[_; 8]>::with_capacity(buffers.len());
        for &(buffer, offset) in buffers.iter() {
            handles.push(buffer.handle());
            offsets.push(offset as u64);
            self.refs.add_buffer(buffer.clone());
        }

        unsafe {
            self.device
                .ash()
                .cmd_bind_vertex_buffers(self.handle, start, &handles, &offsets)
        }
    }

    #[inline(never)]
    fn bind_index_buffer(&mut self, buffer: &crate::backend::Buffer, offset: usize) {
        unsafe {
            self.device.ash().cmd_bind_index_buffer(
                self.handle,
                buffer.handle(),
                offset as u64,
                vk::IndexType::UINT32,
            )
        }
        self.refs.add_buffer(buffer.clone());
    }

    #[inline(never)]
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        unsafe {
            self.device.ash().cmd_draw(
                self.handle,
                vertices.end - vertices.start,
                instances.end - instances.start,
                vertices.start,
                instances.start,
            );
        }
    }

    #[inline(never)]
    fn draw_indexed(&mut self, vertex_offset: i32, indices: Range<u32>, instances: Range<u32>) {
        unsafe {
            self.device.ash().cmd_draw_indexed(
                self.handle,
                indices.end - indices.start,
                instances.end - instances.start,
                indices.start,
                vertex_offset,
                instances.start,
            );
        }
    }
}

pub struct CopyCommandEncoder<'a> {
    device: Device,
    handle: vk::CommandBuffer,
    refs: &'a mut Refs,
}

#[hidden_trait::expose]
impl crate::traits::CopyCommandEncoder for CopyCommandEncoder<'_> {
    #[inline(never)]
    fn barrier(&mut self, after: PipelineStages, before: PipelineStages) {
        barrier(&self.device, self.handle, after, before);
    }

    #[inline(never)]
    fn init_image(&mut self, after: PipelineStages, before: PipelineStages, image: &Image) {
        image_barrier(&self.device, self.handle, after, before, image);
        self.refs.add_image(image.clone());
    }

    #[inline(never)]
    fn copy_buffer_to_image(
        &mut self,
        src: &Buffer,
        start: usize,
        bytes_per_line: usize,
        bytes_per_plane: usize,
        dst: &Image,
        offset: Offset3<u32>,
        extent: Extent3<u32>,
        layers: Range<u32>,
        level: u32,
    ) {
        let texel_size = dst.format().size();
        debug_assert_eq!(bytes_per_line % texel_size, 0);
        debug_assert_eq!(bytes_per_plane % texel_size, 0);
        let texel_per_line = bytes_per_line / texel_size;
        let texel_per_plane = bytes_per_plane / texel_size;

        self.refs.add_buffer(src.clone());
        self.refs.add_image(dst.clone());

        unsafe {
            self.device.ash().cmd_copy_buffer_to_image(
                self.handle,
                src.handle(),
                dst.handle(),
                ash::vk::ImageLayout::GENERAL,
                &[vk::BufferImageCopy {
                    buffer_offset: start as u64,
                    buffer_row_length: texel_per_line as u32,
                    buffer_image_height: texel_per_plane as u32,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: format_aspect(dst.format()),
                        mip_level: dst.base_level() + level,
                        base_array_layer: dst.base_layer() + layers.start,
                        layer_count: layers.end - layers.start,
                    },
                    image_offset: vk::Offset3D {
                        x: offset.x() as i32,
                        y: offset.y() as i32,
                        z: offset.z() as i32,
                    },
                    image_extent: vk::Extent3D {
                        width: extent.width(),
                        height: extent.height(),
                        depth: extent.depth(),
                    },
                }],
            )
        }
    }

    #[inline(never)]
    fn copy_image_region(
        &mut self,
        src: &Image,
        src_offset: Offset3<u32>,
        src_base_layer: u32,
        dst: &Image,
        dst_offset: Offset3<u32>,
        dst_base_layer: u32,
        extent: Extent3<u32>,
        layers: u32,
    ) {
        self.refs.add_image(src.clone());
        self.refs.add_image(dst.clone());
        unsafe {
            self.device.ash().cmd_copy_image(
                self.handle,
                src.handle(),
                ash::vk::ImageLayout::GENERAL,
                dst.handle(),
                ash::vk::ImageLayout::GENERAL,
                &[vk::ImageCopy {
                    src_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: format_aspect(src.format()),
                        mip_level: src.base_level(),
                        base_array_layer: src.base_layer() + src_base_layer,
                        layer_count: layers,
                    },
                    src_offset: vk::Offset3D {
                        x: src_offset.x() as i32,
                        y: src_offset.y() as i32,
                        z: src_offset.z() as i32,
                    },
                    dst_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: format_aspect(dst.format()),
                        mip_level: dst.base_level(),
                        base_array_layer: dst.base_layer() + dst_base_layer,
                        layer_count: layers,
                    },
                    dst_offset: vk::Offset3D {
                        x: dst_offset.x() as i32,
                        y: dst_offset.y() as i32,
                        z: dst_offset.z() as i32,
                    },
                    extent: vk::Extent3D {
                        width: extent.width(),
                        height: extent.height(),
                        depth: extent.depth(),
                    },
                }],
            )
        }
    }

    #[inline(never)]
    fn write_buffer_raw(&mut self, buffer: &Buffer, offset: usize, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        self.refs.add_buffer(buffer.clone());

        const CHUNK_SIZE: usize = 65536;

        let full_chunks = data.len() / CHUNK_SIZE;

        for i in 0..full_chunks {
            unsafe {
                self.device.ash().cmd_update_buffer(
                    self.handle,
                    buffer.handle(),
                    (offset + i * CHUNK_SIZE) as u64,
                    &data[i * CHUNK_SIZE..(i + 1) * CHUNK_SIZE],
                )
            }
        }

        let remainder = data.len() % CHUNK_SIZE;
        if remainder > 0 {
            unsafe {
                self.device.ash().cmd_update_buffer(
                    self.handle,
                    buffer.handle(),
                    (offset + full_chunks * CHUNK_SIZE) as u64,
                    &data[full_chunks * CHUNK_SIZE..],
                )
            }
        }
    }

    #[inline(never)]
    fn write_buffer(
        &mut self,
        buffer: &crate::backend::Buffer,
        offset: usize,
        data: &impl bytemuck::Pod,
    ) {
        self.write_buffer_raw(buffer, offset, bytemuck::bytes_of(data))
    }

    #[inline(never)]
    fn write_buffer_slice(
        &mut self,
        buffer: &crate::backend::Buffer,
        offset: usize,
        data: &[impl bytemuck::Pod],
    ) {
        self.write_buffer_raw(buffer, offset, bytemuck::cast_slice(data))
    }
}

#[inline(never)]
fn barrier(
    device: &Device,
    handle: ash::vk::CommandBuffer,
    after: PipelineStages,
    before: PipelineStages,
) {
    unsafe {
        device.ash().cmd_pipeline_barrier(
            handle,
            ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE | after.into_ash(),
            ash::vk::PipelineStageFlags::TOP_OF_PIPE | before.into_ash(),
            vk::DependencyFlags::empty(),
            &[vk::MemoryBarrier::builder()
                .src_access_mask(access_for_stages(after))
                .dst_access_mask(access_for_stages(before))
                .build()],
            &[],
            &[],
        )
    }
}

#[inline(never)]
fn image_barrier(
    device: &Device,
    handle: ash::vk::CommandBuffer,
    after: PipelineStages,
    before: PipelineStages,
    image: &Image,
) {
    let mut aspect_mask = ash::vk::ImageAspectFlags::empty();
    if image.format().is_color() {
        aspect_mask |= ash::vk::ImageAspectFlags::COLOR;
    }
    if image.format().is_depth() {
        aspect_mask |= ash::vk::ImageAspectFlags::DEPTH;
    }
    if image.format().is_stencil() {
        aspect_mask |= ash::vk::ImageAspectFlags::STENCIL;
    }

    unsafe {
        device.ash().cmd_pipeline_barrier(
            handle,
            ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE | after.into_ash(),
            ash::vk::PipelineStageFlags::TOP_OF_PIPE | before.into_ash(),
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[ash::vk::ImageMemoryBarrier::builder()
                .src_access_mask(access_for_stages(after))
                .dst_access_mask(access_for_stages(before))
                .old_layout(ash::vk::ImageLayout::UNDEFINED)
                .new_layout(ash::vk::ImageLayout::GENERAL)
                .image(image.handle())
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask,
                    base_mip_level: 0,
                    level_count: image.levels(),
                    base_array_layer: 0,
                    layer_count: image.layers(),
                })
                .build()],
        )
    }
}
