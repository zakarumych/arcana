use std::{fmt, ops::Deref};

use crate::generic::SurfaceError;

use super::Image;

pub struct Surface {
    layer: metal::MetalLayer,
}

unsafe impl Send for Surface {}

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

#[derive(Debug)]
pub(crate) enum SurfaceErrorKind {
    SurfaceLost,
}

impl fmt::Display for SurfaceErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SurfaceErrorKind::SurfaceLost => write!(f, "surface lost"),
        }
    }
}

pub struct SurfaceImage {
    drawable: metal::MetalDrawable,
    image: Image,
}

impl SurfaceImage {
    pub(super) fn present(self) {
        self.drawable.present();
    }
}

impl Deref for SurfaceImage {
    type Target = Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

#[hidden_trait::expose]
impl crate::traits::SurfaceImage for SurfaceImage {
    fn image(&self) -> &Image {
        &self.image
    }
}
