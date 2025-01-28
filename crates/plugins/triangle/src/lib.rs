use std::mem::size_of;

use arcana::{
    code::CodeGraphId,
    edict::{self, query::Cpy, world::World},
    events::{emit_event, Event},
    flow::{sleep, FlowEntity},
    gametime::{ClockStep, TimeSpan},
    hash_id,
    hashbrown::HashMap,
    local_name_hash_id,
    mev::{self, Arguments, DeviceRepr},
    model::{ColorModel, ColorValue, Model, Value},
    name,
    work::{Exec, Image2D, Job, JobDesc, JobIdx, Planner},
    Component, Res, View,
};

arcana::declare_plugin!([dummy ...]);

#[derive(mev::Arguments)]
pub struct DTArguments {
    #[mev(vertex)]
    pub colors: mev::Buffer,
}

#[derive(mev::DeviceRepr)]
pub struct DTConstants {
    pub angle: f32,
    pub width: u32,
    pub height: u32,
}

#[arcana::job]
pub struct DrawTriangle {
    pipeline: Option<mev::RenderPipeline>,
    arguments: Option<DTArguments>,
    constants: HashMap<JobIdx, DTConstants>,
}

impl DrawTriangle {
    pub fn desc() -> JobDesc {
        arcana::job_desc! [
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
            constants: HashMap::new(),
        }
    }
}

impl Job for DrawTriangle {
    fn plan(&mut self, mut planner: Planner<'_>, world: &mut World) {
        let idx = planner.idx();

        let Some(target) = planner.create::<Image2D>().copied() else {
            return;
        };

        let angle = world.view::<Cpy<Angle>>().into_iter().next().unwrap().0;

        let constants = self.constants.entry(idx).or_insert(DTConstants {
            angle: 0.0,
            width: 0,
            height: 0,
        });

        constants.angle = angle;

        while constants.angle > 1.0 {
            constants.angle = constants.angle.fract();
        }

        constants.width = target.extent.width();
        constants.height = target.extent.height();
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
                    arguments: &[DTArguments::LAYOUT],
                    constants: DTConstants::SIZE,
                })
                .unwrap()
        });

        let encoder = runner.new_encoder();

        let c1 = match runner.param("c1") {
            Value::Color(ColorValue::Srgb(rgb)) => [rgb.red, rgb.green, rgb.blue],
            _ => [1.0, 1.0, 0.0],
        };

        let c2 = match runner.param("c2") {
            Value::Color(ColorValue::Srgb(rgb)) => [rgb.red, rgb.green, rgb.blue],
            _ => [0.0, 1.0, 1.0],
        };

        let c3 = match runner.param("c3") {
            Value::Color(ColorValue::Srgb(rgb)) => [rgb.red, rgb.green, rgb.blue],
            _ => [1.0, 0.0, 1.0],
        };

        let arguments = self.arguments.get_or_insert_with(|| {
            let colors = runner
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<[f32; 12]>(),
                    name: "colors",
                    usage: mev::BufferUsage::UNIFORM | mev::BufferUsage::TRANSFER_DST,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
            DTArguments { colors }
        });

        encoder.barrier(
            mev::PipelineStages::FRAGMENT_SHADER,
            mev::PipelineStages::TRANSFER,
        );

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

        encoder.barrier(
            mev::PipelineStages::TRANSFER,
            mev::PipelineStages::FRAGMENT_SHADER,
        );

        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::FRAGMENT_SHADER,
            &target,
        );

        let mut render = encoder.render(mev::RenderPassDesc {
            color_attachments: &[
                mev::AttachmentDesc::new(&target).clear(mev::ClearColor(1.0, 0.5, 0.3, 0.0))
            ],
            ..Default::default()
        });

        let dims = target.extent().expect_2d();

        render.with_pipeline(pipeline);
        render.with_arguments(0, arguments);
        render.with_constants(&self.constants[&runner.idx()]);

        render.with_viewport(
            mev::Offset3::ZERO,
            mev::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
        );
        render.with_scissor(mev::Offset2::ZERO, dims);
        render.draw(0..3, 0..1);
        drop(render);
    }
}

#[derive(mev::Arguments)]
pub struct OpArguments {
    #[mev(storage, shader(compute))]
    pub src: mev::Image,
    #[mev(storage, shader(compute))]
    pub dst: mev::Image,
}

#[derive(mev::DeviceRepr)]
pub struct OpConstants {
    pub op: u32,
}

#[arcana::job(op)]
pub struct OpJob {
    pipeline: Option<mev::ComputePipeline>,
}

impl OpJob {
    pub fn desc() -> JobDesc {
        arcana::job_desc! [
            op: in Model::Enum(vec![
                (name!(add), Some(Model::Unit)),
                (name!(sub), Some(Model::Unit)),
                (name!(mul), Some(Model::Unit)),
                (name!(div), Some(Model::Unit)),
            ]),
            src: Image2D,
            dst: mut Image2D,
        ]
    }

    pub fn new() -> Self {
        OpJob { pipeline: None }
    }
}

