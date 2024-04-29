use arcana::{
    edict::World,
    gametime::ClockStep,
    mev::{self, Arguments, DeviceRepr, Image},
    render::{Render, RenderBuilderContext, RenderContext, RenderError, RenderGraph, TargetId},
    stid::WithStid,
    work::{Exec, Image2D, Job, JobCreateDesc, JobDesc, Planner},
};

#[derive(mev::Arguments)]
pub struct MainArguments {
    #[mev(vertex)]
    pub colors: mev::Buffer,
}

#[derive(mev::DeviceRepr)]
pub struct MainConstants {
    pub angle: f32,
    pub width: u32,
    pub height: u32,
}

pub struct MainPass {
    target: TargetId<mev::Image>,
    pipeline: Option<mev::RenderPipeline>,
    arguments: Option<MainArguments>,
    constants: MainConstants,
}

impl MainPass {
    fn build(graph: &mut RenderGraph) -> TargetId<mev::Image> {
        // Start building render.
        let mut builder = RenderBuilderContext::new("main_pass", graph);

        // This render defines a single render target.
        let target = builder.create_target("main", mev::PipelineStages::COLOR_OUTPUT);

        // Build the render with MainPass as `Render` impl.
        // `MainPass::render` will be called every frame to encode commands for this render.
        builder.build(MainPass {
            target,
            pipeline: None,
            arguments: None,
            constants: MainConstants {
                angle: 0.0,
                width: 0,
                height: 0,
            },
        });
        target
    }
}

