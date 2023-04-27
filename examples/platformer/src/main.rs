// use nothing::{
//     gametime::{Clock, FrequencyNumExt, FrequencyTicker},
//     winit, Event, EventLoop, EventLoopBuilder,
// };

use std::convert::Infallible;

use bob::{
    blink_alloc::BlinkAlloc,
    edict::{EntityId, World},
    game::run_game,
    nix,
    render::{Render, RenderBuilderContext, RenderContext, RenderError},
};

pub struct MainPass {
    target: EntityId,
    pipeline: Option<nix::RenderPipeline>,
}

impl Render for MainPass {
    fn render(
        &mut self,
        mut ctx: RenderContext<'_, '_>,
        _world: &World,
        _blink: &BlinkAlloc,
    ) -> Result<(), RenderError> {
        let mut encoder = ctx.new_command_encoder()?;
        let target = ctx.target(self.target).clone();
        let pipeline = self.get_pipeline(ctx.device(), target.format());
        let mut render = encoder.render(nix::RenderPassDesc {
            color_attachments: &[
                nix::AttachmentDesc::new(&target).clear(nix::ClearColor(1.0, 0.5, 0.3, 1.0))
            ],
            ..Default::default()
        });
        render.with_pipeline(pipeline);
        render.draw(0..3, 0..1);
        drop(render);
        ctx.commit(encoder.finish()?);
        Ok(())
    }
}

impl MainPass {
    fn build(world: &mut World) -> EntityId {
        let mut builder = RenderBuilderContext::new("main_pass", world);
        let target = builder.create_target("main");
        builder.build(MainPass {
            target,
            pipeline: None,
        });
        target
    }

    fn get_pipeline(
        &mut self,
        device: &nix::Device,
        target_format: nix::PixelFormat,
    ) -> &nix::RenderPipeline {
        self.pipeline.get_or_insert_with(|| {
            let main_library = device
                .new_shader_library(nix::LibraryDesc {
                    name: "main",
                    input: nix::include_library!("shaders/main.wgsl" as nix::ShaderLanguage::Wgsl),
                })
                .unwrap();

            device
                .new_render_pipeline(nix::RenderPipelineDesc {
                    name: "main",
                    vertex_shader: nix::Shader {
                        library: main_library.clone(),
                        entry: "vs_main".into(),
                    },
                    vertex_attributes: vec![],
                    vertex_layouts: vec![],
                    primitive_topology: nix::PrimitiveTopology::Triangle,
                    raster: Some(nix::RasterDesc {
                        fragment_shader: Some(nix::Shader {
                            library: main_library,
                            entry: "fs_main".into(),
                        }),
                        color_targets: vec![nix::ColorTargetDesc {
                            format: target_format,
                            blend: None,
                        }],
                        depth_stencil: None,
                    }),
                })
                .unwrap()
        })
    }
}

fn main() {
    run_game(|mut game| async move {
        game.render_window = Some(MainPass::build(&mut game.world));
        Ok::<_, Infallible>(game)
    });
}
