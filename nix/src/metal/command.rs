use std::marker::PhantomData;

use crate::generic::{ClearColor, ClearDepthStencil, LoadOp, RenderPassDesc, StoreOp};

pub struct CommandBuffer {
    buffer: metal::CommandBuffer,
}

impl CommandBuffer {
    pub(super) fn commit(self) {
        self.buffer.commit();
    }
}

pub struct CommandEncoder {
    buffer: metal::CommandBuffer,
}

impl CommandEncoder {
    pub(super) fn new(buffer: metal::CommandBuffer) -> Self {
        CommandEncoder { buffer }
    }
}

#[hidden_trait::expose]
impl crate::traits::CommandEncoder for CommandEncoder {
    fn render(&mut self, desc: RenderPassDesc) -> RenderCommandEncoder<'_> {
        let mdesc = metal::RenderPassDescriptor::new();
        let color_attachments = mdesc.color_attachments();
        for (idx, color) in desc.color_attachments.iter().enumerate() {
            let format = color.image.format();
            debug_assert!(!format.is_color());

            let attachment = metal::RenderPassColorAttachmentDescriptor::new();
            attachment.set_texture(Some(color.image.texture()));
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
            debug_assert!(!format.is_color());
            debug_assert!(format.is_depth() || format.is_stencil());

            if format.is_depth() {
                let attachment = mdesc.depth_attachment().unwrap();
                attachment.set_texture(Some(depth.image.texture()));
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
                attachment.set_texture(Some(depth.image.texture()));
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
            _marker: PhantomData,
        }
    }

    fn finish(self) -> CommandBuffer {
        CommandBuffer {
            buffer: self.buffer,
        }
    }
}

pub struct RenderCommandEncoder<'a> {
    encoder: metal::RenderCommandEncoder,
    _marker: PhantomData<&'a mut CommandBuffer>,
}

impl Drop for RenderCommandEncoder<'_> {
    fn drop(&mut self) {
        self.encoder.end_encoding();
    }
}

impl crate::traits::RenderCommandEncoder for RenderCommandEncoder<'_> {}
