use std::mem::size_of;

use arcana::{
    blink_alloc::BlinkAlloc,
    edict::{self, Component, EntityId, Scheduler, World},
    egui::{EguiRender, EguiResource},
    mev::{self, Arguments, DeviceRepr},
    plugin::ArcanaPlugin,
    render::{Render, RenderBuilderContext, RenderContext, RenderError, RenderGraph, TargetId},
    winit::window::Window,
};

arcana::feature_ed! {
    use arcana::project::Dependency;
}

use camera::Camera2;
use scene::Global2;

arcana::export_arcana_plugin!(SdfPlugin);

pub struct SdfPlugin;

impl ArcanaPlugin for SdfPlugin {
    fn name(&self) -> &'static str {
        "sdf"
    }

    arcana::feature_ed! {
        fn dependencies(&self) -> Vec<(&'static dyn ArcanaPlugin, Dependency)> {
            vec![scene::path_dependency(), camera::path_dependency()]
        }
    }

    fn init(&self, _world: &mut World, _scheduler: &mut Scheduler) {
        macro_rules! field_offset {
            ($s:ident . $f:ident) => {
                (std::ptr::addr_of!($s.$f) as usize - std::ptr::addr_of!($s) as usize)
            };
        }
    }
}

#[derive(Component)]
pub struct Shape {
    pub color: [f32; 4],
    pub transform: na::Affine2<f32>,
    pub kind: ShapeKind,
}

impl Shape {
    pub fn new_rect(width: f32, height: f32) -> Self {
        Self {
            color: [0.8, 0.2, 1.0, 1.0],
            transform: na::Affine2::identity(),
            kind: ShapeKind::Rect { width, height },
        }
    }
}

pub enum ShapeKind {
    Circle { radius: f32 },
    Rect { width: f32, height: f32 },
}

#[derive(DeviceRepr)]
struct ShapeDevice {
    inv_tr: mev::mat3,
    color: mev::vec4,
    kind: u32,
    payload: u32,
    layer: u32,
}

#[derive(DeviceRepr)]
struct CirleDevice {
    radius: f32,
}

#[derive(DeviceRepr)]
struct RectDevice {
    half: mev::vec2,
}

#[derive(mev::Arguments)]
pub struct MainArguments {
    #[mev(storage, fragment)]
    pub shapes: mev::Buffer,
    #[mev(storage, fragment)]
    pub circles: mev::Buffer,
    #[mev(storage, fragment)]
    pub rects: mev::Buffer,
}

#[derive(mev::DeviceRepr)]
pub struct MainConstants {
    pub background: mev::vec4,
    pub shape_count: u32,
    pub camera: mev::mat3,
}

pub struct SdfRender {
    camera: EntityId,
    target: TargetId<mev::Image>,
    pipeline: Option<mev::RenderPipeline>,
    arguments: Option<MainArguments>,
    constants: MainConstants,

    shapes_device: Vec<<ShapeDevice as DeviceRepr>::Repr>,
    circles_device: Vec<<CirleDevice as DeviceRepr>::Repr>,
    rects_device: Vec<<RectDevice as DeviceRepr>::Repr>,
}

