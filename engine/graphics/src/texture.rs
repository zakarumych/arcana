use std::{future::Future, io::Cursor};

use arcana_assets::{Asset, AssetBuilder, AssetError, AssetVersion, Assets};
use arcana_id::has_stid;
use edict::component::Component;
use jkl::{
    encode::FixedCode,
    image::{Extent, format::Format, tiles::TileSize},
    jackal::image::{Compression, JackalImageReader},
};
use jkl_mev::image::MevImage;

/// A sampled 2D texture loaded from a single-level JKLI image.
///
/// Supports RGB8 and BC1 with ANS or LZ77+ANS, and R8/RG8/RGBA8 with ANS.
/// BC1 requires `mev::Features::TEXTURE_COMPRESSION_BC` on the shared device.
#[derive(Clone)]
pub struct Texture {
    pub image: mev::Image2D,
}

has_stid!(Texture @ arcana_id);

impl Component for Texture {
    fn name() -> &'static str {
        "Texture"
    }
}

pub struct LoadedTexture(Box<[u8]>);

#[derive(Clone, Copy, Debug, thiserror::Error)]
enum TextureError {
    #[error("Invalid JKLI data")]
    InvalidData,
    #[error("Texture requires exactly one image level")]
    ImageLevelsNotSupported,
    #[error("Texture requires a nonempty plain 2D image")]
    ImageTypeNotSupported,
    #[error("JKLI format-compression combination not supported")]
    FormatNotSupported,
    #[error("JKLI dimensions exceed GPU upload limits")]
    DimensionsTooLarge,
}

impl AssetVersion for Texture {
    #[inline]
    fn version() -> u64 {
        0
    }
}

fn load_texture(data: Box<[u8]>) -> Result<LoadedTexture, AssetError> {
    // Validate fields used for tile-count arithmetic before opening the reader.
    const EXTENT_START: usize = 4 + Compression::SIZE + Format::SIZE;
    const LEVELS_START: usize = EXTENT_START + Extent::SIZE;
    const TILE_START: usize = LEVELS_START + u16::SIZE;
    const HEADER_SIZE: usize = TILE_START + TileSize::SIZE;
    let header = data
        .get(..HEADER_SIZE)
        .ok_or_else(|| AssetError::new(TextureError::InvalidData))?;
    let extent = Extent::fix_decode(header[EXTENT_START..LEVELS_START].try_into().unwrap())
        .map_err(AssetError::new)?;
    let Extent::D2 { width, height } = extent else {
        return Err(AssetError::new(TextureError::ImageTypeNotSupported));
    };
    if width == 0 || height == 0 {
        return Err(AssetError::new(TextureError::ImageTypeNotSupported));
    }
    let levels = u16::from_le_bytes(header[LEVELS_START..TILE_START].try_into().unwrap());
    if levels != 1 {
        return Err(AssetError::new(TextureError::ImageLevelsNotSupported));
    }
    let tile_size =
        TileSize::fix_decode(header[TILE_START..HEADER_SIZE].try_into().unwrap()).unwrap();
    if tile_size.width == 0 || tile_size.height == 0 {
        return Err(AssetError::new(TextureError::InvalidData));
    }
    let tile_count = width
        .div_ceil(usize::from(tile_size.width))
        .checked_mul(height.div_ceil(usize::from(tile_size.height)))
        .filter(|&count| count <= (data.len() - HEADER_SIZE) / u64::SIZE)
        .ok_or_else(|| AssetError::new(TextureError::InvalidData))?;
    let context_start = HEADER_SIZE + tile_count * u64::SIZE;
    let mut reader = JackalImageReader::open(Cursor::new(&data[..])).map_err(AssetError::new)?;
    let block_width = match (reader.format(), reader.compression()) {
        (Format::RGB8, Compression::Ans | Compression::Lz77Ans)
        | (Format::R8 | Format::RG8 | Format::RGBA8, Compression::Ans) => 1,
        (Format::BC1, Compression::Ans | Compression::Lz77Ans) => 4,
        _ => return Err(AssetError::new(TextureError::FormatNotSupported)),
    };
    if matches!(reader.format(), Format::R8 | Format::RG8)
        && tile_size.width % reader.format().tile_width_granularity() != 0
    {
        return Err(AssetError::new(TextureError::InvalidData));
    }
    // Uploader addresses rows and aligned output sizes with 32-bit arithmetic.
    if width > (u32::MAX as usize - 255) / 16
        || height > u32::MAX as usize / block_width
        || width
            .checked_mul(block_width)
            .is_none_or(|v| v > u32::MAX as usize)
    {
        return Err(AssetError::new(TextureError::DimensionsTooLarge));
    }
    let offsets = reader.tile_offsets();
    let mut previous = context_start as u64;
    for &offset in offsets {
        if offset < previous || offset > data.len() as u64 {
            return Err(AssetError::new(TextureError::InvalidData));
        }
        previous = offset;
    }
    let payload_len = reader.tiles_payload_len();
    if payload_len > u32::MAX as u64 || payload_len == 0 {
        return Err(AssetError::new(TextureError::InvalidData));
    }
    for index in 0..reader.tiles() {
        let len = reader.tile_payload_len(index);
        if len == 0 || len % 4 != 0 {
            return Err(AssetError::new(TextureError::InvalidData));
        }
    }
    // Compression tables must contain symbols and fit shader table addressing.
    use jkl::algos::{ans, lz77, vle::Vle};
    fn validate<T>(context: ans::Context<T>) -> std::io::Result<()> {
        let table = context.table();
        // ANS normalization rounds each frequency down in a 32-bit interval.
        let total = table
            .iter()
            .try_fold(0u32, |sum, entry| sum.checked_add(entry.freq.get()));
        if table.is_empty()
            || table.len() > u32::MAX as usize
            || total.is_none_or(|total| {
                total < u32::MAX.saturating_sub(table.len().saturating_sub(1) as u32)
            })
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid JKLI frequency table",
            ));
        }
        Ok(())
    }
    let format = reader.format();
    let compression = reader.compression();
    jkl::bits::read_bits_scope(
        reader.context_reader().map_err(AssetError::new)?,
        |bits| match (format, compression) {
            (Format::R8, Compression::Ans) => validate(ans::Context::<Vle<u8>>::read(bits)?),
            (Format::RG8, Compression::Ans) => validate(ans::Context::<Vle<u16>>::read(bits)?),
            (Format::RGB8 | Format::RGBA8, Compression::Ans) => {
                validate(ans::Context::<Vle<u32>>::read(bits)?)
            }
            (Format::RGB8, Compression::Lz77Ans) => {
                validate(ans::Context::<lz77::Token<Vle<u32>>>::read(bits)?)
            }
            (Format::BC1, Compression::Ans) => {
                validate(ans::Context::<Vle<u16>>::read(bits)?)?;
                validate(ans::Context::<u8>::read(bits)?)
            }
            (Format::BC1, Compression::Lz77Ans) => {
                validate(ans::Context::<lz77::Token<Vle<u16>>>::read(bits)?)?;
                validate(ans::Context::<lz77::Token<u8>>::read(bits)?)
            }
            _ => unreachable!(),
        },
    )
    .map_err(AssetError::new)?;
    Ok(LoadedTexture(data))
}

