use crate::{
    game::*,
    scene::{Block, Paddle},
};
use arcana::ecs::query::Entities;
use arcana::{
    athena::{Vector2, Vector4},
    render::CurrentRenderer,
    scene::dim2::{self, Rect, Transform},
};
use arcana::{
    ecs::world::World,
    mev,
    work_graph::{Exec, Image2D, Image2DReq, Job, JobDesc, Planner, Req, TargetCreateDesc},
};

pub struct BlockbreakerFrame {
    pipeline: Option<(mev::PixelFormat, mev::RenderPipeline)>,
    vertices: Vec<[f32; 6]>,
}

#[arcana::job]
impl Job for BlockbreakerFrame {
    fn name() -> arcana::Name {
        arcana::Name::from_name_static("Blockbreaker frame")
    }

    fn desc() -> JobDesc {
        JobDesc {
            creates: vec![TargetCreateDesc::new::<Image2D>(arcana::name!(Frame))],
            ..JobDesc::default()
        }
    }

    fn new() -> Self {
        Self {
            pipeline: None,
            vertices: Vec::with_capacity(30000),
        }
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        planner.create_with_req::<Image2D>(Image2DReq {
            extent: Req::Suggest(mev::Extent2::new(1, 1)),
            format: Req::Suggest(mev::PixelFormat::Rgba8Unorm),
            usage: mev::ImageUsage::TARGET,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, world: &mut World) {
        let Some(target) = exec.create::<Image2D>() else {
            return;
        };

        if arcana::scene::update_transforms(world).is_err() {
            return;
        }
        crate::ui::prepare_render(world);
        let Some(game) = world.get_resource::<Game>() else {
            return;
        };

        let format = target.format();

        if self.pipeline.as_ref().is_none_or(|(f, _)| *f != format) {
            let library = exec
                .device()
                .new_shader_library(mev::LibraryDesc {
                    name: "Blockbreaker",
                    input: mev::LibraryInput::wgsl(include_str!("frame.wgsl")),
                })
                .expect("Blockbreaker shader must compile");

            let pipeline = exec
                .device()
                .new_render_pipeline(mev::RenderPipelineDesc {
                    name: "Blockbreaker",
                    vertex_shader: library.entry("vertex_main"),
                    vertex_layouts: vec![mev::VertexLayoutDesc {
                        stride: 24,
                        step_mode: mev::VertexStepMode::Vertex,
                    }],
                    vertex_attributes: vec![
                        mev::VertexAttributeDesc {
                            buffer_index: 0,
                            format: mev::VertexFormat::Float32x2,
                            offset: 0,
                        },
                        mev::VertexAttributeDesc {
                            buffer_index: 0,
                            format: mev::VertexFormat::Float32x4,
                            offset: 8,
                        },
                    ],
                    primitive_topology: mev::PrimitiveTopology::Triangle,
                    raster: Some(mev::RasterDesc {
                        fragment_shader: Some(library.entry("fragment_main")),
                        color_targets: vec![mev::ColorTargetDesc::new(format)],
                        depth_stencil: None,
                        front_face: mev::FrontFace::default(),
                        culling: mev::Culling::None,
                    }),
                    constants: 0,
                    arguments: &[],
                })
                .expect("Blockbreaker pipeline must compile");

            self.pipeline = Some((format, pipeline));
        }

        let extent = target.extent();

        self.vertices.clear();

        let mut canvas = Canvas {
            vertices: &mut self.vertices,
            transform: Transform::default(),
        };

        canvas.scene(&game, world);

        let w = extent.width() as f32;
        let h = extent.height() as f32;
        let Some(owner) = world.get_resource::<CurrentRenderer>() else {
            return;
        };
        let Some(camera) = dim2::camera_matrices(world, owner.entity, (w / h) as f64) else {
            return;
        };
        let matrix = camera.projection * camera.view;
        for v in &mut self.vertices {
            let p = matrix * Vector4::new(v[0] as f64, v[1] as f64, 0.0, 1.0);
            v[0] = (p.x / p.w) as f32;
            v[1] = (p.y / p.w) as f32;
        }

        let buffer = exec.device().new_buffer_init(mev::BufferInitDesc {
            name: "Blockbreaker frame geometry",
            data: arcana::bytemuck::cast_slice::<_, u8>(&self.vertices),
            usage: mev::BufferUsage::VERTEX,
        });

        let encoder = exec.new_encoder();

        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::COLOR_OUTPUT,
            &target.0,
        );

        let mut pass = encoder.render(mev::RenderPassDesc {
            name: "Blockbreaker whole frame",
            color_attachments: &[mev::AttachmentDesc::new(&target.0)
                .clear(mev::ClearColor(0.008, 0.012, 0.026, 1.0))],
            depth_stencil_attachment: None,
        });

        if let Some((_, pipeline)) = &self.pipeline {
            pass.with_pipeline(pipeline);
        }

        pass.with_viewport(
            mev::Offset3::new(0.0, 0.0, 0.0),
            mev::Extent3::new(w, h, 1.0),
        );
        pass.with_scissor(mev::Offset2::new(0, 0), extent);
        pass.bind_vertex_buffers(0, &[&buffer]);
        pass.draw(0..self.vertices.len() as u32, 0..1);
    }
}
const WHITE: [f32; 3] = [0.84, 0.94, 1.0];
const CYAN: [f32; 3] = [0.12, 0.85, 0.88];
const PALETTE: [[f32; 3]; 5] = [
    [0.92, 0.19, 0.35],
    [1.0, 0.40, 0.20],
    [0.95, 0.72, 0.16],
    [0.15, 0.76, 0.63],
    [0.20, 0.49, 0.96],
];

struct Canvas<'a> {
    vertices: &'a mut Vec<[f32; 6]>,
    transform: Transform,
}

