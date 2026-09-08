use crate::game::*;
use arcana::{
    ecs::world::World,
    mev,
    work_graph::{Exec, Image2D, Job, JobDesc, Planner, TargetCreateDesc},
};

pub struct BlockbreakerFrame {
    pipeline: Option<(mev::PixelFormat, mev::RenderPipeline)>,
    vertices: Vec<[f32; 6]>,
}

impl Job for BlockbreakerFrame {
    fn name() -> arcana::Name {
        arcana::name!("Blockbreaker frame")
    }

    fn desc() -> JobDesc {
        JobDesc {
            creates: vec![TargetCreateDesc::new::<Image2D>(arcana::name!("Frame"))],
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
        planner.create::<Image2D>();
    }

    fn exec(&mut self, exec: Exec<'_>, world: &mut World) {
        let Some(target) = exec.create::<Image2D>() else {
            return;
        };

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
        };

        canvas.scene(&game);

        let w = extent.width() as f32;
        let h = extent.height() as f32;
        let scale = (w / WIDTH).min(h / HEIGHT);

        for v in &mut self.vertices {
            v[0] = ((v[0] - WIDTH / 2.0) * scale) * 2.0 / w;
            v[1] = -((v[1] - HEIGHT / 2.0) * scale) * 2.0 / h;
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
const MUTED: [f32; 3] = [0.34, 0.46, 0.60];
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
}

impl Canvas<'_> {
    fn vertex(&mut self, p: [f32; 2], c: [f32; 3]) {
        self.vertices.push([p[0], p[1], c[0], c[1], c[2], 1.0]);
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

    fn text(&mut self, text: &str, x: f32, y: f32, size: f32, c: [f32; 3]) {
        for (i, ch) in text.chars().enumerate() {
            for (row, bits) in glyph(ch).into_iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.rect(
                            x + (i * 6 + col) as f32 * size,
                            y + row as f32 * size,
                            size,
                            size,
                            c,
                        );
                    }
                }
            }
        }
    }

    fn centered(&mut self, text: &str, y: f32, size: f32, c: [f32; 3]) {
        self.text(
            text,
            (WIDTH - (text.len() as f32 * 6.0 - 1.0) * size) * 0.5,
            y,
            size,
            c,
        );
    }

    fn scene(&mut self, g: &Game) {
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

        self.text("BLOCK", 40.0, 30.0, 4.0, WHITE);
        self.text("BREAKER", 174.0, 30.0, 4.0, CYAN);
        self.text("CLEAR THE WALL", 42.0, 70.0, 2.0, MUTED);
        self.text("SCORE", 560.0, 28.0, 2.0, MUTED);
        self.text(&format!("{:04}", g.score), 560.0, 49.0, 3.0, WHITE);
        self.text("LIVES", 786.0, 28.0, 2.0, MUTED);

        for i in 0..3 {
            self.circle(
                796.0 + i as f32 * 28.0,
                60.0,
                7.0,
                if i < g.lives {
                    CYAN
                } else {
                    [0.05, 0.10, 0.16]
                },
            );
        }

        for brick in &g.bricks {
            if brick.alive {
                self.rect(
                    brick.x,
                    brick.y + 4.0,
                    BRICK_W,
                    BRICK_H,
                    [0.008, 0.015, 0.03],
                );
                self.rect(brick.x, brick.y, BRICK_W, BRICK_H, PALETTE[brick.row]);
                self.rect(
                    brick.x + 3.0,
                    brick.y + 3.0,
                    BRICK_W - 6.0,
                    2.0,
                    PALETTE[brick.row].map(|v| v * 0.6 + 0.4),
                );
            }
        }

        for (i, p) in g.trail.iter().enumerate() {
            let t = (i + 1) as f32 / 11.0;
            self.circle(
                p[0],
                p[1],
                RADIUS * t,
                [0.025 + t * 0.06, 0.06 + t * 0.17, 0.1 + t * 0.2],
            );
        }

        self.rect(
            g.paddle - PADDLE_HALF - 2.0,
            PADDLE_Y + 4.0,
            PADDLE_HALF * 2.0 + 4.0,
            14.0,
            [0.03, 0.09, 0.14],
        );

        self.rect(
            g.paddle - PADDLE_HALF,
            PADDLE_Y,
            PADDLE_HALF * 2.0,
            12.0,
            CYAN,
        );

        self.rect(
            g.paddle - PADDLE_HALF + 6.0,
            PADDLE_Y + 2.0,
            PADDLE_HALF * 2.0 - 12.0,
            3.0,
            WHITE,
        );

        if !matches!(g.phase, Phase::Lost) {
            self.circle(g.ball[0], g.ball[1], RADIUS, WHITE);
        }

        self.centered(
            "A D / ARROWS  MOVE     SPACE  LAUNCH     R  RESTART",
            688.0,
            2.0,
            MUTED,
        );

        match g.phase {
            Phase::Ready => {
                self.centered("READY TO BREAK?", 410.0, 3.0, WHITE);
                self.centered("PRESS SPACE TO LAUNCH", 452.0, 2.0, CYAN);
            }
            Phase::Won | Phase::Lost => {
                self.rect(220.0, 374.0, 520.0, 142.0, [0.027, 0.052, 0.09]);
                self.centered(
                    if g.phase == Phase::Won {
                        "WALL CLEARED!"
                    } else {
                        "GAME OVER"
                    },
                    398.0,
                    4.0,
                    if g.phase == Phase::Won {
                        CYAN
                    } else {
                        PALETTE[0]
                    },
                );
                self.centered(&format!("FINAL SCORE  {}", g.score), 449.0, 2.0, WHITE);
                self.centered("PRESS R TO PLAY AGAIN", 484.0, 2.0, MUTED);
            }
            Phase::Playing => {}
        }
    }
}

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 2, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '!' => [4, 4, 4, 4, 4, 0, 4],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        _ => [0; 7],
    }
}
