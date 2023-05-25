use std::fmt;

use crate::generic::SurfaceError;

use super::{Image, Queue};

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
    fn next_frame(&mut self, _queue: &mut Queue) -> Result<Frame, SurfaceError> {
        let drawable = self
            .layer
            .next_drawable()
            .ok_or(SurfaceError(SurfaceErrorKind::SurfaceLost))?;
        let image = Image::new(drawable.texture().to_owned());
        Ok(Frame {
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

pub struct Frame {
    drawable: metal::MetalDrawable,
    image: Image,
}

impl Frame {
    #[inline(always)]
    pub(super) fn drawable(&self) -> &metal::MetalDrawableRef {
        &self.drawable
    }
}

#[hidden_trait::expose]
impl crate::traits::Frame for Frame {
    #[inline(always)]
    fn image(&self) -> &Image {
        &self.image
    }
}
