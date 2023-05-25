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

    pub fn format(&self) -> PixelFormat {
        self.texture.pixel_format().try_metal_into().unwrap()
    }

    pub(super) fn metal(&self) -> &metal::Texture {
        &self.texture
    }
}

#[hidden_trait::expose]
impl crate::traits::Image for Image {
    fn id(&self) -> ImageId {
        self.texture.gpu_resource_id()
    }
}

#[repr(transparent)]
pub struct ImageId(u64);

impl crate::private::Sealed for ImageId {}
impl crate::traits::Argument for ImageId {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Image;
}

impl<const N: usize> crate::private::Sealed for [ImageId; N] {}
impl<const N: usize> crate::traits::Argument for [ImageId; N] {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Image;
}

impl crate::private::Sealed for [ImageId] {}
impl crate::traits::Argument for [ImageId] {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Image;
}
