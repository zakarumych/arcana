use std::{marker::PhantomData, ops::Range};

use crate::generic::{
    Arguments, ClearColor, ClearDepthStencil, LoadOp, OutOfMemory, RenderPassDesc, RenderStages,
    ShaderStage, ShaderStages, StoreOp,
};

use super::{argument::MetalArgument, out_of_bounds, Buffer, Frame, RenderPipeline};

pub struct CommandBuffer {
    buffer: metal::CommandBuffer,
}

impl CommandBuffer {
    pub(super) fn commit(self) {
        self.buffer.commit();
    }
}

pub struct CommandEncoder {
    device: metal::Device,
    buffer: metal::CommandBuffer,
}

impl CommandEncoder {
    pub(super) fn new(device: metal::Device, buffer: metal::CommandBuffer) -> Self {
        CommandEncoder { device, buffer }
    }
}

#[hidden_trait::expose]
impl crate::traits::CommandEncoder for CommandEncoder {
    fn copy(&mut self) -> CopyCommandEncoder {
        let encoder = self.buffer.new_blit_command_encoder();
        CopyCommandEncoder {
            device: &mut self.device,
            encoder: encoder.to_owned(),
            _marker: PhantomData,
        }
    }

    fn render(&mut self, desc: RenderPassDesc) -> RenderCommandEncoder<'_> {
        let mdesc = metal::RenderPassDescriptor::new();
        let color_attachments = mdesc.color_attachments();
        for (idx, color) in desc.color_attachments.iter().enumerate() {
            let format = color.image.format();
            debug_assert!(format.is_color());

            let attachment = metal::RenderPassColorAttachmentDescriptor::new();
            attachment.set_texture(Some(color.image.metal()));
            attachment.set_load_action(match color.load {
                LoadOp::Load => metal::MTLLoadAction::Load,
                LoadOp::Clear(ClearColor(r, g, b, a)) => {
                    attachment.set_clear_color(metal::MTLClearColor {
                        red: r.into(),
                        green: g.into(),
                        blue: b.into(),
                        alpha: a.into(),
                    });
                    metal::MTLLoadAction::Clear
                }
                LoadOp::DontCare => metal::MTLLoadAction::DontCare,
            });
            attachment.set_store_action(match color.store {
                StoreOp::Store => metal::MTLStoreAction::Store,
                StoreOp::DontCare => metal::MTLStoreAction::DontCare,
            });
            attachment.set_level(color.level as _);
            attachment.set_slice(color.layer as _);
            color_attachments.set_object_at(idx as _, Some(&attachment));
        }

        if let Some(depth) = desc.depth_stencil_attachment {
            let format = depth.image.format();
            debug_assert!(format.is_depth() || format.is_stencil());

            if format.is_depth() {
                let attachment = mdesc.depth_attachment().unwrap();
                attachment.set_texture(Some(depth.image.metal()));
                attachment.set_load_action(match depth.load {
                    LoadOp::Load => metal::MTLLoadAction::Load,
                    LoadOp::Clear(ClearDepthStencil { depth, .. }) => {
                        attachment.set_clear_depth(depth.into());
                        metal::MTLLoadAction::Clear
                    }
                    LoadOp::DontCare => metal::MTLLoadAction::DontCare,
                });
                attachment.set_store_action(match depth.store {
                    StoreOp::Store => metal::MTLStoreAction::Store,
                    StoreOp::DontCare => metal::MTLStoreAction::DontCare,
                });
                attachment.set_level(depth.level as _);
                attachment.set_slice(depth.layer as _);
            }
            if format.is_stencil() {
                let attachment = mdesc.stencil_attachment().unwrap();
                attachment.set_texture(Some(depth.image.metal()));
                attachment.set_load_action(match depth.load {
                    LoadOp::Load => metal::MTLLoadAction::Load,
                    LoadOp::Clear(ClearDepthStencil { stencil, .. }) => {
                        attachment.set_clear_stencil(stencil.into());
                        metal::MTLLoadAction::Clear
                    }
                    LoadOp::DontCare => metal::MTLLoadAction::DontCare,
                });
                attachment.set_store_action(match depth.store {
                    StoreOp::Store => metal::MTLStoreAction::Store,
                    StoreOp::DontCare => metal::MTLStoreAction::DontCare,
                });
                attachment.set_level(depth.level as _);
                attachment.set_slice(depth.layer as _);
            }
        }

