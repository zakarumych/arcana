use std::{mem::size_of_val, sync::OnceLock};

use arcana::{
    bytemuck,
    mev::{self, Arguments, DeviceRepr},
    offset_of,
    render::{Render, RenderBuilderContext, RenderContext, RenderError, RenderGraph, TargetId},
    texture::Texture,
    Blink, Component, EntityId, World,
};
use egui::epaint::{ClippedShape, Primitive, Vertex};

use hashbrown::{hash_map::Entry, HashMap};

mod event;

pub use egui::*;

#[derive(Clone, Copy)]
enum Sampler {
    NearestNearest = 0,
    NearestLinear = 1,
    LinearNearest = 2,
    LinearLinear = 3,
}

impl Sampler {
    fn from_options(options: TextureOptions) -> Self {
        match (options.minification, options.magnification) {
            (TextureFilter::Nearest, TextureFilter::Nearest) => Sampler::NearestNearest,
            (TextureFilter::Nearest, TextureFilter::Linear) => Sampler::NearestLinear,
            (TextureFilter::Linear, TextureFilter::Nearest) => Sampler::LinearNearest,
            (TextureFilter::Linear, TextureFilter::Linear) => Sampler::LinearLinear,
        }
    }
}

/// Component that can be added to the viewport to display UI using egui.
pub struct Egui {
    cx: Context,
    textures_delta: TexturesDelta,
    shapes: Vec<ClippedShape>,
    textures: HashMap<u64, (mev::Image, Sampler)>,
    raw_input: egui::RawInput,
    mouse_pos: Pos2,
    pixel_per_point: f32,
    size: Vec2,
}

impl Component for Egui {
    fn name() -> &'static str {
        "Egui"
    }
}

static GLOBAL_FONTS: OnceLock<FontDefinitions> = OnceLock::new();

fn fonts() -> FontDefinitions {
    GLOBAL_FONTS
        .get_or_init(|| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            fonts
        })
        .clone()
}

impl Egui {
    pub fn new(size: Vec2, scale_factor: f32) -> Self {
        let fonts = fonts();
        let cx: Context = Context::default();
        cx.set_fonts(fonts);

        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(Rect::from_min_size(Default::default(), size / scale_factor));
        raw_input.pixels_per_point = Some(scale_factor);

        Egui {
            cx,
            textures_delta: TexturesDelta::default(),
            shapes: Vec::new(),
            textures: HashMap::new(),
            mouse_pos: Pos2::ZERO,
            raw_input,
            pixel_per_point: scale_factor,
            size,
        }
    }

    pub fn run<R>(&mut self, run_ui: impl FnOnce(&Context) -> R) -> Option<R> {
        self.cx.begin_frame(self.raw_input.take());
        let ret = run_ui(&self.cx);
        let output = self.cx.end_frame();

        // TODO: Handle platform output
        let _ = output.platform_output;

        self.textures_delta.append(output.textures_delta);
        self.shapes = output.shapes;
        Some(ret)
    }
}

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

pub struct EguiRender {
    id: Option<EntityId>,
    target: TargetId<mev::Image>,
    samplers: Option<[mev::Sampler; 4]>,
    library: Option<mev::Library>,
    linear_pipeline: Option<mev::RenderPipeline>,
    srgb_pipeline: Option<mev::RenderPipeline>,

    vertex_buffer: Option<mev::Buffer>,
    index_buffer: Option<mev::Buffer>,
    load_op: mev::LoadOp<mev::ClearColor>,
}

impl EguiRender {
    fn new(
        id: Option<EntityId>,
        target: TargetId<mev::Image>,
        load_op: mev::LoadOp<mev::ClearColor>,
    ) -> Self {
        EguiRender {
            id,
            target,
            samplers: None,
            library: None,
            linear_pipeline: None,
            srgb_pipeline: None,
            vertex_buffer: None,
            index_buffer: None,
            load_op,
        }
    }

    pub fn build_overlay(
        id: Option<EntityId>,
        target: TargetId<mev::Image>,
        graph: &mut RenderGraph,
    ) -> TargetId<mev::Image> {
        let mut builder = RenderBuilderContext::new("egui", graph);
        let new_target = builder.write_target(target, mev::PipelineStages::COLOR_OUTPUT);
        builder.build(EguiRender::new(id, new_target, mev::LoadOp::Load));
        new_target
    }

