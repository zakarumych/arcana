use std::{marker::PhantomData, mem::size_of, ops::Range};

use metal::NSUInteger;
use smallvec::SmallVec;

use crate::generic::{
    Arguments, ClearColor, ClearDepthStencil, Constants, Extent2, Extent3, LoadOp, Offset2,
    Offset3, OutOfMemory, PipelineStages, RenderPassDesc, ShaderStage, ShaderStages, StoreOp,
};

use super::{out_of_bounds, Buffer, Frame, Image, RenderPipeline};

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

// #[hidden_trait::expose]
impl crate::traits::CommandEncoder for CommandEncoder {
    fn barrier(&mut self, _after: PipelineStages, _before: PipelineStages) {}

    fn init_image(&mut self, _after: PipelineStages, _before: PipelineStages, _image: &Image) {}

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
            attachment.set_level(0);
            attachment.set_slice(0);
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
                attachment.set_level(0);
                attachment.set_slice(0);
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
                attachment.set_level(0);
                attachment.set_slice(0);
            }
        }

        let encoder = self.buffer.new_render_command_encoder(&mdesc);
        RenderCommandEncoder {
            encoder: encoder.to_owned(),
            primitive: metal::MTLPrimitiveType::Triangle,
            index_buffer: None,
            index_buffer_offset: 0,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    fn present(&mut self, frame: Frame, _after: PipelineStages) {
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

// #[hidden_trait::expose]
impl crate::traits::CopyCommandEncoder for CopyCommandEncoder<'_> {
    fn barrier(&mut self, _after: PipelineStages, _before: PipelineStages) {}

    fn init_image(&mut self, _after: PipelineStages, _before: PipelineStages, _image: &Image) {}

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
        debug_assert!(layers.end > layers.start);
        debug_assert!(layers.end == layers.start + 1);

        self.encoder.copy_from_buffer_to_texture(
            src.metal(),
            start as NSUInteger,
            bytes_per_line as NSUInteger,
            bytes_per_plane as NSUInteger,
            metal::MTLSize {
                width: extent.width() as NSUInteger,
                height: extent.height() as NSUInteger,
                depth: extent.depth() as NSUInteger,
            },
            dst.metal(),
            layers.start as NSUInteger,
            level as NSUInteger,
            metal::MTLOrigin {
                x: offset.x() as NSUInteger,
                y: offset.y() as NSUInteger,
                z: offset.z() as NSUInteger,
            },
            metal::MTLBlitOption::empty(),
        );
    }

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
        for layer in 0..layers {
            self.encoder.copy_from_texture(
                src.metal(),
                (src_base_layer + layer) as NSUInteger,
                src.metal().parent_relative_level(),
                metal::MTLOrigin {
                    x: src_offset.x() as NSUInteger,
                    y: src_offset.y() as NSUInteger,
                    z: src_offset.z() as NSUInteger,
                },
                metal::MTLSize {
                    width: extent.width() as NSUInteger,
                    height: extent.height() as NSUInteger,
                    depth: extent.depth() as NSUInteger,
                },
                dst.metal(),
                (dst_base_layer + layer) as NSUInteger,
                dst.metal().parent_relative_level(),
                metal::MTLOrigin {
                    x: dst_offset.x() as NSUInteger,
                    y: dst_offset.y() as NSUInteger,
                    z: dst_offset.z() as NSUInteger,
                },
            );
        }
    }

    fn write_buffer(&mut self, buffer: &Buffer, offset: usize, data: &[u8]) {
        let length = buffer.metal().length();
        let mut data_len = 0;
        let fits = match (u64::try_from(offset), u64::try_from(data.len())) {
            (Ok(off), Ok(len)) => {
                data_len = len;
                match off.checked_add(len) {
                    Some(end) => end <= length,
                    None => false,
                }
            }
            _ => false,
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
            .copy_from_buffer(&staged, 0, buffer.metal(), offset as NSUInteger, data_len);
    }
}

pub struct RenderCommandEncoder<'a> {
    encoder: metal::RenderCommandEncoder,
    primitive: metal::MTLPrimitiveType,
    index_buffer: Option<metal::Buffer>,
    index_buffer_offset: NSUInteger,
    _marker: PhantomData<&'a mut CommandBuffer>,
}

