use arcana::{
    edict::World,
    gametime::ClockStep,
    mev::{self, Arguments, DeviceRepr},
    work::{Exec, Image2D, Job, JobDesc, Planner},
};

#[derive(mev::Arguments)]
pub struct DSArguments {
    #[mev(vertex)]
    pub colors: mev::Buffer,
}

#[derive(mev::DeviceRepr)]
pub struct DSConstants {
    pub angle: f32,
    pub width: u32,
    pub height: u32,
}

pub struct DrawSquare {
    pipeline: Option<mev::RenderPipeline>,
    arguments: Option<DSArguments>,
    constants: DSConstants,
}

impl DrawSquare {
    pub fn desc() -> JobDesc {
        arcana::job_desc! [
            main: +Image2D,
        ]
    }

    pub fn new() -> Self {
        DrawSquare {
            pipeline: None,
            arguments: None,
            constants: DSConstants {
                angle: 0.0,
                width: 0,
                height: 0,
            },
        }
    }
}

impl Job for DrawSquare {
    fn plan(&mut self, mut planner: Planner<'_>, world: &mut World) {
        let Some(target) = planner.create::<Image2D>() else {
            return;
        };

        self.constants = DSConstants {
            angle: world
                .expect_resource::<ClockStep>()
                .now
                .elapsed_since_start()
                .as_secs_f32(),
            width: target.extent.width(),
            height: target.extent.height(),
        };
    }

    fn exec(&mut self, runner: Exec<'_>, _world: &mut World) {
        let Some(target) = runner.create::<Image2D>() else {
            return;
        };

        let pipeline = self.pipeline.get_or_insert_with(|| {
            let main_library = runner
                .device()
                .new_shader_library(mev::LibraryDesc {
                    name: "main",
                    input: mev::include_library!("shaders/main.wgsl" as mev::ShaderLanguage::Wgsl),
                })
                .unwrap();

            runner
                .device()
                .new_render_pipeline(mev::RenderPipelineDesc {
                    name: "main",
                    vertex_shader: main_library.entry("vs_main"),
                    vertex_attributes: vec![],
                    vertex_layouts: vec![],
                    primitive_topology: mev::PrimitiveTopology::Triangle,
                    raster: Some(mev::RasterDesc {
                        fragment_shader: Some(main_library.entry("fs_main")),
                        color_targets: vec![mev::ColorTargetDesc {
                            format: target.format(),
                            blend: Some(mev::BlendDesc::default()),
                        }],
                        depth_stencil: None,
                        front_face: mev::FrontFace::default(),
                        culling: mev::Culling::None,
                    }),
                    arguments: &[DSArguments::LAYOUT],
                    constants: DSConstants::SIZE,
                })
                .unwrap()
        });

        let encoder = runner.new_encoder();

        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::FRAGMENT_SHADER,
            &target,
        );

        let mut render = encoder.render(mev::RenderPassDesc::new().color_attachments(&[
            mev::AttachmentDesc::new(&target).clear(mev::ClearColor(1.0, 0.5, 0.3, 0.0)),
        ]));

        let dims = target.dimensions().expect_2d();

        let arguments = self.arguments.get_or_insert_with(|| {
            let colors = runner
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
                        1.0,
                        0.0,
                        1.0,
                        f32::NAN,
                        1.0,
                        1.0,
                        0.0,
                        f32::NAN,
                        1.0,
                        1.0,
                        0.0,
                        f32::NAN,
                    ]),
                    name: "colors",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
            DSArguments { colors }
        });

        render.with_pipeline(pipeline);
        render.with_arguments(0, arguments);
        render.with_constants(&self.constants);

        render.with_viewport(
            mev::Offset3::ZERO,
            mev::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
        );
        render.with_scissor(mev::Offset2::ZERO, dims);
        render.draw(0..6, 0..1);
        drop(render);
    }
}

arcana::export_arcana_plugin! {
    SquarePlugin {
        // List dependencies
        dependencies: [dummy ...],

        // List jobs
        jobs: [DrawSquare],

        // // Init block
        // in world => {
        //     let world = world.local();

        //     // let window = world.expect_resource::<Window>().id();

        //     let mut graph = world.expect_resource_mut::<RenderGraph>();
        //     // Create main pass.
        //     // It returns target id that it renders to.
        //     let target = MainPass::build(&mut graph);

        //     // let id = world.spawn_one(Egui::new()).id();

        //     // if world.get_resource::<EguiResource>().is_some() {
        //     //     target = EguiRender::build_overlay(id, target, &mut graph);
        //     // }

        //     // Use window's surface for the render target.
        //     graph.present(target);
        //     drop(graph);
        // }
    }
}