    pub fn build(
        id: Option<EntityId>,
        color: mev::ClearColor,
        graph: &mut RenderGraph,
    ) -> TargetId<mev::Image> {
        let mut builder = RenderBuilderContext::new("egui", graph);
        let new_target = builder.create_target("egui-surface", mev::PipelineStages::COLOR_OUTPUT);
        builder.build(EguiRender::new(id, new_target, mev::LoadOp::Clear(color)));
        new_target
    }
}

impl Render for EguiRender {
    fn render(&mut self, world: &World, mut cx: RenderContext<'_, '_>) -> Result<(), RenderError> {
        let mut egui_res;
        let mut egui_view;
        let egui = match self.id {
            None => {
                egui_res = match world.get_resource_mut::<Egui>() {
                    Some(egui) => egui,
                    None => return Ok(()),
                };
                &mut *egui_res
            }
            Some(id) => {
                egui_view = match world.try_view_one::<&mut Egui>(id) {
                    Ok(egui) => egui,
                    Err(_) => return Ok(()),
                };
                match egui_view.get_mut() {
                    Some(egui) => egui,
                    None => return Ok(()),
                }
            }
        };

        let samplers = match &mut self.samplers {
            Some(samplers) => &*samplers,
            none => {
                let sampler_nn = cx.device().new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Nearest,
                    mag_filter: mev::Filter::Nearest,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                let sampler_nl = cx.device().new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Nearest,
                    mag_filter: mev::Filter::Linear,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                let sampler_ln = cx.device().new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Linear,
                    mag_filter: mev::Filter::Nearest,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                let sampler_ll = cx.device().new_sampler(mev::SamplerDesc {
                    min_filter: mev::Filter::Linear,
                    mag_filter: mev::Filter::Linear,
                    address_mode: [mev::AddressMode::ClampToEdge; 3],
                    ..mev::SamplerDesc::new()
                })?;
                none.get_or_insert([sampler_nn, sampler_nl, sampler_ln, sampler_ll])
            }
        };

        let mut encoder = cx.new_command_encoder()?;

        let target = cx.write_target(self.target, &mut encoder).clone();

        {
            let mut copy_encoder = encoder.copy();

            copy_encoder.barrier(
                mev::PipelineStages::VERTEX_INPUT | mev::PipelineStages::FRAGMENT_SHADER,
                mev::PipelineStages::TRANSFER,
            );

            if !egui.textures_delta.set.is_empty() {
                let delta_size = egui.textures_delta.set.iter().fold(0, |acc, (_, delta)| {
                    acc + match &delta.image {
                        ImageData::Color(color) => std::mem::size_of_val(&color.pixels[..]),
                        ImageData::Font(font) => std::mem::size_of_val(&font.pixels[..]),
                    }
                });

                let mut upload_buffer = cx.device().new_buffer(mev::BufferDesc {
                    size: delta_size,
                    usage: mev::BufferUsage::TRANSFER_SRC,
                    memory: mev::Memory::Upload,
                    name: "texture-delta-upload",
                })?;

                let mut offset = 0usize;
                for (_, delta) in egui.textures_delta.set.iter() {
                    match &delta.image {
                        ImageData::Color(color) => unsafe {
                            upload_buffer
                                .write_unchecked(offset, bytemuck::cast_slice(&color.pixels[..]));
                            offset += std::mem::size_of_val(&color.pixels[..]);
                        },
                        ImageData::Font(font) => unsafe {
                            upload_buffer
                                .write_unchecked(offset, bytemuck::cast_slice(&font.pixels[..]));
                            offset += std::mem::size_of_val(&font.pixels[..]);
                        },
                    }
                }

                let mut offset = 0usize;
                for (id, delta) in egui.textures_delta.set.iter() {
                    let region = delta.image.size();
                    let pos = delta.pos.unwrap_or([0; 2]);
                    let size = [pos[0] + region[0], pos[1] + region[1]];

                    let format = match &delta.image {
                        ImageData::Color(_) => mev::PixelFormat::Rgba8Srgb,
                        ImageData::Font(_) => mev::PixelFormat::R32Float,
                    };

                    let mut image: mev::Image;

                    let id = match id {
                        egui::TextureId::Managed(id) => id,
                        _ => continue,
                    };

                    match egui.textures.entry(*id) {
                        Entry::Vacant(entry) => {
                            let mut new_image = cx.device().new_image(mev::ImageDesc {
                                dimensions: mev::Extent2::new(size[0] as u32, size[1] as u32)
                                    .into(),
                                format,
                                usage: mev::ImageUsage::SAMPLED | mev::ImageUsage::TRANSFER_DST,
                                layers: 1,
                                levels: 1,
                                name: &format!("egui-texture-{id:?}"),
                            })?;

                            copy_encoder.init_image(
                                mev::PipelineStages::empty(),
                                mev::PipelineStages::TRANSFER,
                                &new_image,
                            );

                            if let ImageData::Font(_) = &delta.image {
                                new_image = new_image.view(
                                    cx.device(),
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
                            let extent = image.dimensions().to_2d();
                            if (extent.width() as usize) < size[0]
                                || (extent.height() as usize) < size[1]
                            {
                                let mut new_image = cx.device().new_image(mev::ImageDesc {
                                    dimensions: mev::Extent2::new(size[0] as u32, size[1] as u32)
                                        .into(),
                                    format,
                                    usage: mev::ImageUsage::SAMPLED | mev::ImageUsage::TRANSFER_DST,
                                    layers: 1,
                                    levels: 1,
                                    name: &format!("egui-texture-{id:?}"),
                                })?;

                                if let ImageData::Font(_) = &delta.image {
                                    new_image = new_image.view(
                                        cx.device(),
                                        mev::ViewDesc::new(format).swizzle(mev::Swizzle::RRRR),
                                    )?;
                                }

                                copy_encoder.copy_image_region(
                                    &image,
                                    mev::Offset3::ZERO,
                                    0,
                                    &new_image,
                                    mev::Offset3::ZERO,
                                    0,
                                    image.dimensions().to_3d(),
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
                        ImageData::Color(color) => {
                            offset += std::mem::size_of_val(&color.pixels[..]);
                        }
                        ImageData::Font(font) => {
                            offset += std::mem::size_of_val(&font.pixels[..]);
                        }
                    }
                }

                egui.textures_delta.set.clear();
            }

            if !egui.shapes.is_empty() {
                let primitives = egui.cx.tessellate(std::mem::take(&mut egui.shapes));

                if !primitives.is_empty() {
                    let mut total_vertex_size = 0;
                    let mut total_index_size = 0;

                    for primitive in &primitives {
                        match &primitive.primitive {
                            Primitive::Mesh(mesh) => {
                                total_vertex_size += size_of_val(&mesh.vertices[..]);
                                total_vertex_size = (total_vertex_size + 31) & !31;
                                total_index_size += size_of_val(&mesh.indices[..]);
                                total_index_size = (total_index_size + 31) & !31;
                            }
                            Primitive::Callback(_) => todo!(),
                        }
                    }

                    let vertex_buffer = match &mut self.vertex_buffer {
                        Some(buffer) if buffer.size() >= total_vertex_size => buffer,
                        slot => {
                            *slot = None;
                            slot.get_or_insert(cx.device().new_buffer(mev::BufferDesc {
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
                            slot.get_or_insert(cx.device().new_buffer(mev::BufferDesc {
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
                            Primitive::Mesh(mesh) => {
                                copy_encoder.write_buffer_slice(
                                    &vertex_buffer,
                                    vertex_buffer_offset,
                                    &mesh.vertices[..],
                                );
                                copy_encoder.write_buffer_slice(
                                    &index_buffer,
                                    index_buffer_offset,
                                    &mesh.indices[..],
                                );
                                vertex_buffer_offset += size_of_val(&mesh.vertices[..]);
                                vertex_buffer_offset = (vertex_buffer_offset + 31) & !31;
                                index_buffer_offset += size_of_val(&mesh.indices[..]);
                                index_buffer_offset = (index_buffer_offset + 31) & !31;
                            }
                            Primitive::Callback(_) => todo!(),
                        }
                    }

                    copy_encoder.barrier(
                        mev::PipelineStages::TRANSFER,
                        mev::PipelineStages::VERTEX_INPUT | mev::PipelineStages::FRAGMENT_SHADER,
                    );

                    let library = self.library.get_or_insert_with(|| {
                        cx.device()
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
                            cx.device()
                                .new_render_pipeline(mev::RenderPipelineDesc {
                                    name: "egui",
                                    vertex_shader: mev::Shader {
                                        library: library.clone(),
                                        entry: "vs_main".into(),
                                    },
                                    vertex_attributes: vec![
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex.pos) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex.uv) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Unorm8x4,
                                            offset: offset_of!(Vertex.color) as u32,
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
                            cx.device()
                                .new_render_pipeline(mev::RenderPipelineDesc {
                                    name: "egui",
                                    vertex_shader: mev::Shader {
                                        library: library.clone(),
                                        entry: "vs_main".into(),
                                    },
                                    vertex_attributes: vec![
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex.pos) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Float32x2,
                                            offset: offset_of!(Vertex.uv) as u32,
                                            buffer_index: 0,
                                        },
                                        mev::VertexAttributeDesc {
                                            format: mev::VertexFormat::Unorm8x4,
                                            offset: offset_of!(Vertex.color) as u32,
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

                    let dims = target.dimensions().to_2d();

                    let mut render = encoder.render(mev::RenderPassDesc {
                        color_attachments: &[
                            mev::AttachmentDesc::new(&target).load_op(self.load_op)
                        ],
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
                        scale: egui.cx.pixels_per_point(),
                    });

                    let mut vertex_buffer_offset = 0;
                    let mut index_buffer_offset = 0;

                    for primitive in primitives {
                        match primitive.primitive {
                            Primitive::Mesh(mesh) => {
                                macro_rules! next_mesh {
                                    () => {
                                        vertex_buffer_offset += size_of_val(&mesh.vertices[..]);
                                        vertex_buffer_offset = (vertex_buffer_offset + 31) & !31;
                                        index_buffer_offset += size_of_val(&mesh.indices[..]);
                                        index_buffer_offset = (index_buffer_offset + 31) & !31;
                                    };
                                }

                                let offset = mev::Offset2::new(
                                    ((primitive.clip_rect.left() * egui.cx.pixels_per_point())
                                        as i32)
                                        .min(dims.width() as i32)
                                        .max(0),
                                    ((primitive.clip_rect.top() * egui.cx.pixels_per_point())
                                        as i32)
                                        .min(dims.height() as i32)
                                        .max(0),
                                );
                                let extent = mev::Extent2::new(
                                    ((primitive.clip_rect.width() * egui.cx.pixels_per_point())
                                        as u32)
                                        .min(dims.width() as u32 - offset.x() as u32),
                                    ((primitive.clip_rect.height() * egui.cx.pixels_per_point())
                                        as u32)
                                        .min(dims.height() as u32 - offset.y() as u32),
                                );
                                render.with_scissor(offset, extent);

                                let (image, sampler) = match mesh.texture_id {
                                    TextureId::Managed(id) => egui.textures[&id].clone(),
                                    TextureId::User(id) => {
                                        let Some(id) = EntityId::from_bits(id) else {
                                            next_mesh!();
                                            continue;
                                        };

                                        let Ok(mut texture) = world.try_view_one::<&Texture>(id)
                                        else {
                                            next_mesh!();
                                            continue;
                                        };
                                        let Some(texture) = texture.get_mut() else {
                                            next_mesh!();
                                            continue;
                                        };

                                        (texture.image.clone(), Sampler::LinearLinear)
                                    }
                                };

                                render.with_arguments(
                                    0,
                                    &EguiArguments {
                                        sampler: samplers[sampler as usize].clone(),
                                        texture: image.clone(),
                                    },
                                );

                                render.bind_vertex_buffers(
                                    0,
                                    &[(&vertex_buffer, vertex_buffer_offset)],
                                );
                                render.bind_index_buffer(&index_buffer, index_buffer_offset);
                                render.draw_indexed(0, 0..mesh.indices.len() as u32, 0..1);

                                next_mesh!();
                            }
                            Primitive::Callback(_) => todo!(),
                        }
                    }
                }
            }
        }

        for id in egui.textures_delta.free.iter() {
            match id {
                TextureId::Managed(id) => {
                    egui.textures.remove(id);
                }
                TextureId::User(_) => {}
            }
        }
        egui.textures_delta.free.clear();

        cx.commit(encoder.finish()?);

        Ok(())
    }
}

pub struct EguiFilter;

impl arcana::events::EventFilter for EguiFilter {
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: &arcana::events::Event) -> bool {
        let world = world.local();

        if let arcana::events::Event::ViewportEvent { event } = event {
            if let Some(mut egui) = world.get_resource_mut::<Egui>() {
                if egui.handle_event(event) {
                    // Egui consumed this event.
                    return true;
                }
            }
        }

        false
    }
}
