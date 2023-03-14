use crate::generic::PixelFormat;

use super::from::TryMetalInto;

#[derive(Clone)]
pub struct Image {
    texture: metal::Texture,
}

unsafe impl Send for Image {}

impl Image {
    pub(super) fn new(texture: metal::Texture) -> Self {
        Image { texture }
    }

    pub(super) fn texture(&self) -> &metal::Texture {
        &self.texture
    }

    pub fn format(&self) -> PixelFormat {
        self.texture.pixel_format().try_metal_into().unwrap()
    }
}