        let encoder = self.buffer.new_render_command_encoder(&mdesc);
        RenderCommandEncoder {
            encoder: encoder.to_owned(),
            primitive: metal::MTLPrimitiveType::Triangle,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    fn present(&mut self, frame: Frame) {
        self.buffer.present_drawable(frame.drawable());
    }

    #[inline(always)]
    fn finish(self) -> Result<CommandBuffer, OutOfMemory> {
        Ok(CommandBuffer {
            buffer: self.buffer,
        })
    }
}

pub struct CopyCommandEncoder<'a> {
    device: &'a mut metal::DeviceRef,
    encoder: metal::BlitCommandEncoder,
    _marker: PhantomData<&'a mut CommandBuffer>,
}

#[hidden_trait::expose]
impl crate::traits::CopyCommandEncoder for CopyCommandEncoder<'_> {
    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8]) {
        let length = buffer.metal().length();
        let mut data_len = 0;
        let fits = match u64::try_from(data.len()) {
            Ok(len) => {
                data_len = len;
                match offset.checked_add(len) {
                    Some(end) => end <= length,
                    None => false,
                }
            }
            Err(_) => false,
        };
        if !fits {
            out_of_bounds();
        }

        let staged = self.device.new_buffer_with_data(
            data.as_ptr().cast(),
            data_len,
            metal::MTLResourceOptions::StorageModePrivate,
        );

        self.encoder
            .copy_from_buffer(&staged, 0, buffer.metal(), offset, data_len);
    }
}

pub struct RenderCommandEncoder<'a> {
    encoder: metal::RenderCommandEncoder,
    primitive: metal::MTLPrimitiveType,
    _marker: PhantomData<&'a mut CommandBuffer>,
}

impl Drop for RenderCommandEncoder<'_> {
    fn drop(&mut self) {
        self.encoder.end_encoding();
    }
}

#[hidden_trait::expose]
impl crate::traits::RenderCommandEncoder for RenderCommandEncoder<'_> {
    fn barrier(&mut self, _after: RenderStages, _before: RenderStages) {}

    fn with_pipeline(&mut self, pipeline: &RenderPipeline) {
        self.encoder.set_render_pipeline_state(pipeline.metal());
        self.primitive = pipeline.primitive();
    }

    fn with_arguments(&mut self, arguments: &mut impl Arguments, index: u32) {
        let mut raw = [0u8; 128];
        assert!(arguments.raw_len() <= raw.len());
        arguments.fill(raw);

        self.encoder.set_vertex_bytes(
            index as _,
            arguments.as_bytes().as_ptr().cast(),
            arguments.as_bytes().len() as _,
        );
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        if vertices.end <= vertices.start {
            // Rendering no vertices is a no-op
            return;
        }
        if instances.end <= instances.start {
            // Rendering no instances is a no-op
            return;
        }

        if instances.end - 1 == instances.start {
            // Rendering single instance
            self.encoder.draw_primitives(
                self.primitive,
                vertices.start.into(),
                (vertices.end - vertices.start).into(),
            );
        } else if instances.start == 0 {
            // Rendering multiple instances
            self.encoder.draw_primitives_instanced(
                self.primitive,
                vertices.start.into(),
                (vertices.end - vertices.start).into(),
                instances.end.into(),
            );
        } else {
            // Rendering multiple instances with a base instance
            self.encoder.draw_primitives_instanced_base_instance(
                self.primitive,
                vertices.start.into(),
                (vertices.end - vertices.start).into(),
                (instances.end - instances.start).into(),
                instances.start.into(),
            );
        }
    }
}
