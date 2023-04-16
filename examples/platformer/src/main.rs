// use nothing::{
//     gametime::{Clock, FrequencyNumExt, FrequencyTicker},
//     winit, Event, EventLoop, EventLoopBuilder,
// };

use std::{convert::Infallible, future::Ready};

use bob::{
    blink_alloc::BlinkAlloc,
    edict::{EntityId, World},
    game::run_game,
    nix,
    render::{Render, RenderBuilderContext, RenderContext},
};
use tokio::io::AsyncRead;

pub struct MainPass {
    target: EntityId,
}

impl Render for MainPass {
    fn render(
        &mut self,
        mut ctx: RenderContext<'_, '_>,
        _world: &World,
        _blink: &BlinkAlloc,
    ) -> Result<(), bob::render::RenderError> {
        let mut encoder = ctx.new_command_encoder().unwrap();
        let image = ctx.target(self.target);
        let render_encoder = encoder.render(nix::RenderPassDesc {
            name: "test".into(),
            color_attachments: &[nix::AttachmentDesc {
                image,
                load: nix::LoadOp::Clear(nix::ClearColor(1.0, 0.5, 0.3, 1.0)),
                store: nix::StoreOp::Store,
                layer: 0,
                level: 0,
            }],
            depth_stencil_attachment: None,
        });
        drop(render_encoder);
        ctx.commit(encoder.finish()?);
        Ok(())
    }
}

fn main() {
    run_game(|mut game| async move {
        let mut builder = RenderBuilderContext::new("main_pass", &mut game.world);
        let target = builder.create_target("main");
        builder.build(move |mut ctx: RenderContext, _: &World, _: &BlinkAlloc| {
            let mut encoder = ctx.new_command_encoder()?;
            encoder.render(nix::RenderPassDesc {
                color_attachments: &[nix::AttachmentDesc::new(ctx.target(target))
                    .clear(nix::ClearColor(1.0, 0.5, 0.3, 1.0))],
                ..Default::default()
            });
            ctx.commit(encoder.finish()?);
            Ok(())
        });

        game.render_to_window = Some(target);

        Ok::<_, Infallible>(game)
    });
}
