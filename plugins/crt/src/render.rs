use arcana::{
    ecs::world::World,
    mev,
    model::Value,
    work_graph::{
        Exec, Image2D, Image2DReq, Job, JobDesc, Planner, Req, TargetCreateDesc, TargetReadDesc,
    },
};
use mev::{Arguments, DeviceRepr};

const DEFAULT_WIDTH: u32 = 320;
const DEFAULT_HEIGHT: u32 = 180;

#[derive(Clone, Copy, Debug, PartialEq)]
struct CrtParameters {
    width: u32,
    height: u32,
    curvature: f32,
}

impl CrtParameters {
    fn from_values(width: &Value, height: &Value, curvature: &Value) -> Self {
        Self {
            width: positive_extent(width, DEFAULT_WIDTH),
            height: positive_extent(height, DEFAULT_HEIGHT),
            curvature: nonnegative_finite(curvature),
        }
    }
}

fn positive_extent(value: &Value, fallback: u32) -> u32 {
    match value {
        Value::Int(value) if *value > 0 => (*value).min(i32::MAX as i64) as u32,
        _ => fallback,
    }
}

fn nonnegative_finite(value: &Value) -> f32 {
    match value {
        Value::Float(value) if value.is_finite() && *value > 0.0 => {
            value.min(f32::MAX as f64).to_owned() as f32
        }
        _ => 0.0,
    }
}

#[derive(mev::Arguments)]
struct CrtArguments {
    #[mev(fragment)]
    sampler: mev::Sampler,
    #[mev(sampled, fragment)]
    source: mev::Image2D,
}

#[derive(mev::DeviceRepr)]
struct CrtConstants {
    source_size: [f32; 2],
    curvature: f32,
}

fn create_pipeline(device: &mev::Device, format: mev::PixelFormat) -> mev::RenderPipeline {
    let library = device
        .new_shader_library(mev::LibraryDesc {
            name: "CRT screen",
            input: mev::LibraryInput::wgsl(include_str!("crt.wgsl")),
        })
        .expect("CRT shader must compile");
    device
        .new_render_pipeline(mev::RenderPipelineDesc {
            name: "CRT screen",
            vertex_shader: library.entry("vertex_main"),
            vertex_layouts: vec![],
            vertex_attributes: vec![],
            primitive_topology: mev::PrimitiveTopology::Triangle,
            raster: Some(mev::RasterDesc {
                fragment_shader: Some(library.entry("fragment_main")),
                color_targets: vec![mev::ColorTargetDesc::new(format)],
                depth_stencil: None,
                front_face: mev::FrontFace::default(),
                culling: mev::Culling::None,
            }),
            constants: CrtConstants::SIZE,
            arguments: &[CrtArguments::LAYOUT],
        })
        .expect("CRT pipeline must compile")
}

/// Renders a sampled image as discrete luminous pixels on a curved CRT surface.
/// Width and height default to 320 by 180 when nonpositive. Curvature is
/// nonnegative, and zero produces an exactly flat screen.
pub struct CrtScreen {
    pipeline: Option<(mev::PixelFormat, mev::RenderPipeline)>,
    sampler: Option<mev::Sampler>,
}

#[arcana::job]
impl Job for CrtScreen {
    fn name() -> arcana::Name {
        arcana::format_name!("CRT screen")
    }

    fn desc() -> JobDesc {
        JobDesc {
            params: vec![
                (arcana::name!(width), arcana::model::Model::Int),
                (arcana::name!(height), arcana::model::Model::Int),
                (arcana::name!(curvature), arcana::model::Model::Float),
            ],
            reads: vec![TargetReadDesc::new::<Image2D>(arcana::name!(Source))],
            updates: vec![],
            creates: vec![TargetCreateDesc::new::<Image2D>(arcana::name!(Image))],
        }
    }

    fn new() -> Self {
        Self {
            pipeline: None,
            sampler: None,
        }
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        let params = CrtParameters::from_values(
            planner.param("width"),
            planner.param("height"),
            planner.param("curvature"),
        );

        let extent = mev::Extent2::new(params.width, params.height);
        if planner
            .create_with_req::<Image2D>(Image2DReq {
                extent: Req::Suggest(extent),
                format: Req::Suggest(mev::PixelFormat::Rgba8Unorm),
                usage: mev::ImageUsage::TARGET,
            })
            .is_none()
        {
            return;
        }

        planner.read::<Image2D>(Image2DReq {
            extent: Req::Suggest(extent),
            format: Req::Suggest(mev::PixelFormat::Rgba8Unorm),
            usage: mev::ImageUsage::SAMPLED,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, _: &mut World) {
        let Some(output) = exec.create::<Image2D>() else {
            return;
        };

        let Some(source) = exec.read::<Image2D>() else {
            return;
        };

        let output_extent = output.extent();
        let source_extent = source.extent();
        if output_extent.width() == 0
            || output_extent.height() == 0
            || source_extent.width() == 0
            || source_extent.height() == 0
        {
            return;
        }

        let format = output.format();
        if self
            .pipeline
            .as_ref()
            .is_none_or(|(cached, _)| *cached != format)
        {
            let pipeline = create_pipeline(exec.device(), format);
            self.pipeline = Some((format, pipeline));
        }
        let sampler = self.sampler.get_or_insert_with(|| {
            exec.device().new_sampler(mev::SamplerDesc {
                min_filter: mev::Filter::Nearest,
                mag_filter: mev::Filter::Nearest,
                address_mode: [mev::AddressMode::ClampToEdge; 3],
                ..mev::SamplerDesc::default()
            })
        });
        let params = CrtParameters::from_values(
            exec.param("width"),
            exec.param("height"),
            exec.param("curvature"),
        );
        let constants = CrtConstants {
            source_size: [source_extent.width() as f32, source_extent.height() as f32],
            curvature: params.curvature,
        };
        let encoder = exec.new_encoder();
        encoder.barrier(
            mev::PipelineStages::COLOR_OUTPUT,
            mev::PipelineStages::FRAGMENT_SHADER,
        );
        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::COLOR_OUTPUT,
            &output.0,
        );
        encoder.barrier(
            mev::PipelineStages::all(),
            mev::PipelineStages::FRAGMENT_SHADER | mev::PipelineStages::COLOR_OUTPUT,
        );
        let mut pass = encoder.render(mev::RenderPassDesc {
            name: "CRT screen",
            color_attachments: &[
                mev::AttachmentDesc::new(&output.0).clear(mev::ClearColor::TRANSPARENT)
            ],
            depth_stencil_attachment: None,
        });
        if let Some((_, pipeline)) = &self.pipeline {
            pass.with_pipeline(pipeline);
        }
        pass.with_viewport(
            mev::Offset3::new(0.0, 0.0, 0.0),
            mev::Extent3::new(
                output_extent.width() as f32,
                output_extent.height() as f32,
                1.0,
            ),
        );
        pass.with_scissor(mev::Offset2::new(0, 0), output_extent);
        pass.with_constants(&constants);
        pass.with_arguments(
            0,
            &CrtArguments {
                sampler: sampler.clone(),
                source: source.0.clone(),
            },
        );
        pass.draw(0..6, 0..1);
    }
}
