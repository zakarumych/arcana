use std::mem::{offset_of, size_of_val};

use arcana::{
    bytemuck,
    mev::{self, Arguments, DeviceRepr},
};
use egui::epaint::Vertex;
use hashbrown::{hash_map::Entry, HashMap};

use super::Sampler;

#[derive(mev::Arguments)]
struct EguiArguments {
    #[mev(fragment)]
    sampler: mev::Sampler,
    #[mev(fragment)]
    texture: mev::Image,
}

#[derive(mev::DeviceRepr)]
struct EguiConstants {
    width: u32,
    height: u32,
    scale: f32,
}

pub struct Render {
    samplers: Option<[mev::Sampler; 4]>,
    library: Option<mev::Library>,
    linear_pipeline: Option<mev::RenderPipeline>,
    srgb_pipeline: Option<mev::RenderPipeline>,

    vertex_buffer: Option<mev::Buffer>,
    index_buffer: Option<mev::Buffer>,
}

impl Render {
    pub const fn new() -> Self {
        Render {
            samplers: None,
            library: None,
            linear_pipeline: None,
            srgb_pipeline: None,
            vertex_buffer: None,
            index_buffer: None,
        }
    }

    pub fn render(
        &mut self,
        cx: &egui::Context,
        mut frame: mev::Frame,
        queue: &mut mev::Queue,
        textures: &mut HashMap<egui::TextureId, (mev::Image, Sampler)>,
        textures_delta: &mut egui::TexturesDelta,
        shapes: Vec<egui::epaint::ClippedShape>,
        pixels_per_point: f32,
    ) -> Result<(), mev::DeviceError> {
        let samplers = match &mut self.samplers {
            Some(samplers) => &*samplers,
            none => {
                let sampler_nn = queue.new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Nearest,
                    mag_filter: mev::Filter::Nearest,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                let sampler_nl = queue.new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Nearest,
                    mag_filter: mev::Filter::Linear,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                let sampler_ln = queue.new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Linear,
                    mag_filter: mev::Filter::Nearest,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                let sampler_ll = queue.new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Linear,
                    mag_filter: mev::Filter::Linear,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                none.get_or_insert([sampler_nn, sampler_nl, sampler_ln, sampler_ll])
            }
        };

        let mut encoder = queue.new_command_encoder()?;

        encoder.init_image(
            mev::PipelineStages::empty(),
            mev::PipelineStages::FRAGMENT_SHADER,
            &frame.image(),
        );

        {
            let mut copy_encoder = encoder.copy();

            copy_encoder.barrier(
                mev::PipelineStages::VERTEX_INPUT | mev::PipelineStages::FRAGMENT_SHADER,
                mev::PipelineStages::TRANSFER,
            );

            if !textures_delta.set.is_empty() {
                let delta_size = textures_delta.set.iter().fold(0, |acc, (_, delta)| {
                    acc + match &delta.image {
                        egui::ImageData::Color(color) => std::mem::size_of_val(&color.pixels[..]),
                        egui::ImageData::Font(font) => std::mem::size_of_val(&font.pixels[..]),
                    }
                });

                let mut upload_buffer = queue.new_buffer(mev::BufferDesc {
                    size: delta_size,
                    usage: mev::BufferUsage::TRANSFER_SRC,
                    memory: mev::Memory::Upload,
                    name: "texture-delta-upload",
                })?;

                let mut offset = 0usize;
                for (_, delta) in textures_delta.set.iter() {
                    match &delta.image {
                        egui::ImageData::Color(color) => unsafe {
                            upload_buffer
                                .write_unchecked(offset, bytemuck::cast_slice(&color.pixels[..]));
                            offset += std::mem::size_of_val(&color.pixels[..]);
                        },
                        egui::ImageData::Font(font) => unsafe {
                            upload_buffer
                                .write_unchecked(offset, bytemuck::cast_slice(&font.pixels[..]));
                            offset += std::mem::size_of_val(&font.pixels[..]);
                        },
                    }
                }

                let mut offset = 0usize;
                for &(id, ref delta) in &textures_delta.set {
                    let region = delta.image.size();
                    let pos = delta.pos.unwrap_or([0; 2]);
                    let size = [pos[0] + region[0], pos[1] + region[1]];

                    let format = match &delta.image {
                        egui::ImageData::Color(_) => mev::PixelFormat::Rgba8Srgb,
                        egui::ImageData::Font(_) => mev::PixelFormat::R32Float,
                    };

                    let mut image: mev::Image;

                    match textures.entry(id) {
                        Entry::Vacant(entry) => {
                            let mut new_image = queue.new_image(mev::ImageDesc {
                                dimensions: mev::Extent2::new(size[0] as u32, size[1] as u32)
                                    .into(),
                                format,
                                usage: mev::ImageUsage::SAMPLED
                                    | mev::ImageUsage::TRANSFER_DST
                                    | mev::ImageUsage::TRANSFER_SRC,
                                layers: 1,
                                levels: 1,
                                name: &format!("egui-texture-{id:?}"),
                            })?;

                            copy_encoder.init_image(
                                mev::PipelineStages::empty(),
                                mev::PipelineStages::TRANSFER,
                                &new_image,
                            );

                            if let egui::ImageData::Font(_) = &delta.image {
                                new_image = new_image.view(
                                    queue,
                                    mev::ViewDesc::new(format).swizzle(mev::Swizzle::RRRR),
                                )?;
                            }

                            image = entry
                                .insert((new_image, Sampler::from_options(delta.options)))
                                .0
                                .clone();
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().1 = Sampler::from_options(delta.options);
                            image = entry.get().0.clone();
                            let extent = image.dimensions().expect_2d();
                            if (extent.width() as usize) < size[0]
                                || (extent.height() as usize) < size[1]
                            {
                                let mut new_image = queue.new_image(mev::ImageDesc {
                                    dimensions: mev::Extent2::new(size[0] as u32, size[1] as u32)
                                        .into(),
                                    format,
                                    usage: mev::ImageUsage::SAMPLED
                                        | mev::ImageUsage::TRANSFER_DST
                                        | mev::ImageUsage::TRANSFER_SRC,
                                    layers: 1,
                                    levels: 1,
                                    name: &format!("egui-texture-{id:?}"),
                                })?;

                                copy_encoder.init_image(
                                    mev::PipelineStages::empty(),
                                    mev::PipelineStages::TRANSFER,
                                    &new_image,
                                );

                                if let egui::ImageData::Font(_) = &delta.image {
                                    new_image = new_image.view(
                                        queue,
                                        mev::ViewDesc::new(format).swizzle(mev::Swizzle::RRRR),
                                    )?;
                                }

                                copy_encoder.copy_image_region(
                                    &image,
                                    0,
                                    0,
                                    mev::Offset3::ZERO,
                                    &new_image,
                                    0,
                                    0,
                                    mev::Offset3::ZERO,
                                    image.dimensions().into_3d(),
                                    1,
                                );

                                entry.get_mut().0 = new_image.clone();
                                image = new_image;
                            }
                        }
                    }

                    copy_encoder.copy_buffer_to_image(
                        &upload_buffer,
                        offset,
                        4 * region[0],
                        0,
                        &image,
                        mev::Offset3::new(pos[0] as u32, pos[1] as u32, 0),
                        mev::Extent3::new(region[0] as u32, region[1] as u32, 1),
                        0..1,
                        0,
                    );

                    match &delta.image {
                        egui::ImageData::Color(color) => {
                            offset += std::mem::size_of_val(&color.pixels[..]);
                        }
                        egui::ImageData::Font(font) => {
                            offset += std::mem::size_of_val(&font.pixels[..]);
                        }
                    }
                }

                textures_delta.set.clear();
            }

            for &id in &textures_delta.free {
                textures.remove(&id);
            }
            textures_delta.free.clear();

            let target = frame.image();

            if !shapes.is_empty() {
                let primitives = cx.tessellate(shapes, pixels_per_point);

                if !primitives.is_empty() {
                    let mut total_vertex_size = 0;
                    let mut total_index_size = 0;

                    for primitive in &primitives {
                        match &primitive.primitive {
                            egui::epaint::Primitive::Mesh(mesh) => {
                                total_vertex_size += size_of_val(&mesh.vertices[..]);
                                total_vertex_size = (total_vertex_size + 31) & !31;
                                total_index_size += size_of_val(&mesh.indices[..]);
                                total_index_size = (total_index_size + 31) & !31;
                            }
                            egui::epaint::Primitive::Callback(_) => todo!(),
                        }
                    }

                    let vertex_buffer = match &mut self.vertex_buffer {
                        Some(buffer) if buffer.size() >= total_vertex_size => buffer,
                        slot => {
                            *slot = None;
                            slot.get_or_insert(queue.new_buffer(mev::BufferDesc {
                                size: total_vertex_size,
                                usage: mev::BufferUsage::VERTEX | mev::BufferUsage::TRANSFER_DST,
                                memory: mev::Memory::Device,
                                name: "egui-vertex-buffer",
                            })?)
                        }
                    };

                    let index_buffer = match &mut self.index_buffer {
                        Some(buffer) if buffer.size() >= total_index_size => buffer,
                        slot => {
                            *slot = None;
                            slot.get_or_insert(queue.new_buffer(mev::BufferDesc {
                                size: total_index_size,
                                usage: mev::BufferUsage::INDEX | mev::BufferUsage::TRANSFER_DST,
                                memory: mev::Memory::Device,
                                name: "egui-index-buffer",
                            })?)
                        }
                    };

                    let mut vertex_buffer_offset = 0;
                    let mut index_buffer_offset = 0;

                    for primitive in &primitives {
                        match &primitive.primitive {
                            egui::epaint::Primitive::Mesh(mesh) => {
                                copy_encoder.write_buffer_slice(
                                    vertex_buffer.slice(vertex_buffer_offset..),
                                    &mesh.vertices[..],
                                );
                                copy_encoder.write_buffer_slice(
                                    index_buffer.slice(index_buffer_offset..),
                                    &mesh.indices[..],
                                );
                                vertex_buffer_offset += size_of_val(&mesh.vertices[..]);
                                vertex_buffer_offset = (vertex_buffer_offset + 31) & !31;
                                index_buffer_offset += size_of_val(&mesh.indices[..]);
                                index_buffer_offset = (index_buffer_offset + 31) & !31;
                            }
                            egui::epaint::Primitive::Callback(_) => todo!(),
                        }
                    }

                    copy_encoder.barrier(
                        mev::PipelineStages::TRANSFER,
                        mev::PipelineStages::VERTEX_INPUT | mev::PipelineStages::FRAGMENT_SHADER,
                    );

                    let library = self.library.get_or_insert_with(|| {
                        queue
                            .new_shader_library(mev::LibraryDesc {
                                name: "egui",
                                input: mev::include_library!(
                                    "shaders/egui.wgsl" as mev::ShaderLanguage::Wgsl
                                ),
                            })
                            .unwrap()
                    });

                    let pipeline = if target.format().is_srgb() {
                        self.srgb_pipeline.get_or_insert_with(|| {
                            queue
                                .new_render_pipeline(mev::RenderPipelineDesc {
                                    name: "egui",
                                    vertex_shader: mev::Shader {
                                        library: library.clone(),
                                        entry: "vs_main".into(),
                                    },
                                    vertex_attributes: vec![
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex, pos) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex, uv) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Unorm8x4,
                                            offset: offset_of!(Vertex, color) as u32,
                                            buffer_index: 0,
                                        },
                                    ],
                                    vertex_layouts: vec![mev::VertexLayoutDesc {
                                        stride: std::mem::size_of::<Vertex>() as u32,
                                        step_mode: mev::VertexStepMode::Vertex,
                                    }],
                                    primitive_topology: mev::PrimitiveTopology::Triangle,
                                    raster: Some(mev::RasterDesc {
                                        fragment_shader: Some(mev::Shader {
                                            library: library.clone(),
                                            entry: "fs_main_srgb".into(),
                                        }),
                                        color_targets: vec![mev::ColorTargetDesc {
                                            format: target.format(),
                                            blend: Some(mev::BlendDesc::default()),
                                        }],
                                        depth_stencil: None,
                                        front_face: mev::FrontFace::default(),
                                        culling: mev::Culling::None,
                                    }),
                                    arguments: &[EguiArguments::LAYOUT],
                                    constants: EguiConstants::SIZE,
                                })
                                .unwrap()
                        })
                    } else {
                        self.linear_pipeline.get_or_insert_with(|| {
                            queue
                                .new_render_pipeline(mev::RenderPipelineDesc {
                                    name: "egui",
                                    vertex_shader: mev::Shader {
                                        library: library.clone(),
                                        entry: "vs_main".into(),
                                    },
                                    vertex_attributes: vec![
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex, pos) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex, uv) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Unorm8x4,
                                            offset: offset_of!(Vertex, color) as u32,
                                            buffer_index: 0,
                                        },
                                    ],
                                    vertex_layouts: vec![mev::VertexLayoutDesc {
                                        stride: std::mem::size_of::<Vertex>() as u32,
                                        step_mode: mev::VertexStepMode::Vertex,
                                    }],
                                    primitive_topology: mev::PrimitiveTopology::Triangle,
                                    raster: Some(mev::RasterDesc {
                                        fragment_shader: Some(mev::Shader {
                                            library: library.clone(),
                                            entry: "fs_main_linear".into(),
                                        }),
                                        color_targets: vec![mev::ColorTargetDesc {
                                            format: target.format(),
                                            blend: Some(mev::BlendDesc::default()),
                                        }],
                                        depth_stencil: None,
                                        front_face: mev::FrontFace::default(),
                                        culling: mev::Culling::None,
                                    }),
                                    arguments: &[EguiArguments::LAYOUT],
                                    constants: EguiConstants::SIZE,
                                })
                                .unwrap()
                        })
                    };

                    drop(copy_encoder);

                    let dims = target.dimensions().expect_2d();

                    let mut render = encoder.render(mev::RenderPassDesc {
                        color_attachments: &[mev::AttachmentDesc::new(&target).no_load()],
                        ..Default::default()
                    });

                    render.with_pipeline(pipeline);
                    render.with_viewport(
                        mev::Offset3::ZERO,
                        mev::Extent3::new(dims.width() as f32, dims.height() as f32, 1.0),
                    );
                    render.with_constants(&EguiConstants {
                        width: dims.width(),
                        height: dims.height(),
                        scale: pixels_per_point,
                    });

                    let mut vertex_buffer_offset = 0;
                    let mut index_buffer_offset = 0;

                    for primitive in primitives {
                        match primitive.primitive {
                            egui::epaint::Primitive::Mesh(mesh) => {
                                macro_rules! next_mesh {
                                    () => {
                                        vertex_buffer_offset += size_of_val(&mesh.vertices[..]);
                                        vertex_buffer_offset = (vertex_buffer_offset + 31) & !31;
                                        index_buffer_offset += size_of_val(&mesh.indices[..]);
                                        index_buffer_offset = (index_buffer_offset + 31) & !31;
                                    };
                                }

                                let offset = mev::Offset2::new(
                                    ((primitive.clip_rect.left() * pixels_per_point) as i32)
                                        .min(dims.width() as i32)
                                        .max(0),
                                    ((primitive.clip_rect.top() * pixels_per_point) as i32)
                                        .min(dims.height() as i32)
                                        .max(0),
                                );
                                let extent = mev::Extent2::new(
                                    ((primitive.clip_rect.width() * pixels_per_point) as u32)
                                        .min(dims.width() as u32 - offset.x() as u32),
                                    ((primitive.clip_rect.height() * pixels_per_point) as u32)
                                        .min(dims.height() as u32 - offset.y() as u32),
                                );

                                if let Some((image, sampler)) = textures.get(&mesh.texture_id) {
                                    render.with_scissor(offset, extent);

                                    render.with_arguments(
                                        0,
                                        &EguiArguments {
                                            sampler: samplers[*sampler as usize].clone(),
                                            texture: image.clone(),
                                        },
                                    );

                                    render.bind_vertex_buffers(
                                        0,
                                        &[vertex_buffer.slice(vertex_buffer_offset..)],
                                    );
                                    render.bind_index_buffer(
                                        index_buffer.slice(index_buffer_offset..),
                                    );
                                    render.draw_indexed(0, 0..mesh.indices.len() as u32, 0..1);
                                }

                                next_mesh!();
                            }
                            egui::epaint::Primitive::Callback(_) => todo!(),
                        }
                    }
                }
            }
        }

        queue.sync_frame(&mut frame, mev::PipelineStages::FRAGMENT_SHADER);
        encoder.present(frame, mev::PipelineStages::FRAGMENT_SHADER);

        let cbuf = encoder.finish()?;

        queue.submit(std::iter::once(cbuf), true)?;

        Ok(())
    }
}
