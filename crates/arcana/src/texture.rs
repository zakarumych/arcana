use std::future::Future;

use basis_universal::{self, TranscodeError, TranscodeParameters, TranscoderTextureFormat};
use edict::component::Component;
use mev::Extent2;
use smallvec::SmallVec;

use crate::assets::{Asset, AssetBuilder, Assets};

#[derive(Clone)]
pub struct Texture {
    pub image: mev::Image,
}

impl Component for Texture {
    fn name() -> &'static str {
        "Texture"
    }
}

pub struct LoadedTexture {
    extent: Extent2,

    level_offsets: SmallVec<[usize; 8]>,
    transcoded_bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
enum TextureError {
    #[error("Invalid data")]
    InvalidData,
    #[error("Invalid image count")]
    InvalidImageCount,
    #[error("No image levels found")]
    NoImageLevels,
    #[error("Image type not supported")]
    ImageTypeNotSupported,
    #[error("Format not supported")]
    FormatNotSupported,
    #[error("Decode failed")]
    DecodeFailed,
}

impl Asset for Texture {
    type Loaded = LoadedTexture;

    fn load(
        data: Box<[u8]>,
        assets: &Assets,
    ) -> impl Future<Output = Result<Self::Loaded, crate::assets::Error>> + Send {
        futures::future::ready(load_texture(data, assets))
    }

    fn build(
        loaded: LoadedTexture,
        builder: &mut AssetBuilder,
    ) -> Result<Self, crate::assets::Error> {
        let image = builder
            .device()
            .new_image(mev::ImageDesc {
                dimensions: loaded.extent.into(),
                format: mev::PixelFormat::Rgba8Unorm,
                usage: mev::ImageUsage::SAMPLED | mev::ImageUsage::TRANSFER_DST,
                layers: 1,
                levels: loaded.level_offsets.len() as u32,
                name: "texture",
            })
            .map_err(crate::assets::Error::new)?;

        let scratch = builder
            .device()
            .new_buffer_init(mev::BufferInitDesc {
                data: &loaded.transcoded_bytes,
                usage: mev::BufferUsage::TRANSFER_SRC,
                memory: mev::Memory::Upload,
                name: "scratch",
            })
            .map_err(crate::assets::Error::new)?;

        let mut encoder = builder.encoder().copy();

        encoder.init_image(
            mev::PipelineStages::empty(),
            mev::PipelineStages::all(),
            &image,
        );

        for (level, offset) in std::iter::once(0).chain(loaded.level_offsets).enumerate() {
            encoder.copy_buffer_to_image(
                &scratch,
                offset,
                4 * loaded.extent.width() as usize,
                4 * loaded.extent.width() as usize * loaded.extent.height() as usize,
                &image,
                mev::Offset3::ZERO,
                loaded.extent.to_3d(),
                0..1,
                level as u32,
            );
        }

        Ok(Texture { image })
    }
}

fn load_texture(data: Box<[u8]>, _assets: &Assets) -> Result<LoadedTexture, crate::assets::Error> {
    let mut transcoder = basis_universal::Transcoder::new();

    if !transcoder.validate_header(&data) {
        return Err(crate::assets::Error::new(TextureError::InvalidData));
    }

    match transcoder.basis_texture_type(&data) {
        basis_universal::BasisTextureType::TextureType2D => {
            let image_count = transcoder.image_count(&data);
            if image_count != 1 {
                return Err(crate::assets::Error::new(TextureError::InvalidImageCount));
            }

            let info = transcoder.image_info(&data, 0).unwrap();

            let image_level_count = transcoder.image_level_count(&data, 0);
            if image_level_count == 0 {
                return Err(crate::assets::Error::msg("No image levels found"));
            }

            let mut level_offsets = SmallVec::new();
            let mut transcoded_bytes = Vec::new();

            for l in 0..image_level_count {
                if let Err(()) = transcoder.prepare_transcoding(&data) {
                    return Err(crate::assets::Error::new(TextureError::NoImageLevels));
                }

                let result = transcoder.transcode_image_level(
                    &data,
                    TranscoderTextureFormat::RGBA32,
                    TranscodeParameters {
                        image_index: 0,
                        level_index: l,
                        decode_flags: None,
                        output_row_pitch_in_blocks_or_pixels: None,
                        output_rows_in_pixels: None,
                    },
                );

                match result {
                    Err(TranscodeError::TranscodeFormatNotSupported) => {
                        return Err(crate::assets::Error::new(TextureError::FormatNotSupported));
                    }
                    Err(TranscodeError::ImageLevelNotFound) => {
                        unreachable!();
                    }
                    Err(TranscodeError::TranscodeFailed) => {
                        return Err(crate::assets::Error::new(TextureError::DecodeFailed))
                    }
                    Ok(bytes) => {
                        if l != 0 {
                            level_offsets.push(transcoded_bytes.len());
                        }
                        transcoded_bytes.extend_from_slice(&bytes);
                    }
                }
            }

            Ok(LoadedTexture {
                extent: Extent2::new(info.m_width, info.m_height),
                level_offsets,
                transcoded_bytes,
            })
        }
        _ => {
            return Err(crate::assets::Error::new(
                TextureError::ImageTypeNotSupported,
            ))
        }
    }
}
