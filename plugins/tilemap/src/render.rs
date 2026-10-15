use arcana::{
    ecs::{query::Entities, world::World},
    mev,
    render::CurrentRenderer,
    scene::dim2::{Transform, camera_matrices},
    work_graph::{Exec, Image2D, Image2DReq, Job, JobDesc, Planner, Req, TargetUpdateDesc},
};
use mev::{Arguments, DeviceRepr};

use crate::{Map, MapData, TilesetData};

#[derive(mev::Arguments)]
struct TileArguments {
    #[mev(fragment)]
    sampler: mev::Sampler,
    #[mev(sampled, fragment)]
    atlas: mev::Image2D,
}

#[derive(mev::DeviceRepr)]
struct TileConstants {
    local_to_clip: mev::mat4x4,
}

fn create_pipeline(device: &mev::Device, format: mev::PixelFormat) -> mev::RenderPipeline {
    let library = device
        .new_shader_library(mev::LibraryDesc {
            name: "TileMap",
            input: mev::LibraryInput::wgsl(include_str!("tilemap.wgsl")),
        })
        .expect("TileMap shader must compile");
    device
        .new_render_pipeline(mev::RenderPipelineDesc {
            name: "TileMap",
            vertex_shader: library.entry("vertex_main"),
            vertex_layouts: vec![mev::VertexLayoutDesc {
                stride: 16,
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
                    format: mev::VertexFormat::Float32x2,
                    offset: 8,
                },
            ],
            primitive_topology: mev::PrimitiveTopology::Triangle,
            raster: Some(mev::RasterDesc {
                fragment_shader: Some(library.entry("fragment_main")),
                color_targets: vec![mev::ColorTargetDesc::new(format).alpha_blend()],
                depth_stencil: None,
                front_face: mev::FrontFace::default(),
                culling: mev::Culling::None,
            }),
            constants: TileConstants::SIZE,
            arguments: &[TileArguments::LAYOUT],
        })
        .expect("TileMap pipeline must compile")
}

/// Renders world `Map` components through the current renderer entity's 2D camera.
/// Updates an initialized color target, preserving empty cells and pixels outside the map.
/// Each map's top-left corner is its local origin, with logical Y pointing down.
/// Maps use their optional 2D transform and draw in ascending entity ID order.
pub struct TileMap {
    pipeline: Option<(mev::PixelFormat, mev::RenderPipeline)>,
    sampler: Option<mev::Sampler>,
}

#[arcana::job]
impl Job for TileMap {
    fn name() -> arcana::Name {
        arcana::name!(TileMap)
    }

    fn desc() -> JobDesc {
        JobDesc {
            updates: vec![TargetUpdateDesc::new::<Image2D>(arcana::name!(Target))],
            ..JobDesc::default()
        }
    }