impl SdfRender {
    pub fn build(camera: EntityId, graph: &mut RenderGraph) -> TargetId<mev::Image> {
        // Start building render.
        let mut builder = RenderBuilderContext::new("main_pass", graph);

        // This render defines a single render target.
        let target = builder.create_target("main", mev::PipelineStages::COLOR_OUTPUT);

        // Build the render with SdfRender as `Render` impl.
        // `SdfRender::render` will be called every frame to encode commands for this render.
        builder.build(SdfRender {
            camera,
            target,
            pipeline: None,
            arguments: None,
            constants: MainConstants {
                background: mev::vec4(0.5, 0.2, 0.1, 1.0),
                shape_count: 0,
                camera: mev::mat3::from([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
            },
            shapes_device: Vec::new(),
            circles_device: Vec::new(),
            rects_device: Vec::new(),
        });
        target
    }
}

impl Render for SdfRender {
    fn render(
        &mut self,
        mut ctx: RenderContext<'_, '_>,
        world: &World,
        _blink: &BlinkAlloc,
    ) -> Result<(), RenderError> {
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

        let dims = target.dimensions().to_2d();

        let camera = world
            .try_view_one::<(&Global2, &Camera2)>(self.camera)
            .expect("Camera is missing");

        let camera = {
            let (g, c) = camera.get().unwrap();

            let viewport = c
                .viewport
                .transform(0.5, dims.width() as f32 / dims.height() as f32);

            <[[f32; 3]; 3]>::from((g.iso * viewport).to_homogeneous())
        };

        let shapes = world.view::<(&Global2, &Shape)>();
        let shapes_count = shapes.iter().count();

        let arguments = self.arguments.get_or_insert_with(|| {
            let shapes = ctx
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<<ShapeDevice as DeviceRepr>::Repr>()
                        * shapes_count.next_power_of_two(),
                    name: "shapes",
                    usage: mev::BufferUsage::STORAGE | mev::BufferUsage::TRANSFER_DST,
                    memory: mev::Memory::Shared,
                })
                .unwrap();

            let circles = ctx
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<<CirleDevice as DeviceRepr>::Repr>()
                        * shapes_count.next_power_of_two(),
                    name: "circles",
                    usage: mev::BufferUsage::STORAGE | mev::BufferUsage::TRANSFER_DST,
                    memory: mev::Memory::Shared,
                })
                .unwrap();

            let rects = ctx
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<<RectDevice as DeviceRepr>::Repr>()
                        * shapes_count.next_power_of_two(),
                    name: "rects",
                    usage: mev::BufferUsage::STORAGE | mev::BufferUsage::TRANSFER_DST,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
            MainArguments {
                shapes,
                circles,
                rects,
            }
        });

        if arguments.shapes.size() < size_of::<ShapeDevice>() * shapes_count {
            arguments.shapes = ctx
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<<ShapeDevice as DeviceRepr>::Repr>()
                        * shapes_count.next_power_of_two(),
                    name: "shapes",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
        }

        if arguments.circles.size() < size_of::<<CirleDevice as DeviceRepr>::Repr>() * shapes_count
        {
            arguments.circles = ctx
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<CirleDevice>() * shapes_count.next_power_of_two(),
                    name: "circles",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
        }

        if arguments.rects.size() < size_of::<<RectDevice as DeviceRepr>::Repr>() * shapes_count {
            arguments.rects = ctx
                .device()
                .new_buffer(mev::BufferDesc {
                    size: size_of::<CirleDevice>() * shapes_count.next_power_of_two(),
                    name: "rects",
                    usage: mev::BufferUsage::UNIFORM,
                    memory: mev::Memory::Shared,
                })
                .unwrap();
        }

        self.constants = MainConstants {
            background: mev::vec4(0.5, 0.2, 0.1, 1.0),
            shape_count: shapes_count as u32,
            camera: mev::mat3::from(camera),
        };

        self.shapes_device.clear();
        self.circles_device.clear();
        self.rects_device.clear();
        for (global, shape) in shapes.iter() {
            let inv_tr = global.iso.inverse().to_homogeneous() * shape.transform.inverse().matrix();

            self.shapes_device.push(
                ShapeDevice {
                    kind: match shape.kind {
                        ShapeKind::Circle { .. } => 0,
                        ShapeKind::Rect { .. } => 1,
                    },
                    payload: match shape.kind {
                        ShapeKind::Circle { .. } => self.circles_device.len() as u32,
                        ShapeKind::Rect { .. } => self.rects_device.len() as u32,
                    },
                    color: mev::vec(shape.color),
                    inv_tr: inv_tr.as_ref().into(),
                    layer: 0,
                }
                .as_repr(),
            );

            match shape.kind {
                ShapeKind::Circle { radius } => {
                    self.circles_device.push(CirleDevice { radius }.as_repr());
                }
                ShapeKind::Rect { width, height } => {
                    self.rects_device.push(
                        RectDevice {
                            half: mev::vec2(width / 2.0, height / 2.0),
                        }
                        .as_repr(),
                    );
                }
            }
        }

        {
            let mut copy = encoder.copy();
            copy.write_buffer_slice(&arguments.shapes, 0, &self.shapes_device);
            copy.write_buffer_slice(&arguments.circles, 0, &self.circles_device);
            copy.write_buffer_slice(&arguments.rects, 0, &self.rects_device);
        }

        let mut render = encoder.render(mev::RenderPassDesc {
            color_attachments: &[
                mev::AttachmentDesc::new(&target).clear(mev::ClearColor(0.0, 0.0, 0.0, 1.0))
            ],
            ..Default::default()
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
