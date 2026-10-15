use arcana_ecs::world::World;
use arcana_model::{Model, Reflect, Value, ValueError};
use arcana_work_graph::{
    Exec, Image2D, Image2DReq, Job, JobDesc, JobId, Planner, Req, TargetReadDesc, TargetUpdateDesc,
};
use mev::{Arguments, DeviceRepr};

/// Scaling of a centered image overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayScale {
    /// Scale independently along each axis.
    Stretch,
    /// Preserve aspect ratio and fit the target width.
    Width,
    /// Preserve aspect ratio and fit the target height.
    Height,
    /// Smaller width/height ratio: fit entirely inside the target.
    #[default]
    Min,
    /// Larger width/height ratio: cover the target, cropping excess.
    Max,
}

impl OverlayScale {
    const VARIANTS: [Self; 5] = [
        Self::Min,
        Self::Stretch,
        Self::Width,
        Self::Height,
        Self::Max,
    ];

    fn ident(self) -> crate::Ident {
        match self {
            Self::Stretch => crate::ident!(Stretch),
            Self::Width => crate::ident!(Width),
            Self::Height => crate::ident!(Height),
            Self::Min => crate::ident!(Min),
            Self::Max => crate::ident!(Max),
        }
    }

    /// Returns per-axis scale. Integer snapping floors each scale and clamps
    /// it to at least one, so undersized targets crop instead of downsampling.
    pub fn factors(self, source: mev::Extent2, target: mev::Extent2, integer: bool) -> [f32; 2] {
        let x = target.width() as f32 / source.width().max(1) as f32;
        let y = target.height() as f32 / source.height().max(1) as f32;
        let scale = match self {
            Self::Stretch => [x, y],
            Self::Width => [x; 2],
            Self::Height => [y; 2],
            Self::Min => [x.min(y); 2],
            Self::Max => [x.max(y); 2],
        };
        if integer {
            scale.map(|v| v.floor().max(1.0))
        } else {
            scale
        }
    }
}

impl Reflect for OverlayScale {
    fn model() -> Model {
        Model::Enum(
            Self::VARIANTS
                .into_iter()
                .map(|v| (v.ident(), Some(Model::Unit)))
                .collect(),
        )
    }

    fn to_value(&self) -> Value {
        Value::Enum(self.ident(), Box::new(Value::Unit))
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        if let Value::Enum(name, payload) = value {
            if matches!(*payload, Value::Unit) {
                if let Some(v) = Self::VARIANTS.into_iter().find(|v| v.ident() == name) {
                    return Ok(v);
                }
            }
        }
        Err(ValueError::Custom("Expected overlay scale variant".into()))
    }
}

#[derive(mev::Arguments)]
struct OverlayArguments {
    #[mev(fragment)]
    sampler: mev::Sampler,
    #[mev(sampled, fragment)]
    image: mev::Image2D,
}

#[derive(mev::DeviceRepr)]
struct OverlayConstants {
    size: [f32; 2],
}

/// Nearest-sampled, centered source-over overlay of premultiplied RGBA.
/// Updates an initialized target, preserving its pixels outside the overlay.
/// Parameters are `Scale` (default `Min`) and `Integer scale` (default false).
pub struct OverlayImage {
    pipeline: Option<(mev::PixelFormat, mev::RenderPipeline)>,
    sampler: Option<mev::Sampler>,
}

impl OverlayImage {
    pub fn id() -> JobId {
        arcana_id::hash_id!("arcana::render::OverlayImage" => JobId)
    }
}

impl Job for OverlayImage {
    fn name() -> crate::Name {
        crate::Name::from_name_static("Overlay image")
    }

    fn desc() -> JobDesc {
        JobDesc {
            params: vec![
                (
                    crate::Name::from_name_static("Scale"),
                    OverlayScale::model(),
                ),
                (crate::Name::from_name_static("Integer scale"), Model::Bool),
            ],
            reads: vec![TargetReadDesc::new::<Image2D>(crate::name!(Source))],
            updates: vec![TargetUpdateDesc::new::<Image2D>(
                crate::Name::from_name_static("Target"),
            )],
            creates: vec![],
        }
    }

