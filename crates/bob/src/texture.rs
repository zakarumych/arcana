use std::future::{ready, Ready};

use crate::assets::BobBuilder;

#[derive(Clone)]
pub struct Texture {
    image: nix::Image,
}

impl argosy::Asset for Texture {
    type Decoded = (rapid_qoi::Qoi, Vec<u8>);
    type DecodeError = rapid_qoi::DecodeError;
    type BuildError = nix::ImageError;
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
    fn build(builder: &mut BobBuilder, decoded: Self::Decoded) -> Result<Self, nix::ImageError> {
        let (qoi, bytes) = decoded;

        let image = builder.device.new_image(nix::ImageDesc {
            name: "texture",
            dimensions: nix::ImageDimensions::D2(qoi.width, qoi.height),
            format: nix::PixelFormat::Rgba8Srgb,
            usage: nix::ImageUsage::SAMPLED | nix::ImageUsage::TRANSFER_DST,
            levels: 1,
            layers: 1,
        })?;

        let staging = builder.device.new_buffer_init(nix::BufferInitDesc {
            data: &bytes,
            name: "texture-staging",
            usage: nix::BufferUsage::TRANSFER_SRC,
            memory: nix::Memory::Upload,
        })?;

        builder.encoder.copy_buffer_to_image(
            &staging,
            0,
            0,
            0,
            &image,
            nix::Offset3::ZERO,
            nix::Extent3::new(qoi.width, qoi.height, 1),
            0..1,
            0,
        );

        Ok(Texture { image })
    }
}
