use std::marker::PhantomData;

use ash::vk;

use crate::generic::{ClearColor, ClearDepthStencil, LoadOp, OutOfMemory, RenderPassDesc, StoreOp};

use super::{device::WeakDevice, handle_host_oom, unexpected_error, Device};

pub struct CommandBuffer {
    handle: vk::CommandBuffer,
}

impl CommandBuffer {
    pub(super) fn handle(self) -> vk::CommandBuffer {
        self.handle
    }
}

pub struct CommandEncoder {
    owner: WeakDevice,
    handle: vk::CommandBuffer,
}

impl CommandEncoder {
    pub(super) fn new(owner: WeakDevice, handle: vk::CommandBuffer) -> Self {
        CommandEncoder { owner, handle }
    }
}

#[hidden_trait::expose]
impl crate::traits::CommandEncoder for CommandEncoder {
    fn barrier(&mut self) {
        let Some(device) = self.owner.upgrade() else {
            return;
        };

        unsafe {
            device.ash().cmd_pipeline_barrier(
                self.handle,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::from_raw(0b1_1111_1111_1111_1111))
                    .dst_access_mask(vk::AccessFlags::from_raw(0b1_1111_1111_1111_1111))
                    .build()],
                &[],
                &[],
            )
        }
    }

    fn render(&mut self, desc: RenderPassDesc) -> RenderCommandEncoder<'_> {
        let Some(device) = self.owner.upgrade() else {
            return RenderCommandEncoder {
                device: None,
                handle: self.handle,
                _marker: PhantomData,
            };
        };

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
                    device.ash(),
                    color.level..color.level + 1,
                    color.layer..color.layer + 1,
                )
                .unwrap();

            attachment.image_view = view;
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
                        device.ash(),
                        depth.level..depth.level + 1,
                        depth.layer..depth.layer + 1,
                    )
                    .unwrap();

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
                depth_attachment = attachment;
                info.p_depth_attachment = &*depth_attachment;
            }
            if format.is_stencil() {
                let mut attachment = vk::RenderingAttachmentInfo::builder();

                let view = depth
                    .image
                    .view(
                        device.ash(),
                        depth.level..depth.level + 1,
                        depth.layer..depth.layer + 1,
                    )
                    .unwrap();

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
            device.ash().cmd_begin_rendering(
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
            device: Some(device),
            handle: self.handle,
            _marker: PhantomData,
        }
    }

    fn finish(self) -> Result<CommandBuffer, OutOfMemory> {
        if let Some(device) = self.owner.upgrade() {
            let result = unsafe { device.ash().end_command_buffer(self.handle) };
            result.map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
                _ => unexpected_error(err),
            })?;
        }
        Ok(CommandBuffer {
            handle: self.handle,
        })
    }
}

pub struct RenderCommandEncoder<'a> {
    device: Option<Device>,
    handle: vk::CommandBuffer,
    _marker: PhantomData<&'a mut CommandBuffer>,
}

impl Drop for RenderCommandEncoder<'_> {
    fn drop(&mut self) {
        if let Some(device) = &self.device {
            unsafe { device.ash().cmd_end_rendering(self.handle) }
        }
    }
}

impl crate::traits::RenderCommandEncoder for RenderCommandEncoder<'_> {}