    fn new() -> Self {
        Self {
            pipeline: None,
            sampler: None,
        }
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        if planner.update::<Image2D>().is_none() {
            return;
        }
        planner.read::<Image2D>(Image2DReq {
            extent: Req::Suggest(mev::Extent2::new(1, 1)),
            format: Req::Suggest(mev::PixelFormat::Rgba8Unorm),
            usage: mev::ImageUsage::SAMPLED,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, _: &mut World) {
        let Some(target) = exec.update::<Image2D>() else {
            return;
        };
        let Some(source) = exec.read::<Image2D>() else {
            return;
        };
        let scale = match OverlayScale::from_value(exec.param("Scale").clone()) {
            Ok(scale) => scale,
            Err(error) => {
                tracing::error!("Invalid overlay scale: {error}");
                return;
            }
        };
        let integer = match exec.param("Integer scale") {
            Value::Bool(value) => *value,
            _ => {
                tracing::error!("Invalid overlay integer scale");
                return;
            }
        };
        let extent = target.extent();
        let source_extent = source.extent();
        if extent.width() == 0
            || extent.height() == 0
            || source_extent.width() == 0
            || source_extent.height() == 0
        {
            return;
        }
        let format = target.format();
        if self.pipeline.as_ref().is_none_or(|(f, _)| *f != format) {
            let library = exec
                .device()
                .new_shader_library(mev::LibraryDesc {
                    name: "Overlay image",
                    input: mev::LibraryInput::wgsl(include_str!("overlay.wgsl")),
                })
                .expect("Overlay shader must compile");
            let pipeline = exec
                .device()
                .new_render_pipeline(mev::RenderPipelineDesc {
                    name: "Overlay image",
                    vertex_shader: library.entry("vertex_main"),
                    vertex_layouts: vec![],
                    vertex_attributes: vec![],
                    primitive_topology: mev::PrimitiveTopology::Triangle,
                    raster: Some(mev::RasterDesc {
                        fragment_shader: Some(library.entry("fragment_main")),
                        color_targets: vec![mev::ColorTargetDesc::new(format).alpha_blend()],
                        depth_stencil: None,
                        front_face: mev::FrontFace::default(),
                        culling: mev::Culling::None,
                    }),
                    constants: OverlayConstants::SIZE,
                    arguments: &[OverlayArguments::LAYOUT],
                })
                .expect("Overlay pipeline must compile");
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
        let factors = scale.factors(source_extent, extent, integer);
        let constants = OverlayConstants {
            size: [
                source_extent.width() as f32 * factors[0] / extent.width() as f32,
                source_extent.height() as f32 * factors[1] / extent.height() as f32,
            ],
        };
        let encoder = exec.new_encoder();
        encoder.barrier(
            mev::PipelineStages::all(),
            mev::PipelineStages::FRAGMENT_SHADER | mev::PipelineStages::COLOR_OUTPUT,
        );
        let mut pass = encoder.render(mev::RenderPassDesc {
            name: "Overlay image",
            color_attachments: &[mev::AttachmentDesc::new(&target.0)],
            depth_stencil_attachment: None,
        });
        if let Some((_, pipeline)) = &self.pipeline {
            pass.with_pipeline(pipeline);
        }
        pass.with_viewport(
            mev::Offset3::new(0.0, 0.0, 0.0),
            mev::Extent3::new(extent.width() as f32, extent.height() as f32, 1.0),
        );
        pass.with_scissor(mev::Offset2::new(0, 0), extent);
        pass.with_constants(&constants);
        pass.with_arguments(
            0,
            &OverlayArguments {
                sampler: sampler.clone(),
                image: source.0.clone(),
            },
        );
        pass.draw(0..6, 0..1);
    }
}
