use arcana::mev;

use crate::Font;

/// Binding-independent WGSL helpers for sampling distance-field glyphs.
///
/// Concatenate this source before a render job's shader source, for example
/// `format!("{RASTERIZER_WGSL}\n{job_shader}")`. Bind the uploaded atlas as a
/// `texture_2d<f32>` with a normalized linear sampler, without anisotropy or
/// mipmaps. The atlas is linear RGBA8, never sRGB.
///
/// `arcana_font_sample` takes atlas UV coordinates, the glyph's atlas rectangle
/// `(x, y, width, height)` in texels, the full distance range in screen pixels,
/// and `is_msdf`. For uniform scaling, the screen range is
/// `font.distance_range() * output_pixels_per_em / font.pixels_per_em()`.
/// Coverage clamps this range to at least one screen pixel.
/// `arcana_font_screen_range` derives it from unclamped UV derivatives in a
/// fragment shader when the transform is not known on the CPU.
///
/// ```wgsl
/// @group(0) @binding(0) var atlas: texture_2d<f32>;
/// @group(0) @binding(1) var atlas_sampler: sampler;
///
/// // Fragment shader body; rect, distance_range, is_msdf and color are
/// // supplied by the render job. uv uses the atlas's top-down coordinates.
/// // let range = arcana_font_screen_range(uv,
/// //     vec2<f32>(textureDimensions(atlas)), distance_range);
/// // let alpha = color.a * arcana_font_sample(atlas, atlas_sampler,
/// //     uv, rect, range, is_msdf);
/// // return vec4<f32>(color.rgb * alpha, alpha); // premultiplied alpha
/// ```
pub const RASTERIZER_WGSL: &str = include_str!("rasterizer.wgsl");

impl Font {
    /// Records an atlas upload and returns its sampled, single-level image.
    ///
    /// The encoder must belong to `device` and a queue supporting transfer and
    /// `next_stages`, the shader stages that will sample the atlas. For a render
    /// job's fragment shader, pass `mev::PipelineStages::FRAGMENT_SHADER`.
    /// Submit this encoder before
    /// sampling the returned image on that queue. The command buffer retains
    /// the staging allocation until GPU execution completes. Each call creates
    /// an independent image; callers can cache the result for their device.
    pub fn upload(
        &self,
        device: &mev::Device,
        encoder: &mut mev::CommandEncoder,
        next_stages: mev::PipelineStages,
    ) -> mev::Image2D {
        let image = device.new_image(
            mev::ImageDesc::new_d2_texture(
                self.width(),
                self.height(),
                mev::PixelFormat::Rgba8Unorm,
            )
            .with_name("font distance-field atlas"),
        );
        // WebGPU buffer-to-texture copies require 256-byte row alignment.
        let row_bytes = self.width() as usize * 4;
        let row_stride = row_bytes.next_multiple_of(256);
        let upload = if row_bytes == row_stride {
            std::borrow::Cow::Borrowed(self.pixels())
        } else {
            let mut padded = vec![0; row_stride * self.height() as usize];
            for (src, dst) in self
                .pixels()
                .chunks_exact(row_bytes)
                .zip(padded.chunks_exact_mut(row_stride))
            {
                dst[..row_bytes].copy_from_slice(src);
            }
            std::borrow::Cow::Owned(padded)
        };
        let staging = device.new_buffer_init(mev::BufferInitDesc {
            data: upload.as_ref(),
            usage: mev::BufferUsage::TRANSFER_SRC | mev::BufferUsage::HOST_WRITE,
            name: "font atlas upload",
        });
        encoder.init_image(
            mev::PipelineStages::empty(),
            mev::PipelineStages::TRANSFER,
            &image,
        );
        encoder.copy().copy_buffer_to_image(
            &staging,
            mev::TexelShape::new(mev::PixelFormat::Rgba8Unorm, image.extent())
                .with_row_stride(row_stride),
            &image,
        );
        encoder.barrier(mev::PipelineStages::TRANSFER, next_stages);
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FieldKind, FontMetrics, Glyph, asset::FontData};
    use mev::Arguments as _;

    #[derive(mev::Arguments)]
    struct AtlasArguments {
        #[mev(sampled, fragment)]
        atlas: mev::Image2D,
        #[mev(fragment)]
        sampler: mev::Sampler,
    }

