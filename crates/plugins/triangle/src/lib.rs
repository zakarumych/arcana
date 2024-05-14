use std::mem::size_of;

use arcana::{
    edict::World,
    gametime::ClockStep,
    hashbrown::HashMap,
    mev::{self, Arguments, DeviceRepr},
    model::{ColorModel, ColorValue, Model, Value},
    work::{Exec, Image2D, Job, JobDesc, Planner},
    Name,
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

pub struct DrawTriangle {
    pipeline: Option<mev::RenderPipeline>,
    arguments: Option<MainArguments>,
    constants: MainConstants,
}

impl DrawTriangle {
    pub fn desc() -> JobDesc {
        arcana::job_desc! [
            speed: in Model::Float,
            c1: in Model::Color(ColorModel::Srgb),
            c2: in Model::Color(ColorModel::Srgb),
            c3: in Model::Color(ColorModel::Srgb),
            main: +Image2D,
        ]
    }

    pub fn new() -> Self {
        DrawTriangle {
            pipeline: None,
            arguments: None,
            constants: MainConstants {
                angle: 0.0,
                width: 0,
                height: 0,
            },
        }
    }
}

impl Job for DrawTriangle {
    fn plan(&mut self, mut planner: Planner<'_>, world: &mut World, params: &HashMap<Name, Value>) {
        let Some(target) = planner.create::<Image2D>() else {
            return;
        };

        let speed = match params.get("speed") {
            Some(Value::Int(speed)) => (*speed) as f32,
            Some(Value::Float(speed)) => (*speed) as f32,
            _ => 1.0,
        };

        self.constants.angle += world.expect_resource::<ClockStep>().step.as_secs_f32() * speed;

        while self.constants.angle > 1.0 {
            self.constants.angle = self.constants.angle.fract();
        }

        self.constants.width = target.extent.width();
        self.constants.height = target.extent.height();
    }

    fn exec(&mut self, runner: Exec<'_>, _world: &mut World, params: &HashMap<Name, Value>) {
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

        let c1 = match params.get("c1") {
            Some(Value::Color(ColorValue::Srgb(rgb))) => [rgb.red, rgb.green, rgb.blue],
            _ => [1.0, 1.0, 0.0],
        };

        let c2 = match params.get("c2") {
            Some(Value::Color(ColorValue::Srgb(rgb))) => [rgb.red, rgb.green, rgb.blue],
            _ => [0.0, 1.0, 1.0],
        };

        let c3 = match params.get("c3") {
            Some(Value::Color(ColorValue::Srgb(rgb))) => [rgb.red, rgb.green, rgb.blue],
            _ => [1.0, 0.0, 1.0],
        };

        let arguments = self.arguments.get_or_insert_with(|| {
            let colors = runner
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<[f32; 12]>(),
                    name: "colors",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
            MainArguments { colors }
        });

        encoder.copy().write_buffer_slice(
            arguments.colors.slice(..),
            &[
                c1[0],
                c1[1],
                c1[2],
                f32::NAN,
                c2[0],
                c2[1],
                c2[2],
                f32::NAN,
                c3[0],
                c3[1],
                c3[2],
                f32::NAN,
            ],
        );

        let mut render = encoder.render(mev::RenderPassDesc {
            color_attachments: &[
                mev::AttachmentDesc::new(&target).clear(mev::ClearColor(1.0, 0.5, 0.3, 0.0))
            ],
            ..Default::default()
        });

        let dims = target.dimensions().expect_2d();

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
    }
}

arcana::export_arcana_plugin! {
    TrianglePlugin {
        // List dependencies
        dependencies: [dummy ...],

        // List jobs
        jobs: [DrawTriangle],

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
