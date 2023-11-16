use std::future::{ready, Ready};

use edict::Component;

use crate::assets::BobBuilder;

#[derive(Clone)]
pub struct Texture {
    pub image: mev::Image,
}

impl Component for Texture {
    fn name() -> &'static str {
        "Texture"
    }
}

impl argosy::Asset for Texture {
    type Decoded = (rapid_qoi::Qoi, Vec<u8>);
    type DecodeError = rapid_qoi::DecodeError;
    type BuildError = mev::OutOfMemory;
    type Fut = Ready<Result<(rapid_qoi::Qoi, Vec<u8>), rapid_qoi::DecodeError>>;

    fn name() -> &'static str {
        "texture"
    }

    fn decode(bytes: Box<[u8]>, _: &argosy::Loader) -> Self::Fut {
        let result = rapid_qoi::Qoi::decode_alloc(&*bytes);
        ready(result)
    }
}

impl argosy::AssetBuild<BobBuilder<'_>> for Texture {
    fn build(builder: &mut BobBuilder, decoded: Self::Decoded) -> Result<Self, mev::OutOfMemory> {
        let (qoi, bytes) = decoded;

        let image = builder.device.new_image(mev::ImageDesc {
            name: "texture",
            dimensions: mev::ImageDimensions::D2(qoi.width, qoi.height),
            format: mev::PixelFormat::Rgba8Srgb,
            usage: mev::ImageUsage::SAMPLED | mev::ImageUsage::TRANSFER_DST,
            levels: 1,
            layers: 1,
        })?;

        let staging = builder.device.new_buffer_init(mev::BufferInitDesc {
            data: &bytes,
            name: "texture-staging",
            usage: mev::BufferUsage::TRANSFER_SRC,
            memory: mev::Memory::Upload,
        })?;

        builder.encoder.copy_buffer_to_image(
            &staging,
            0,
            0,
            0,
            &image,
            mev::Offset3::ZERO,
            mev::Extent3::new(qoi.width, qoi.height, 1),
            0..1,
            0,
        );

        Ok(Texture { image })
    }
}