impl Job for OpJob {
    fn plan(&mut self, mut planner: Planner<'_>, _world: &mut World) {
        let Some(dst) = planner.update::<Image2D>() else {
            return;
        };
        let src = *dst;
        planner.read::<Image2D>(src);
    }

    fn exec(&mut self, runner: Exec<'_>, _world: &mut World) {
        let Some(dst) = runner.update::<Image2D>() else {
            return;
        };

        let Some(src) = runner.read::<Image2D>() else {
            return;
        };

        let op = match runner.param("op") {
            Value::Enum(op, _) => match op.as_str() {
                "add" => 0,
                "sub" => 1,
                "mul" => 2,
                "div" => 3,
                _ => 0,
            },
            _ => 0,
        };

        let pipeline = self.pipeline.get_or_insert_with(|| {
            let library = runner
                .device()
                .new_shader_library(mev::LibraryDesc {
                    name: "main",
                    input: mev::include_library!("shaders/op.wgsl" as mev::ShaderLanguage::Wgsl),
                })
                .unwrap();

            runner
                .device()
                .new_compute_pipeline(mev::ComputePipelineDesc {
                    name: "main",
                    shader: mev::Shader {
                        library,
                        entry: "main".into(),
                    },
                    work_group_size: [1, 1, 1],
                    arguments: &[OpArguments::LAYOUT],
                    constants: size_of::<OpConstants>(),
                })
                .unwrap()
        });

        let encoder = runner.new_encoder();

        encoder.barrier(mev::PipelineStages::all(), mev::PipelineStages::all());

        let mut compute = encoder.compute();

        let dims = src.extent().expect_2d();

        compute.with_pipeline(pipeline);
        compute.with_arguments(
            0,
            &OpArguments {
                src: src.0.clone(),
                dst: dst.0.clone(),
            },
        );
        compute.with_constants(&OpConstants { op });

        compute.dispatch(dims.to_3d());
        drop(compute);

        encoder.barrier(mev::PipelineStages::all(), mev::PipelineStages::all());
    }
}

fn x2(_: FlowEntity, a: &f32) -> (f32,) {
    (a * 2.0,)
}

fn mul(_: FlowEntity, a: &f32, b: &f32) -> (f32,) {
    (a * b,)
}

fn add(_: FlowEntity, a: &f32, b: &f32) -> (f32,) {
    (a + b,)
}

async fn wait(e: FlowEntity) {
    tracing::info!("Sleeping for a second");
    sleep(TimeSpan::SECOND, e.world()).await;
    tracing::info!("One second passed");
}

#[derive(Clone, Copy, Component)]
struct Speed(f32);

fn set_angle_speed(e: FlowEntity, speed: &f32) {
    tracing::info!("Setting triangle speed to {}", speed);
    let _ = e.set(Speed(*speed));
}

fn get_angle_speed(e: FlowEntity) -> (f32,) {
    (e.get_cloned::<Speed>().unwrap().0,)
}

#[derive(Clone, Copy, Component)]
struct Angle(f32);

fn set_angle(mut e: FlowEntity, angle: &f32) {
    tracing::info!("Setting triangle angle to {}", angle);
    let mut angle = *angle;

    if angle >= 1.0 {
        angle = angle.fract();
    }

    let _ = e.set(Angle(angle));
}

fn get_angle(e: FlowEntity) -> (f32,) {
    (e.get_cloned::<Angle>().unwrap().0,)
}

#[arcana::system]
fn rotate_system(view: View<(&mut Angle, &Speed)>, clock: Res<ClockStep>) {
    for (angle, speed) in view {
        angle.0 += speed.0 * clock.step.as_secs_f32();

        if angle.0 >= 1.0 {
            angle.0 = angle.0.fract();
        }
    }
}

#[arcana::init]
fn init(world: &mut World) {
    let e = world
        .spawn((
            Speed(std::f32::consts::FRAC_1_PI * 0.5),
            Angle(0.0),
            hash_id!("speedup" => CodeGraphId),
        ))
        .id();

    // spawn_block!(in world -> {
    //     for _ in 0..1 {
    //         emit_event(world, Event::new(local_name_hash_id!(Start), e));
    //         sleep(TimeSpan::SECOND, world).await;
    //     }
    // });
}

// arcana::export_arcana_plugin! {
//     TrianglePlugin {
//         // List dependencies
//         dependencies: [dummy ...],

//         // List systems
//         systems: [rotate_system],

//         // List jobs
//         jobs: [DrawTriangle, op: OpJob::desc() => OpJob::new()],

//         events: [Start],

//         pure_codes: [x2, mul, add, get_angle_speed, get_angle],
//         flow_codes: [wait, set_angle_speed, set_angle],

//         // Init block
//         in world => {
//             let e = world.spawn((
//                 Speed(std::f32::consts::FRAC_1_PI * 0.5),
//                 Angle(0.0),
//                 Code {
//                     code_id: hash_id!("speedup"),
//                 }
//             )).id();

//             spawn_block!(in world -> {
//                 for _ in 0..1 {
//                     emit_event(world, Event::new(local_name_hash_id!(Start), e));
//                     sleep(TimeSpan::SECOND, world).await;
//                 }
//             });
//         }
//     }
// }