impl Drop for RenderCommandEncoder<'_> {
    fn drop(&mut self) {
        self.encoder.end_encoding();
    }
}

// #[hidden_trait::expose]
impl crate::traits::RenderCommandEncoder for RenderCommandEncoder<'_> {
    fn with_pipeline(&mut self, pipeline: &RenderPipeline) {
        self.encoder.set_render_pipeline_state(pipeline.metal());
        self.primitive = pipeline.primitive();
    }

    fn with_viewport(&mut self, offset: Offset3<f32>, extent: Extent3<f32>) {
        let viewport = metal::MTLViewport {
            originX: offset.x().into(),
            originY: offset.y().into(),
            width: extent.width().into(),
            height: extent.height().into(),
            znear: offset.z().into(),
            zfar: (offset.z() + extent.depth()).into(),
        };
        self.encoder.set_viewport(viewport);
    }

    fn with_scissor(&mut self, offset: Offset2<i32>, extent: Extent2<u32>) {
        debug_assert!(offset.x() >= 0);
        debug_assert!(offset.y() >= 0);

        let scissor = metal::MTLScissorRect {
            x: offset.x() as NSUInteger,
            y: offset.y() as NSUInteger,
            width: extent.width() as NSUInteger,
            height: extent.height() as NSUInteger,
        };
        self.encoder.set_scissor_rect(scissor);
    }

    /// Sets arguments group for the current pipeline.
    fn with_arguments(&mut self, group: u32, arguments: &impl Arguments) {
        todo!();
    }

    /// Sets constants for the current pipeline.
    fn with_constants(&mut self, constants: &impl Constants) {
        todo!();
    }

    /// Bind vertex buffer to the current pipeline.
    fn bind_vertex_buffers(&mut self, start: u32, buffers: &[(&crate::backend::Buffer, usize)]) {
        let offsets = buffers
            .iter()
            .map(|(_, o)| *o as NSUInteger)
            .collect::<SmallVec<[_; 8]>>();
        let buffers = buffers
            .iter()
            .map(|(b, _)| Some(&**b.metal()))
            .collect::<SmallVec<[_; 8]>>();

        self.encoder
            .set_vertex_buffers(start as NSUInteger, &buffers, &offsets);
    }

    /// Bind index buffer to the current pipeline.
    fn bind_index_buffer(&mut self, buffer: &crate::backend::Buffer, offset: usize) {
        self.index_buffer = Some(buffer.metal().clone());
        self.index_buffer_offset = offset as NSUInteger;
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

    fn draw_indexed(&mut self, vertex_offset: i32, indices: Range<u32>, instances: Range<u32>) {
        debug_assert!(vertex_offset >= 0);

        if indices.end <= indices.start {
            // Rendering no indices is a no-op
            return;
        }
        if instances.end <= instances.start {
            // Rendering no instances is a no-op
            return;
        }

        if instances.end - 1 == instances.start && vertex_offset == 0 {
            // Rendering single instance
            self.encoder.draw_indexed_primitives(
                self.primitive,
                (indices.end - indices.start).into(),
                metal::MTLIndexType::UInt32,
                self.index_buffer.as_ref().unwrap(),
                (self.index_buffer_offset + (indices.start as NSUInteger * 4)).into(),
            );
        } else if instances.start == 0 && vertex_offset == 0 {
            // Rendering multiple instances
            self.encoder.draw_indexed_primitives_instanced(
                self.primitive,
                (indices.end - indices.start).into(),
                metal::MTLIndexType::UInt32,
                self.index_buffer.as_ref().unwrap(),
                (self.index_buffer_offset + (indices.start as NSUInteger * 4)).into(),
                instances.end.into(),
            );
        } else {
            // Rendering multiple instances with a base instance
            self.encoder
                .draw_indexed_primitives_instanced_base_instance(
                    self.primitive,
                    (indices.end - indices.start).into(),
                    metal::MTLIndexType::UInt32,
                    self.index_buffer.as_ref().unwrap(),
                    (self.index_buffer_offset + (indices.start as NSUInteger * 4)).into(),
                    (instances.end - instances.start).into(),
                    instances.start.into(),
                    vertex_offset as NSUInteger,
                );
        }
    }
}
