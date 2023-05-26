use bob::{
    blink_alloc::BlinkAlloc,
    edict::{Res, ResMutNoSend, Scheduler, World},
    egui::{EguiRender, EguiResource},
    gametime::ClockStep,
    nix::{self, Arguments, Constants},
    plugin::BobPlugin,
    render::{Render, RenderBuilderContext, RenderContext, RenderError, RenderGraph, TargetId},
    winit::window::Window,
};

#[derive(nix::Arguments)]
pub struct MainArguments {
    #[nix(vertex)]
    pub colors: nix::Buffer,
}

#[derive(nix::Constants)]
pub struct MainConstants {
    pub angle: f32,
    pub width: u32,
    pub height: u32,
}

pub struct MainPass {
    target: TargetId<nix::Image>,
    pipeline: Option<nix::RenderPipeline>,
    arguments: Option<MainArguments>,
    constants: MainConstants,
}

impl MainPass {
    fn build(graph: &mut RenderGraph) -> TargetId<nix::Image> {
        // Start building render.
        let mut builder = RenderBuilderContext::new("main_pass", graph);

        // This render defines a single render target.
        let target = builder.create_target("main", nix::PipelineStages::COLOR_OUTPUT);

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
                .new_shader_library(nix::LibraryDesc {
                    name: "main",
                    input: nix::include_library!("shaders/main.wgsl" as nix::ShaderLanguage::Wgsl),
                })
                .unwrap();

            ctx.device()
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
                            format: target.format(),
                            blend: Some(nix::BlendDesc::default()),
                        }],
                        depth_stencil: None,
                        front_face: nix::FrontFace::default(),
                        culling: nix::Culling::Back,
                    }),
                    arguments: &[MainArguments::LAYOUT],
                    constants: MainConstants::SIZE,
                })
                .unwrap()
        });

        let mut render = encoder.render(nix::RenderPassDesc {
            color_attachments: &[
                nix::AttachmentDesc::new(&target).clear(nix::ClearColor(1.0, 0.5, 0.3, 0.0))
            ],
            ..Default::default()
        });

        let dims = target.dimensions().to_2d();

        let arguments = self.arguments.get_or_insert_with(|| {
            let colors = ctx
                .device()
                .new_buffer_init(nix::BufferInitDesc {
                    data: bob::bytemuck::cast_slice(&[
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
                    usage: nix::BufferUsage::UNIFORM,
                    memory: nix::Memory::Shared,
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
            nix::Offset3::ZERO,
            nix::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
        );
        render.with_scissor(nix::Offset2::ZERO, dims);
        render.draw(0..3, 0..1);
        drop(render);
        ctx.commit(encoder.finish()?);
        Ok(())
    }
}

pub struct GamePlugin;

impl BobPlugin for GamePlugin {
    fn name(&self) -> &'static str {
        "triangle"
    }

    fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
        let world = world.local();

        let window = world.expect_resource::<Window>().id();

        let mut graph = world.expect_resource_mut::<RenderGraph>();
        // Create main pass.
        // It returns target id that it renders to.
        let mut target = MainPass::build(&mut graph);

        if world.get_resource::<EguiResource>().is_some() {
            target = EguiRender::build_overlay(target, &mut graph, window);
        }

        // Use window's surface for the render target.
        graph.present(target, window);
        drop(graph);

        scheduler.add_system(
            move |mut egui: ResMutNoSend<EguiResource>, window: Res<Window>| {
                egui.run(&window, |ctx| {
                    bob::egui::Window::new("Hello triangle!")
                        .resizable(false)
                        .collapsible(true)
                        .show(ctx, |ui| {
                            ui.label("Hello triangle!");
                        });
                });
            },
        );
    }
}

bob::export_bob_plugins![GamePlugin];