    #[test]
    #[ignore = "Requires a native graphics device; uploads, renders and reads back both font modes"]
    fn native_upload_and_raster_match_cpu() {
        let instance = mev::Instance::load().expect("native graphics instance");
        let (index, family) = instance
            .capabilities()
            .devices
            .iter()
            .enumerate()
            .find_map(|(index, caps)| {
                caps.families
                    .iter()
                    .position(|family| family.queue_flags.contains(mev::QueueFlags::GRAPHICS))
                    .map(|family| (index, family as u32))
            })
            .unwrap_or_else(|| {
                panic!(
                    "graphics queue family; capabilities: {:?}",
                    instance.capabilities()
                )
            });
        let (device, mut queues) = instance
            .new_device(mev::DeviceDesc {
                idx: index,
                queues: &[family],
                features: mev::Features::empty(),
            })
            .expect("native graphics device");
        let mut queue = queues.pop().expect("requested queue");

        for kind in [FieldKind::Sdf, FieldKind::Msdf] {
            let mut pixels = vec![255; 6 * 5 * 4];
            // Different RGB channels and asymmetric rows distinguish median
            // decoding, red decoding, Y orientation and neighboring glyph bleed.
            let values = [28u8, 96, 154, 209, 192, 111, 69, 171, 145, 58, 218, 123];
            for y in 0..3 {
                for x in 0..4 {
                    let at = ((y + 1) * 6 + x + 1) * 4;
                    pixels[at..at + 4].copy_from_slice(&[
                        255 - values[y * 4 + x],
                        values[y * 4 + x],
                        230,
                        255,
                    ]);
                }
            }
            pixels[28..32].copy_from_slice(&[0, 0, 0, 255]);
            pixels[40..44].copy_from_slice(&[255, 255, 255, 255]);
            let font = Font::new(FontData {
                width: 6,
                height: 5,
                pixels,
                kind,
                distance_range: 1.0,
                pixels_per_em: 2.0,
                metrics: FontMetrics {
                    ascender: 3.0,
                    descender: 0.0,
                    line_height: 3.0,
                    underline_y: 0.0,
                    underline_thickness: 0.1,
                },
                glyphs: vec![Glyph {
                    character: 'F',
                    glyph_id: 1,
                    advance: 2.0,
                    plane_bounds: [0.0, 0.0, 2.0, 1.5],
                    atlas_bounds: [1, 1, 4, 3],
                }],
            })
            .expect("synthetic font");
            for scale in [1.0, 2.0, 4.0, 6.0] {
                let cpu = font.rasterize_glyph('F', scale).expect("CPU raster");
                let source = format!(
                    "{RASTERIZER_WGSL}\n{}",
                    format!(
                        r#"
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {{
    let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(positions[index], 0.0, 1.0);
}}
@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {{
    let plane = (position.xy + vec2<f32>(0.0, {offset_y})) / {scale} + vec2<f32>(0.0, 1.5);
    let uv = (vec2<f32>(1.0) + plane * 2.0) / vec2<f32>(6.0, 5.0);
    let alpha = arcana_font_sample(atlas, atlas_sampler, uv, vec4<u32>(1, 1, 4, 3), {range}, {msdf});
    return vec4<f32>(alpha);
}}
"#,
                        scale = format!("{scale:.1}"),
                        offset_y = format!("{:.1}", cpu.offset[1] as f32),
                        range = format!("{:.1}", scale / 2.0),
                        msdf = kind == FieldKind::Msdf
                    )
                );
                let library = device
                    .new_shader_library(mev::LibraryDesc {
                        name: "font parity",
                        input: mev::LibraryInput::wgsl(source),
                    })
                    .expect("font shader compilation");
                let pipeline = device
                    .new_render_pipeline(
                        mev::RenderPipelineDesc::new("font parity", library.entry("vertex"))
                            .raster(mev::RasterDesc {
                                fragment_shader: Some(library.entry("fragment")),
                                color_targets: vec![mev::ColorTargetDesc::new(
                                    mev::PixelFormat::Rgba8Unorm,
                                )],
                                culling: mev::Culling::None,
                                ..Default::default()
                            })
                            .arguments(&[AtlasArguments::LAYOUT]),
                    )
                    .expect("font render pipeline");
                let target = device.new_image(mev::ImageDesc::new_d2(
                    cpu.width,
                    cpu.height,
                    mev::PixelFormat::Rgba8Unorm,
                    mev::ImageUsage::TARGET | mev::ImageUsage::TRANSFER_SRC,
                ));
                let mut readback = device.new_buffer(mev::BufferDesc {
                    size: cpu.pixels.len() * 4,
                    usage: mev::BufferUsage::TRANSFER_DST | mev::BufferUsage::HOST_READ,
                    name: "font parity readback",
                });
                let mut encoder = queue.new_command_encoder();
                let atlas =
                    font.upload(&device, &mut encoder, mev::PipelineStages::FRAGMENT_SHADER);
                let sampler = device.new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Linear,
                    mag_filter: mev::Filter::Linear,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..Default::default()
                });
                encoder.init_image(
                    mev::PipelineStages::empty(),
                    mev::PipelineStages::COLOR_OUTPUT,
                    &target,
                );
                {
                    let attachments = [mev::AttachmentDesc::new(target.as_image())
                        .clear(mev::ClearColor::TRANSPARENT)];
                    let mut render = encoder.render(mev::RenderPassDesc {
                        name: "font parity",
                        color_attachments: &attachments,
                        depth_stencil_attachment: None,
                    });
                    render.with_pipeline(&pipeline);
                    render.with_viewport(
                        mev::Offset3::new(0.0, 0.0, 0.0),
                        mev::Extent3::new(cpu.width as f32, cpu.height as f32, 1.0),
                    );
                    render.with_scissor(mev::Offset2::new(0, 0), target.extent());
                    render.with_arguments(0, &AtlasArguments { atlas, sampler });
                    render.draw(0..3, 0..1);
                }
                encoder.barrier(
                    mev::PipelineStages::COLOR_OUTPUT,
                    mev::PipelineStages::TRANSFER,
                );
                encoder.copy().copy_image_to_buffer(
                    &target,
                    &readback,
                    mev::TexelShape::new(mev::PixelFormat::Rgba8Unorm, target.extent()),
                );
                queue
                    .submit_checkpoint([encoder.finish()])
                    .expect("font render submission");
                queue.wait_idle().expect("font render completion");
                let mut actual = vec![0; cpu.pixels.len() * 4];
                readback.read(0, &mut actual).expect("font readback");
                for (index, (&expected, pixel)) in
                    cpu.pixels.iter().zip(actual.chunks_exact(4)).enumerate()
                {
                    assert!(
                        pixel.iter().all(|&value| value.abs_diff(expected) <= 2),
                        "{kind:?}, scale {scale}, pixel {index}: CPU {expected}, GPU {pixel:?}"
                    );
                }
            }
        }
    }
}
