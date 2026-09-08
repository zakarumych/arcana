use std::future::Future;

use arcana_assets::{Asset, AssetBuilder, AssetError, AssetVersion, Assets};
use arcana_id::has_stid;
use edict::component::Component;
use smallvec::SmallVec;

/// An asset that stores an image that can be used for sampling in GPU jobs.
#[derive(Clone)]
pub struct Texture {
    pub image: mev::Image,
}

has_stid!(Texture @ arcana_id);

impl Component for Texture {
    fn name() -> &'static str {
        "Texture"
    }
}

#[allow(dead_code)]
pub struct LoadedTexture {
    extent: mev::ImageExtent,
    format: mev::PixelFormat,
    line_pitch: usize,
    plane_pitch: usize,

    level_offsets: SmallVec<[usize; 8]>,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[allow(dead_code)]
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

impl AssetVersion for Texture {
    #[inline]
    fn version() -> u64
    where
        Self: Sized,
    {
        0
    }
}

impl Asset for Texture {
    type Loaded = LoadedTexture;

    fn load(
        _data: &[u8],
        _assets: &Assets,
    ) -> impl Future<Output = Result<Self::Loaded, AssetError>> + Send {
        async move { todo!() }
    }

    fn build(_loaded: LoadedTexture, _builder: &mut AssetBuilder) -> Result<Self, AssetError> {
        todo!()
    }
}