impl Asset for Texture {
    type Loaded = LoadedTexture;

    fn load(
        data: Box<[u8]>,
        _assets: &Assets,
    ) -> impl Future<Output = Result<Self::Loaded, AssetError>> + Send {
        async move { load_texture(data) }
    }

    fn build(loaded: LoadedTexture, builder: &mut AssetBuilder) -> Result<Self, AssetError> {
        let mut reader =
            JackalImageReader::open(Cursor::new(&loaded.0[..])).map_err(AssetError::new)?;
        let gpu = &mut *builder.gpu;
        let pixels = gpu
            .uploader
            .upload_from_reader(&mut reader, &gpu.device, &mut builder.encoder)
            .map_err(AssetError::new)?;
        builder.encoder.barrier(
            mev::PipelineStages::COMPUTE_SHADER,
            mev::PipelineStages::TRANSFER,
        );
        let image = pixels.make_image(&gpu.device, &mut builder.encoder);
        builder.encoder.barrier(
            mev::PipelineStages::TRANSFER,
            mev::PipelineStages::VERTEX_SHADER
                | mev::PipelineStages::FRAGMENT_SHADER
                | mev::PipelineStages::COMPUTE_SHADER,
        );
        Ok(Texture {
            image: mev::Image2D::try_from(image).map_err(AssetError::new)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jkl::{
        image::{Image, block::bc1, tiles::TileSize},
        jackal::image::{Options, Pixel, write_image},
        math::{R8U, Rg8U, Rgb8U, Rgba8U},
    };
    use mev::Arguments;

    fn encode<T: Pixel>(image: jkl::image::ImageRef<'_, T>, compression: Compression) -> Box<[u8]> {
        let mut output = Cursor::new(Vec::new());
        write_image(
            image,
            Options::new()
                .with_compression(compression)
                .with_tile_size(TileSize {
                    width: 2u16.max(T::FORMAT.tile_width_granularity()),
                    height: 2,
                }),
            &mut output,
        )
        .unwrap();
        output.into_inner().into_boxed_slice()
    }

    fn rgb(compression: Compression) -> Box<[u8]> {
        encode(
            Image::new_2d(
                3,
                2,
                vec![
                    Rgb8U::RED,
                    Rgb8U::GREEN,
                    Rgb8U::BLUE,
                    Rgb8U::WHITE,
                    Rgb8U::BLACK,
                    Rgb8U::RED,
                ]
                .into_boxed_slice(),
            )
            .as_ref(),
            compression,
        )
    }

    #[test]
    fn supported_formats_load() {
        assert!(
            load_texture(encode(
                Image::new_2d(3, 2, vec![R8U(123); 6].into_boxed_slice()).as_ref(),
                Compression::Ans
            ))
            .is_ok()
        );
        assert!(
            load_texture(encode(
                Image::new_2d(3, 2, vec![Rg8U([12, 34]); 6].into_boxed_slice()).as_ref(),
                Compression::Ans
            ))
            .is_ok()
        );
        for compression in [Compression::Ans, Compression::Lz77Ans] {
            assert!(load_texture(rgb(compression)).is_ok());
            assert!(
                load_texture(encode(
                    Image::new_2d(2, 2, vec![bc1::Block::BLACK; 4].into_boxed_slice()).as_ref(),
                    compression
                ))
                .is_ok()
            );
        }
        assert!(
            load_texture(encode(
                Image::new_2d(3, 2, vec![Rgba8U([12, 34, 56, 78]); 6].into_boxed_slice()).as_ref(),
                Compression::Ans
            ))
            .is_ok()
        );
    }

    #[test]
    fn unsupported_formats_and_dimensions_fail() {
        for compression in [Compression::None, Compression::Lz77Ans] {
            assert!(
                load_texture(encode(
                    Image::new_2d(3, 2, vec![R8U(123); 6].into_boxed_slice()).as_ref(),
                    compression
                ))
                .is_err()
            );
            assert!(
                load_texture(encode(
                    Image::new_2d(3, 2, vec![Rg8U([12, 34]); 6].into_boxed_slice()).as_ref(),
                    compression
                ))
                .is_err()
            );
        }
        for compression in [Compression::None, Compression::Lz77, Compression::RleAns] {
            assert!(
                load_texture(rgb(compression))
                    .err()
                    .unwrap()
                    .is::<TextureError>()
            );
        }
        assert!(
            load_texture(encode(
                Image::new_2d(3, 2, vec![Rgba8U([12, 34, 56, 78]); 6].into_boxed_slice()).as_ref(),
                Compression::Lz77Ans
            ))
            .is_err()
        );
        for dimensions in [
            jkl::image::Dimensions::D1,
            jkl::image::Dimensions::D2Array,
            jkl::image::Dimensions::D3,
        ] {
            let layers = if dimensions == jkl::image::Dimensions::D1 {
                1
            } else {
                2
            };
            let image = Image::new(
                dimensions,
                [2, 1, layers],
                vec![Rgb8U::RED; 2 * layers].into_boxed_slice(),
            );
            assert!(load_texture(encode(image.as_ref(), Compression::Ans)).is_err());
        }
    }

    #[test]
    fn packed_formats_reject_unaligned_tiles_and_empty_tables() {
        let fixtures = [
            encode(
                Image::new_2d(1, 1, vec![R8U(123)].into_boxed_slice()).as_ref(),
                Compression::Ans,
            ),
            encode(
                Image::new_2d(1, 1, vec![Rg8U([12, 34])].into_boxed_slice()).as_ref(),
                Compression::Ans,
            ),
        ];
        for valid in fixtures {
            let mut unaligned = valid.clone();
            unaligned[34..36].copy_from_slice(&1u16.to_le_bytes());
            assert!(load_texture(unaligned).is_err());
            let reader = JackalImageReader::open(Cursor::new(&valid[..])).unwrap();
            let context_start = 38 + reader.tiles() * u64::SIZE;
            let context_end = reader.tile_offsets()[0] as usize;
            let mut empty = valid;
            empty[context_start..context_end].fill(0);
            assert!(load_texture(empty).is_err());
        }
    }

    #[test]
    fn malformed_metadata_fails_without_panicking() {
        let valid = rgb(Compression::Ans);
        for length in [0, 4, 37, valid.len() - 1] {
            assert!(load_texture(valid[..length].to_vec().into_boxed_slice()).is_err());
        }
        for (range, bytes) in [
            (0..4, vec![0; 4]),
            (8..16, vec![0; 8]),
            (32..34, 0u16.to_le_bytes().to_vec()),
            (32..34, 2u16.to_le_bytes().to_vec()),
            (34..36, vec![0; 2]),
            (36..38, vec![0; 2]),
            (38..46, u64::MAX.to_le_bytes().to_vec()),
            (38..46, 0u64.to_le_bytes().to_vec()),
        ] {
            let mut data = valid.clone();
            data[range].copy_from_slice(&bytes);
            assert!(load_texture(data).is_err());
        }
    }

    #[derive(mev::Arguments)]
    struct ReadbackArguments {
        #[mev(sampled, compute)]
        image: mev::Image,
        #[mev(storage, compute)]
        pixels: mev::Buffer,
    }

    #[test]
    fn malformed_compression_tables_fail() {
        let valid = rgb(Compression::Ans);
        let reader = JackalImageReader::open(Cursor::new(&valid[..])).unwrap();
        let context_start = 38 + reader.tiles() * u64::SIZE;
        let context_end = reader.tile_offsets()[0] as usize;
        let mut empty = valid.clone();
        empty[context_start..context_end].fill(0);
        assert!(load_texture(empty).is_err());

        let mut malformed = Cursor::new(Vec::new());
        jkl::bits::write_bits_scope(&mut malformed, |bits| {
            jkl::algos::vle::encode(1usize, bits)?;
            jkl::algos::vle::encode_non_zero::<u32, _>(
                std::num::NonZeroU32::new(1).unwrap(),
                bits,
            )?;
            jkl::algos::vle::encode(0u32, bits)
        })
        .unwrap();
        let malformed = malformed.into_inner();
        let mut data = valid;
        data[context_start..context_end].fill(0);
        data[context_start..context_start + malformed.len()].copy_from_slice(&malformed);
        assert!(load_texture(data).is_err());
    }

    struct FixtureLoader(Box<[u8]>);
    impl arcana_assets::Loader for FixtureLoader {
        fn load<'a>(
            &'a self,
            _: arcana_assets::AssetId,
        ) -> futures::future::BoxFuture<'a, Result<arcana_assets::AssetData, AssetError>> {
            Box::pin(async {
                Ok(arcana_assets::AssetData {
                    bytes: self.0.clone(),
                    version: 0,
                })
            })
        }
        fn update<'a>(
            &'a self,
            _: arcana_assets::AssetId,
            _: u64,
        ) -> futures::future::BoxFuture<'a, Result<Option<arcana_assets::AssetData>, AssetError>>
        {
            Box::pin(async { Ok(None) })
        }
    }

