use std::mem::size_of;

use arcana::mev::{self, Arguments};
use miette::IntoDiagnostic;

#[derive(mev::DeviceRepr)]
struct ImageSampleConstants {
    extent: mev::vec2,
}

#[derive(mev::Arguments)]
struct ImageSampleArguments {
    #[mev(shader(fragment), sampled)]
    src: mev::Image,
    #[mev(fragment)]
    sampler: mev::Sampler,
}

#[derive(Clone)]
pub struct ImageSample {
    pipeline: mev::RenderPipeline,
    sampler: mev::Sampler,
}

impl ImageSample {
    pub fn new(device: &mev::Device) -> miette::Result<Self> {
        let library = device
            .new_shader_library(mev::LibraryDesc {
                name: "image-sampler",
                input: mev::include_library!("./shaders/sample.wgsl" as mev::ShaderLanguage::Wgsl),
            })
            .into_diagnostic()?;

        let pipeline = device
            .new_render_pipeline(mev::RenderPipelineDesc {
                name: "sample-image",
                vertex_shader: library.entry("vs_main"),
                vertex_attributes: Vec::new(),
                vertex_layouts: Vec::new(),
                primitive_topology: mev::PrimitiveTopology::Triangle,
                raster: Some(mev::RasterDesc {
                    fragment_shader: Some(library.entry("fs_main")),
                    color_targets: vec![mev::ColorTargetDesc {
                        format: mev::PixelFormat::Rgba8Srgb,
                        blend: Some(mev::BlendDesc::default()),
                    }],
                    depth_stencil: None,
                    front_face: mev::FrontFace::Clockwise,
                    culling: mev::Culling::None,
                }),
                constants: size_of::<ImageSampleConstants>(),
                arguments: &[ImageSampleArguments::LAYOUT],
            })
            .into_diagnostic()?;

        let sampler = device
            .new_sampler(mev::SamplerDesc::default())
            .into_diagnostic()?;

        Ok(ImageSample { pipeline, sampler })
    }

    pub fn sample(
        &self,
        src: mev::Image,
        dst: mev::Image,
        encoder: &mut mev::CommandEncoder,
    ) -> miette::Result<()> {
        let dims = dst.dimensions().expect_2d();
        let constants = ImageSampleConstants {
            extent: mev::vec2(dims.width() as f32, dims.height() as f32),
        };

        let args = ImageSampleArguments {
            src,
            sampler: self.sampler.clone(),
        };

        encoder.barrier(
            mev::PipelineStages::all(),
            mev::PipelineStages::FRAGMENT_SHADER,
        );
        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::FRAGMENT_SHADER,
            &dst,
        );

        let mut render = encoder.render(
            mev::RenderPassDesc::new()
                .name("image-sample")
                .color_attachments(&[mev::AttachmentDesc::new(&dst).no_load()]),
        );

        render.with_pipeline(&self.pipeline);
        render.with_arguments(0, &args);
        render.with_constants(&constants);

        render.with_viewport(
            mev::Offset3::ZERO,
            mev::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
        );
        render.with_scissor(mev::Offset2::ZERO, dims);

        render.draw(0..3, 0..1);
        drop(render);

        encoder.barrier(
            mev::PipelineStages::FRAGMENT_SHADER,
            mev::PipelineStages::all(),
        );

        Ok(())
    }
}
