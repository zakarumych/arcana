use arcana::{
    ecs::world::World,
    mev,
    model::Reflect,
    work_graph::{Exec, Image2D, Image2DReq, Job, JobDesc, Planner, Req, TargetCreateDesc},
};
use pixie_ui::{
    math::{Pos, Rect},
    style::{Attributes, Style},
    ui::UiState,
    world::WorldExt,
};

/// Draws the world's Pixie roots into a transparent, fixed-resolution image.
/// Width and height parameters use 320 and 180 when nonpositive.
pub struct PixieRender {
    pub style: Style,
    renderer: Option<pixie_ui_mev::Renderer>,
}

#[arcana::job]
impl Job for PixieRender {
    fn name() -> arcana::Name {
        arcana::format_name!("Pixie UI")
    }

    fn desc() -> JobDesc {
        JobDesc {
            params: vec![
                (arcana::name!(width), i64::model()),
                (arcana::name!(height), i64::model()),
            ],
            creates: vec![TargetCreateDesc::new::<Image2D>(arcana::format_name!(
                "Image"
            ))],
            ..JobDesc::default()
        }
    }

    fn new() -> Self {
        Self {
            style: Style::new(Attributes::default()),
            renderer: None,
        }
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        let width = i64::from_value(planner.param("width").clone())
            .ok()
            .filter(|v| *v > 0)
            .unwrap_or(320)
            .min(i32::MAX as i64) as u32;

        let height = i64::from_value(planner.param("height").clone())
            .ok()
            .filter(|v| *v > 0)
            .unwrap_or(180)
            .min(i32::MAX as i64) as u32;

        planner.create_with_req::<Image2D>(Image2DReq {
            extent: Req::Strict(mev::Extent2::new(width, height)),
            format: Req::Strict(mev::PixelFormat::Rgba8Unorm),
            usage: mev::ImageUsage::TARGET,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, world: &mut World) {
        let Some(image) = exec.create::<Image2D>() else {
            return;
        };

        world.with_resource(UiState::new).set_rect(Rect {
            lt: Pos::ZERO,
            rb: Pos {
                x: image.extent().width() as i32,
                y: image.extent().height() as i32,
            },
        });

        pixie_ui::widget::sync_widget_parents(world);
        self.style.resolve_attributes(world);
        pixie_ui::layout::layout_system(world);
        let mut draws = Vec::new();
        world.draw_ui(&mut draws);
        let encoder = exec.new_encoder();
        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::COLOR_OUTPUT,
            &image.0,
        );
        drop(encoder.render(mev::RenderPassDesc {
            name: "Clear Pixie UI",
            color_attachments: &[
                mev::AttachmentDesc::new(&image.0).clear(mev::ClearColor(0.0, 0.0, 0.0, 0.0)),
            ],
            depth_stencil_attachment: None,
        }));
        encoder.barrier(
            mev::PipelineStages::COLOR_OUTPUT,
            mev::PipelineStages::COLOR_OUTPUT,
        );
        if self.renderer.is_none() {
            self.renderer = Some(
                pixie_ui_mev::Renderer::new_recorded(exec.device(), encoder)
                    .expect("Pixie renderer initialization failed"),
            );
        }
        if let (Some(renderer), Some(ui)) = (&mut self.renderer, world.get_resource::<UiState>()) {
            renderer
                .render_recorded(encoder, &image.0, &draws, &ui)
                .expect("Pixie rendering failed");
        }
    }
}
