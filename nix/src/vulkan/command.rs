use std::ops::Range;

use ash::vk;

use crate::generic::{
    Arguments, ClearColor, ClearDepthStencil, Constants, Extent2, Extent3, LoadOp, Offset2,
    Offset3, OutOfMemory, PipelineStages, RenderPassDesc, StoreOp,
};

use super::{
    access::access_for_stages, from::IntoAsh, handle_host_oom, layout::PipelineLayout, refs::Refs,
    unexpected_error, Buffer, Device, Frame, Image, RenderPipeline,
};

pub struct CommandBuffer {
    handle: vk::CommandBuffer,
    present: Vec<Frame>,
    refs: Refs,
}

impl CommandBuffer {
    pub(super) fn deconstruct(self) -> (vk::CommandBuffer, Vec<Frame>, Refs) {
        (self.handle, self.present, self.refs)
    }
}

pub struct CommandEncoder {
    device: Device,
    handle: vk::CommandBuffer,
    present: Vec<Frame>,
    refs: Refs,
}

impl CommandEncoder {
    pub(super) fn new(device: Device, handle: vk::CommandBuffer, refs: Refs) -> Self {
        CommandEncoder {
            device,
            handle,
            present: Vec::new(),
            refs,
        }
    }
}

#[hidden_trait::expose]
impl crate::traits::CommandEncoder for CommandEncoder {
    #[inline(always)]
    fn barrier(&mut self, after: PipelineStages, before: PipelineStages) {
        barrier(&self.device, self.handle, after, before);
    }

    #[inline(always)]
    fn init_image(&mut self, after: PipelineStages, before: PipelineStages, image: &Image) {
        image_barrier(&self.device, self.handle, after, before, image);
        self.refs.add_image(image.clone());
    }

    #[inline]
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

            let color_extent = color.image.extent_2d();
            extent.width = extent.width.min(color_extent.width);
            extent.height = extent.height.min(color_extent.height);

            let mut attachment = vk::RenderingAttachmentInfo::builder();

            let view = color
                .image
                .view(
                    self.device.ash(),
                    color.level..color.level + 1,
                    color.layer..color.layer + 1,
                )
                .unwrap();

            self.refs.add_image(color.image.clone());

            attachment.image_view = view;
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

            let depth_extent = depth.image.extent_2d();
            extent.width = extent.width.min(depth_extent.width);
            extent.height = extent.height.min(depth_extent.height);

            if format.is_depth() {
                let mut attachment = vk::RenderingAttachmentInfo::builder();

                let view = depth
                    .image
                    .view(
                        self.device.ash(),
                        depth.level..depth.level + 1,
                        depth.layer..depth.layer + 1,
                    )
                    .unwrap();

                self.refs.add_image(depth.image.clone());

                attachment.image_view = view;
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

                let view = depth
                    .image
                    .view(
                        self.device.ash(),
                        depth.level..depth.level + 1,
                        depth.layer..depth.layer + 1,
                    )
                    .unwrap();

                self.refs.add_image(depth.image.clone());

                attachment.image_view = view;
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

    #[inline(always)]
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

    #[inline]
    fn finish(self) -> Result<CommandBuffer, OutOfMemory> {
        let result = unsafe { self.device.ash().end_command_buffer(self.handle) };
        result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
            _ => unexpected_error(err),
        })?;

        Ok(CommandBuffer {
            handle: self.handle,
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
    #[inline(always)]
    pub(super) fn handle(&self) -> vk::CommandBuffer {
        self.handle
    }

    #[inline(always)]
    pub(super) fn device(&self) -> &Device {
        &self.device
    }

    #[inline(always)]
    pub(super) fn current_layout(&self) -> Option<&PipelineLayout> {
        self.current_layout.as_ref()
    }

    #[inline(always)]
    pub(super) fn refs_mut(&mut self) -> &mut Refs {
        &mut self.refs
    }
}

impl Drop for RenderCommandEncoder<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { self.device.ash().cmd_end_rendering(self.handle) }
    }
}

#[hidden_trait::expose]
impl crate::traits::RenderCommandEncoder for RenderCommandEncoder<'_> {
    #[inline]
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

    #[inline(always)]
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

    #[inline(always)]
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
    fn with_constants(&mut self, constants: &impl Constants) {
        let Some(layout) = self.current_layout.as_ref() else {
            panic!("Constants binding requires a pipeline to be bound to the encoder");
        };

        let data = constants.as_pod();

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

    #[inline(always)]
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
}

pub struct CopyCommandEncoder<'a> {
    device: Device,
    handle: vk::CommandBuffer,
    refs: &'a mut Refs,
}

impl Drop for CopyCommandEncoder<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { self.device.ash().cmd_end_rendering(self.handle) }
    }
}

#[hidden_trait::expose]
impl crate::traits::CopyCommandEncoder for CopyCommandEncoder<'_> {
    #[inline(always)]
    fn barrier(&mut self, after: PipelineStages, before: PipelineStages) {
        barrier(&self.device, self.handle, after, before);
    }

    #[inline]
    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8]) {
        self.refs.add_buffer(buffer.clone());
        unsafe {
            self.device
                .ash()
                .cmd_update_buffer(self.handle, buffer.handle(), offset, data)
        }
    }
}

#[inline]
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

#[inline]
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
