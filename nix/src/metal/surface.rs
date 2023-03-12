use crate::generic::SurfaceError;

use super::Image;

pub struct Surface {
    layer: metal::MetalLayer,
}

impl Surface {
    pub(super) fn new(layer: metal::MetalLayer) -> Self {
        Surface { layer }
    }
}

#[hidden_trait::expose]
impl crate::traits::Surface for Surface {
    fn next_image(&mut self) -> Result<SurfaceImage, SurfaceError> {
        let drawable = self
            .layer
            .next_drawable()
            .ok_or(SurfaceError(SurfaceErrorKind::SurfaceLost))?;
        let image = Image::new(drawable.texture().to_owned());
        Ok(SurfaceImage {
            drawable: drawable.to_owned(),
            image,
        })
    }
}

pub(crate) enum SurfaceErrorKind {
    SurfaceLost,
}

pub struct SurfaceImage {
    drawable: metal::MetalDrawable,
    image: Image,
}

#[hidden_trait::expose]
impl crate::traits::SurfaceImage for SurfaceImage {
    fn image(&self) -> &Image {
        &self.image
    }
}