    #[test]
    #[ignore = "requires a native GPU device"]
    fn gpu_texture_pixels() {
        let instance = mev::Instance::load().unwrap();
        let (idx, family) = instance
            .capabilities()
            .devices
            .iter()
            .enumerate()
            .find_map(|(idx, device)| {
                device
                    .families
                    .iter()
                    .position(|family| {
                        family.queue_flags.contains(
                            mev::QueueFlags::GRAPHICS
                                | mev::QueueFlags::COMPUTE
                                | mev::QueueFlags::TRANSFER,
                        )
                    })
                    .map(|family| (idx, family as u32))
            })
            .expect("graphics/compute/transfer queue");
        let features = mev::Features::BUFFER_MAP
            | mev::Features::BUFFER_MAP_AT_CREATION
            | mev::Features::TEXTURE_COMPRESSION_BC;
        assert!(
            instance.capabilities().devices[idx]
                .features
                .contains(features)
        );
        let (_, mut queues) = instance
            .new_device(mev::DeviceDesc {
                idx,
                queues: &[family],
                features,
            })
            .unwrap();
        let mut gpu = arcana_gpu::Gpu::new(queues.pop().unwrap()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let shader = gpu.device.new_shader_library(mev::LibraryDesc {
            name: "texture readback shader",
            input: mev::LibraryInput::wgsl("@group(0) @binding(0) var image: texture_2d<f32>; @group(0) @binding(1) var<storage, read_write> pixels: array<u32>; @compute @workgroup_size(8,8) fn readback(@builtin(global_invocation_id) id: vec3<u32>) { let dims = textureDimensions(image); if (id.x >= dims.x || id.y >= dims.y) { return; } let rgba = vec4<u32>(round(textureLoad(image, vec2<i32>(id.xy), 0) * 255.0)); pixels[id.y*dims.x+id.x] = rgba.x | (rgba.y<<8u) | (rgba.z<<16u) | (rgba.w<<24u); }"),
        }).unwrap();
        let pipeline = gpu
            .device
            .new_compute_pipeline(mev::ComputePipelineDesc {
                name: "texture readback",
                shader: shader.entry("readback"),
                work_group_size: [8, 8, 1],
                constants: 0,
                arguments: &[ReadbackArguments::LAYOUT],
            })
            .unwrap();
        let rgba = vec![
            Rgba8U([12, 34, 56, 78]),
            Rgba8U([192, 5, 32, 0]),
            Rgba8U([255, 240, 128, 255]),
            Rgba8U([0, 127, 255, 128]),
            Rgba8U([90, 80, 70, 60]),
            Rgba8U([25, 50, 75, 100]),
        ];
        let rgb_expected = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255,
            0, 0, 255,
        ];
        let mut fixtures = vec![
            crate::importer::tests::imported_png(),
            (rgb(Compression::Ans), (3, 2), rgb_expected.clone()),
            (rgb(Compression::Lz77Ans), (3, 2), rgb_expected),
            (
                encode(
                    Image::new_2d(3, 2, rgba.clone().into_boxed_slice()).as_ref(),
                    Compression::Ans,
                ),
                (3, 2),
                rgba.into_iter().flat_map(|pixel| pixel.0).collect(),
            ),
        ];
        for compression in [Compression::Ans, Compression::Lz77Ans] {
            let colors = [
                Rgb8U::RED,
                Rgb8U::GREEN,
                Rgb8U::BLUE,
                Rgb8U::WHITE,
                Rgb8U::BLACK,
                Rgb8U::RED,
            ];
            let blocks = colors.map(|color| bc1::Block {
                color0: jkl::math::Rgb565::from_8u(color),
                color1: jkl::math::Rgb565::BLACK,
                indices: [0; 4],
            });
            let mut expected = Vec::new();
            for block_row in colors.chunks(3) {
                for _ in 0..4 {
                    for color in block_row {
                        for _ in 0..4 {
                            expected.extend_from_slice(&[color.0[0], color.0[1], color.0[2], 255]);
                        }
                    }
                }
            }
            fixtures.push((
                encode(Image::new_2d(3, 2, blocks.as_slice()).as_ref(), compression),
                (12, 8),
                expected,
            ));
        }
        fixtures.extend(crate::importer::tests::configured_gpu_fixtures());
        for (data, (width, height), expected) in fixtures {
            let format = JackalImageReader::open(Cursor::new(&data[..]))
                .unwrap()
                .format();
            let assets = Assets::new([std::sync::Arc::new(FixtureLoader(data))
                as std::sync::Arc<dyn arcana_assets::Loader>]);
            let id = arcana_assets::AssetId::new(std::num::NonZeroU64::new(1).unwrap());
            let mut context = arcana_assets::AssetBuildContext::new();
            let texture = runtime.block_on(async {
                for _ in 0..1000 {
                    match assets.get::<Texture>(id) {
                        std::task::Poll::Ready(texture) => return texture.unwrap(),
                        std::task::Poll::Pending => {}
                    }
                    tokio::task::yield_now().await;
                    context.build_assets(&assets, &mut gpu).unwrap();
                }
                panic!("texture failed to finish building");
            });
            assert_eq!(texture.image.extent(), mev::Extent2::new(width, height));
            let packed = matches!(format, Format::R8 | Format::RG8);
            if packed {
                assert_eq!(
                    texture.image.format(),
                    if format == Format::R8 {
                        mev::PixelFormat::R8Unorm
                    } else {
                        mev::PixelFormat::Rg8Unorm
                    }
                );
            }
            let mut output = gpu.device.new_buffer(mev::BufferDesc {
                name: "texture readback",
                size: expected.len(),
                usage: mev::BufferUsage::STORAGE | mev::BufferUsage::HOST_READ,
            });
            let mut encoder = gpu.queue.new_command_encoder();
            {
                let mut compute = encoder.compute();
                compute.with_pipeline(&pipeline);
                compute.with_arguments(
                    0,
                    &ReadbackArguments {
                        image: texture.image.clone().into_image(),
                        pixels: output.clone(),
                    },
                );
                compute.dispatch(mev::Extent3::new(width.div_ceil(8), height.div_ceil(8), 1));
            }
            gpu.queue.submit_checkpoint([encoder.finish()]).unwrap();
            gpu.queue.wait_idle().unwrap();
            let mut actual = vec![0u8; expected.len()];
            output.read(0, &mut actual).unwrap();
            for (index, (&actual, &expected)) in actual.iter().zip(&expected).enumerate() {
                assert!(
                    actual.abs_diff(expected) <= if packed || index % 4 == 3 { 0 } else { 1 },
                    "pixel byte {index}: {actual} != {expected}"
                );
            }
        }
    }
}