impl Canvas<'_> {
    fn vertex(&mut self, p: [f32; 2], c: [f32; 3]) {
        let p = self
            .transform
            .transform_point(Vector2::new(p[0] as f64, p[1] as f64));
        self.vertices
            .push([p.x as f32, p.y as f32, c[0], c[1], c[2], 1.0]);
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: [f32; 3]) {
        for p in [
            [x, y],
            [x + w, y],
            [x, y + h],
            [x, y + h],
            [x + w, y],
            [x + w, y + h],
        ] {
            self.vertex(p, c);
        }
    }

    fn circle(&mut self, x: f32, y: f32, r: f32, c: [f32; 3]) {
        for i in 0..24 {
            let a = i as f32 * std::f32::consts::TAU / 24.0;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 24.0;
            self.vertex([x, y], c);
            self.vertex([x + r * a.cos(), y + r * a.sin()], c);
            self.vertex([x + r * b.cos(), y + r * b.sin()], c);
        }
    }

    fn scene(&mut self, g: &Game, world: &World) {
        self.rect(0.0, 0.0, WIDTH, HEIGHT, [0.012, 0.022, 0.045]);
        self.rect(LEFT, TOP, RIGHT - LEFT, 572.0, [0.016, 0.030, 0.060]);
        for x in (64..920).step_by(32) {
            for y in (120..665).step_by(32) {
                self.rect(x as f32, y as f32, 1.0, 1.0, [0.035, 0.06, 0.095]);
            }
        }
        self.rect(LEFT - 3.0, TOP, 3.0, 572.0, [0.09, 0.19, 0.29]);
        self.rect(RIGHT, TOP, 3.0, 572.0, [0.09, 0.19, 0.29]);
        self.rect(
            LEFT - 3.0,
            TOP - 3.0,
            RIGHT - LEFT + 6.0,
            3.0,
            [0.09, 0.19, 0.29],
        );

        for (id, block, t, r) in world.view::<(Entities, &Block, &Transform, &Rect)>().iter() {
            if g.destroyed.contains(&id.id()) {
                continue;
            }
            self.transform = *t;
            let color = PALETTE[((50u32.saturating_sub(block.points)) / 10).min(4) as usize];
            let x = r.min.x as f32;
            let y = r.min.y as f32;
            let w = (r.max.x - r.min.x) as f32;
            let h = (r.max.y - r.min.y) as f32;
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            self.rect(x, y + 4.0, w, h, [0.008, 0.015, 0.03]);
            self.rect(x, y, w, h, color);
            self.rect(
                x + 3.0,
                y + 3.0,
                (w - 6.0).max(0.0),
                2.0,
                color.map(|v| v * 0.6 + 0.4),
            );
        }
        for (_, t, r) in world.view::<(&Paddle, &Transform, &Rect)>().iter() {
            self.transform = *t;
            let w = (r.max.x - r.min.x) as f32;
            let h = (r.max.y - r.min.y) as f32;
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            self.rect(r.min.x as f32, r.min.y as f32, w, h, CYAN);
            self.rect(
                r.min.x as f32 + 6.0,
                r.min.y as f32 + 2.0,
                (w - 12.0).max(0.0),
                3.0,
                WHITE,
            );
        }
        self.transform = Transform::default();
        for (i, p) in g.trail.iter().enumerate() {
            let t = (i + 1) as f32 / 11.0;
            self.circle(
                p[0],
                p[1],
                RADIUS * t,
                [0.025 + t * 0.06, 0.06 + t * 0.17, 0.1 + t * 0.2],
            );
        }
        if g.phase != Phase::Lost {
            let anchor = if g.phase == Phase::Ready {
                world
                    .view::<(Entities, &Paddle, &Transform, &Rect)>()
                    .iter()
                    .min_by_key(|(id, _, _, _)| id.id())
                    .map(|(_, _, t, r)| {
                        t.transform_point(Vector2::new(
                            (r.min.x + r.max.x) * 0.5,
                            r.min.y - RADIUS as f64 - 1.0,
                        ))
                    })
            } else {
                None
            };
            let p = anchor.map_or(g.ball, |p| [p.x as f32, p.y as f32]);
            self.circle(p[0], p[1], RADIUS, WHITE);
        }
        self.transform = Transform::default();
    }
}