impl Render for MainPass {
    fn render(&mut self, world: &World, mut ctx: RenderContext<'_, '_>) -> Result<(), RenderError> {
        let mut encoder = ctx.new_command_encoder()?;
        let target = ctx.write_target(self.target, &mut encoder).clone();
        let pipeline = self.pipeline.get_or_insert_with(|| {
            let main_library = ctx
                .device()
                .new_shader_library(mev::LibraryDesc {
                    name: "main",
                    input: mev::include_library!("shaders/main.wgsl" as mev::ShaderLanguage::Wgsl),
                })
                .unwrap();

            ctx.device()
                .new_render_pipeline(mev::RenderPipelineDesc {
                    name: "main",
                    vertex_shader: mev::Shader {
                        library: main_library.clone(),
                        entry: "vs_main".into(),
                    },
                    vertex_attributes: vec![],
                    vertex_layouts: vec![],
                    primitive_topology: mev::PrimitiveTopology::Triangle,
                    raster: Some(mev::RasterDesc {
                        fragment_shader: Some(mev::Shader {
                            library: main_library,
                            entry: "fs_main".into(),
                        }),
                        color_targets: vec![mev::ColorTargetDesc {
                            format: target.format(),
                            blend: Some(mev::BlendDesc::default()),
                        }],
                        depth_stencil: None,
                        front_face: mev::FrontFace::default(),
                        culling: mev::Culling::Back,
                    }),
                    arguments: &[MainArguments::LAYOUT],
                    constants: MainConstants::SIZE,
                })
                .unwrap()
        });

        let mut render = encoder.render(mev::RenderPassDesc {
            color_attachments: &[
                mev::AttachmentDesc::new(&target).clear(mev::ClearColor(1.0, 0.5, 0.3, 0.0))
            ],
            ..Default::default()
        });

        let dims = target.dimensions().to_2d();

        let arguments = self.arguments.get_or_insert_with(|| {
            let colors = ctx
                .device()
                .new_buffer_init(mev::BufferInitDesc {
                    data: arcana::bytemuck::cast_slice(&[
                        1.0,
                        1.0,
                        0.0,
                        f32::NAN,
                        0.0,
                        1.0,
                        1.0,
                        f32::NAN,
                        1.0,
                        0.0,
                        1.0,
                        f32::NAN,
                    ]),
                    name: "colors",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
            MainArguments { colors }
        });

        self.constants = MainConstants {
            angle: world
                .expect_resource::<ClockStep>()
                .now
                .elapsed_since_start()
                .as_secs_f32(),
            width: dims.width(),
            height: dims.height(),
        };

        render.with_pipeline(pipeline);
        render.with_arguments(0, arguments);
        render.with_constants(&self.constants);

        render.with_viewport(
            mev::Offset3::ZERO,
            mev::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
        );
        render.with_scissor(mev::Offset2::ZERO, dims);
        render.draw(0..3, 0..1);
        drop(render);
        ctx.commit(encoder.finish()?);
        Ok(())
    }
}

// const fn bezier<const N: usize>(t: f64) -> [f64; N + 1] {
//     let mut weights = [0.0; N + 1];
//     weights[0] = 1.0;
//     for i in 1..=N {
//         let ti = t.powi(i as i32);
//         let tni = (1.0 - t).powi(i as i32);
//         let mut ci = [0.0; N + 1];
//         for j in 0..i {
//             let mij = binomial_coefficient(i - 1, j);
//             ci[j] = mij as f64 * tni * t.powi((i - j - 1) as i32);
//         }
//         weights[i] = ci
//             .iter()
//             .zip(weights.iter())
//             .map(|(&c, &w)| c * w)
//             .sum::<f64>()
//             / ti;
//     }
//     weights
// }

// const fn binomial_coefficient(n: usize, k: usize) -> usize {
//     let mut res = 1;
//     for i in 0..k {
//         res *= n - i;
//         res /= i + 1;
//     }
//     res
// }

pub struct MainJob {
    pipeline: Option<mev::RenderPipeline>,
    arguments: Option<MainArguments>,
    constants: MainConstants,
}

impl MainJob {
    pub fn desc() -> JobDesc {
        job_desc!(+Image2D => "main")
    }
}

impl Job for MainJob {
    fn plan(&mut self, mut planner: Planner<'_>) {
        let Some(target) = planner.create::<Image2D>() else {
            return;
        };

        self.constants = MainConstants {
            angle: world
                .expect_resource::<ClockStep>()
                .now
                .elapsed_since_start()
                .as_secs_f32(),
            width: target.extent.width(),
            height: target.extent.height(),
        };
    }

    fn exec(&mut self, mut runner: Exec<'_>) {
        let Some(target) = runner.create::<Image2D>() else {
            return;
        };

        let pipeline = self.pipeline.get_or_insert_with(|| {
            let main_library = ctx
                .device()
                .new_shader_library(mev::LibraryDesc {
                    name: "main",
                    input: mev::include_library!("shaders/main.wgsl" as mev::ShaderLanguage::Wgsl),
                })
                .unwrap();

            ctx.device()
                .new_render_pipeline(mev::RenderPipelineDesc {
                    name: "main",
                    vertex_shader: mev::Shader {
                        library: main_library.clone(),
                        entry: "vs_main".into(),
                    },
                    vertex_attributes: vec![],
                    vertex_layouts: vec![],
                    primitive_topology: mev::PrimitiveTopology::Triangle,
                    raster: Some(mev::RasterDesc {
                        fragment_shader: Some(mev::Shader {
                            library: main_library,
                            entry: "fs_main".into(),
                        }),
                        color_targets: vec![mev::ColorTargetDesc {
                            format: target.format(),
                            blend: Some(mev::BlendDesc::default()),
                        }],
                        depth_stencil: None,
                        front_face: mev::FrontFace::default(),
                        culling: mev::Culling::Back,
                    }),
                    arguments: &[MainArguments::LAYOUT],
                    constants: MainConstants::SIZE,
                })
                .unwrap()
        });

        let encoder = runner.new_encoder();
        let mut render = encoder.render(mev::RenderPassDesc {
            color_attachments: &[
                mev::AttachmentDesc::new(&target).clear(mev::ClearColor(1.0, 0.5, 0.3, 0.0))
            ],
            ..Default::default()
        });

        let dims = target.dimensions().to_2d();

        let arguments = self.arguments.get_or_insert_with(|| {
            let colors = ctx
                .device()
                .new_buffer_init(mev::BufferInitDesc {
                    data: arcana::bytemuck::cast_slice(&[
                        1.0,
                        1.0,
                        0.0,
                        f32::NAN,
                        0.0,
                        1.0,
                        1.0,
                        f32::NAN,
                        1.0,
                        0.0,
                        1.0,
                        f32::NAN,
                    ]),
                    name: "colors",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
            MainArguments { colors }
        });

        render.with_pipeline(pipeline);
        render.with_arguments(0, arguments);
        render.with_constants(&self.constants);

        render.with_viewport(
            mev::Offset3::ZERO,
            mev::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
        );
        render.with_scissor(mev::Offset2::ZERO, dims);
        render.draw(0..3, 0..1);
        drop(render);
        ctx.commit(encoder.finish()?);
        Ok(())
    }
}

arcana::export_arcana_plugin! {
    TrianglePlugin {
        dependencies: [dummy ...],
        in world => {
            let world = world.local();

            // let window = world.expect_resource::<Window>().id();

            let mut graph = world.expect_resource_mut::<RenderGraph>();
            // Create main pass.
            // It returns target id that it renders to.
            let target = MainPass::build(&mut graph);

            // let id = world.spawn_one(Egui::new()).id();

            // if world.get_resource::<EguiResource>().is_some() {
            //     target = EguiRender::build_overlay(id, target, &mut graph);
            // }

            // Use window's surface for the render target.
            graph.present(target);
            drop(graph);
        }
    }
}