    fn new() -> Self {
        Self {
            pipeline: None,
            sampler: None,
        }
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        planner.update_with_req::<Image2D>(Image2DReq {
            extent: Req::Suggest(mev::Extent2::new(1, 1)),
            format: Req::Suggest(mev::PixelFormat::Rgb8Unorm),
            usage: mev::ImageUsage::TARGET,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, world: &mut World) {
        let Some(target) = exec.update::<Image2D>() else {
            return;
        };
        let extent = target.extent();
        if extent.width() == 0 || extent.height() == 0 {
            return;
        }
        let format = target.format();
        if !matches!(
            format,
            mev::PixelFormat::Rgb8Unorm
                | mev::PixelFormat::Rgb8Srgb
                | mev::PixelFormat::Rgba8Unorm
                | mev::PixelFormat::Rgba8Srgb
        ) {
            arcana::tracing::error!("TileMap requires an RGB8 or RGBA8 color target");
            return;
        }
        let Some(renderer) = world.get_resource::<CurrentRenderer>() else {
            return;
        };
        let Some(camera) = camera_matrices(
            world,
            renderer.entity,
            f64::from(extent.width()) / f64::from(extent.height()),
        ) else {
            return;
        };
        let maps = world.view::<(Entities, &Map, Option<&Transform>)>();
        let mut maps: Vec<_> = maps.iter().collect();
        maps.sort_unstable_by_key(|(entity, _, _)| entity.id());
        let mut draws = Vec::new();
        for (_, map, transform) in maps {
            let local_to_clip = (camera.view_projection()
                * transform.copied().unwrap_or_default().matrix())
            .arrays()
            .map(|column| column.map(|v| v as f32));
            if local_to_clip.iter().flatten().any(|v| !v.is_finite()) {
                continue;
            }
            let atlas = &map.tileset().texture().image;
            let Some(vertices) = tile_vertices(map.data(), map.tileset().data(), atlas.extent())
            else {
                arcana::tracing::error!("TileMap geometry exceeds rendering limits");
                continue;
            };
            if vertices.is_empty() {
                continue;
            }
            let buffer = exec.device().new_buffer_init(mev::BufferInitDesc {
                name: "TileMap vertices",
                data: vertices.as_slice(),
                usage: mev::BufferUsage::VERTEX | mev::BufferUsage::TRANSIENT,
            });
            draws.push((buffer, atlas.clone(), local_to_clip, vertices.len() as u32));
        }
        if draws.is_empty() {
            return;
        }
        if self
            .pipeline
            .as_ref()
            .is_none_or(|(cached, _)| *cached != format)
        {
            self.pipeline = Some((format, create_pipeline(exec.device(), format)));
        }
        let sampler = self.sampler.get_or_insert_with(|| {
            exec.device().new_sampler(mev::SamplerDesc {
                min_filter: mev::Filter::Nearest,
                mag_filter: mev::Filter::Nearest,
                address_mode: [mev::AddressMode::ClampToEdge; 3],
                ..mev::SamplerDesc::default()
            })
        });
        let encoder = exec.new_encoder();
        encoder.barrier(
            mev::PipelineStages::all(),
            mev::PipelineStages::VERTEX_INPUT
                | mev::PipelineStages::FRAGMENT_SHADER
                | mev::PipelineStages::COLOR_OUTPUT,
        );
        let mut pass = encoder.render(mev::RenderPassDesc {
            name: "TileMap",
            color_attachments: &[mev::AttachmentDesc::new(&target.0)],
            depth_stencil_attachment: None,
        });
        pass.with_pipeline(&self.pipeline.as_ref().unwrap().1);
        pass.with_viewport(
            mev::Offset3::new(0.0, 0.0, 0.0),
            mev::Extent3::new(extent.width() as f32, extent.height() as f32, 1.0),
        );
        pass.with_scissor(mev::Offset2::new(0, 0), extent);
        for (buffer, atlas, local_to_clip, vertex_count) in &draws {
            pass.with_constants(&TileConstants {
                local_to_clip: (*local_to_clip).into(),
            });
            pass.with_arguments(
                0,
                &TileArguments {
                    sampler: sampler.clone(),
                    atlas: atlas.clone(),
                },
            );
            pass.bind_vertex_buffers(0, &[buffer]);
            pass.draw(0..*vertex_count, 0..1);
        }
    }
}

fn tile_vertices(
    map: &MapData,
    tileset: &TilesetData,
    extent: mev::Extent2,
) -> Option<Vec<[f32; 4]>> {
    let occupied = map
        .cells
        .iter()
        .flatten()
        .filter(|&&cell| cell != 0)
        .count();
    let count = occupied.checked_mul(6)?;
    u32::try_from(count).ok()?;
    let mut vertices = Vec::with_capacity(count);
    let [width, height] = tileset.tile_size;
    for (row, cells) in map.cells.iter().enumerate() {
        let y = (map.cells.len() - 1 - row) as f32 * height;
        for (column, &cell) in cells.iter().enumerate() {
            if cell == 0 {
                continue;
            }
            let x = column as f32 * width;
            let [u, v, w, h] = tileset.tiles.get((cell - 1) as usize)?.region;
            let u0 = u as f32 / extent.width() as f32;
            let v0 = v as f32 / extent.height() as f32;
            let u1 = (u + w) as f32 / extent.width() as f32;
            let v1 = (v + h) as f32 / extent.height() as f32;
            let corners = [
                [x, y, u0, v0],
                [x + width, y, u1, v0],
                [x + width, y + height, u1, v1],
                [x, y + height, u0, v1],
            ];
            if corners.iter().flatten().any(|v| !v.is_finite()) {
                return None;
            }
            vertices.extend([
                corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
            ]);
        }
    }
    Some(vertices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TileData;
    use arcana::assets::AssetId;

    #[test]
    fn geometry_uses_bottom_to_top_rows_one_based_cells_and_top_left_uvs() {
        let id = AssetId::new(std::num::NonZeroU64::new(1).unwrap());
        let map = MapData {
            tileset: id,
            cells: vec![vec![0, 2], vec![1, 0]],
        };
        let tileset = TilesetData {
            image: id,
            tile_size: [3.0, 5.0],
            tiles: vec![
                TileData {
                    name: "first".into(),
                    region: [0, 0, 2, 4],
                },
                TileData {
                    name: "second".into(),
                    region: [4, 4, 4, 4],
                },
            ],
        };
        let vertices = tile_vertices(&map, &tileset, mev::Extent2::new(8, 16)).unwrap();
        assert_eq!(vertices.len(), 12);
        assert_eq!(
            &vertices[..6],
            &[
                [3.0, 5.0, 0.5, 0.25],
                [6.0, 5.0, 1.0, 0.25],
                [6.0, 10.0, 1.0, 0.5],
                [3.0, 5.0, 0.5, 0.25],
                [6.0, 10.0, 1.0, 0.5],
                [3.0, 10.0, 0.5, 0.5],
            ]
        );
        assert_eq!(vertices[6], [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(vertices[8], [3.0, 5.0, 0.25, 0.25]);
    }
}
