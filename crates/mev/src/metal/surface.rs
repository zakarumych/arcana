use std::fmt;

use core_graphics_types::{
    base::CGFloat,
    geometry::{CGRect, CGSize},
};
use objc::{msg_send, runtime::Object, sel, sel_impl};

use crate::generic::{PipelineStages, SurfaceError};

use super::{Image, Queue};

const UPDATE_SIZE_FRAMES_WAIT: u64 = 10;

pub struct Surface {
    layer: metal::MetalLayer,
    view: *mut objc::runtime::Object,
    next_update_frames_wait: u64,
}

unsafe impl Send for Surface {}

impl Drop for Surface {
    fn drop(&mut self) {
        if !self.view.is_null() {
            unsafe {
                let () = msg_send![self.view, release];
            }
        }
    }
}

impl Surface {
    pub(super) fn new(layer: metal::MetalLayer, view: *mut Object) -> Self {
        if !view.is_null() {
            unsafe {
                let () = msg_send![view, retain];
            }
        }

        Surface {
            layer,
            view,
            next_update_frames_wait: UPDATE_SIZE_FRAMES_WAIT,
        }
    }
}

unsafe fn window_scale_factor(view: *mut Object) -> f64 {
    let mut scale_factor: CGFloat = 1.0;
    unsafe {
        let window: *mut Object = msg_send![view, window];
        if !window.is_null() {
            scale_factor = msg_send![window, backingScaleFactor];
        }
    }
    scale_factor
}

unsafe fn view_size(view: *mut Object) -> CGSize {
    unsafe {
        let frame: CGRect = msg_send![view, bounds];
        frame.size
    }
}

#[hidden_trait::expose]
impl crate::traits::Surface for Surface {
    fn next_frame(
        &mut self,
        _queue: &mut Queue,
        _before: PipelineStages,
    ) -> Result<Frame, SurfaceError> {
        if self.next_update_frames_wait == 0 {
            if !self.view.is_null() {
                unsafe {
                    let draw_size = self.layer.drawable_size();

                    let scale = window_scale_factor(self.view);
                    let size = view_size(self.view);

                    if draw_size.width != size.width * scale
                        || draw_size.height != size.height * scale
                    {
                        self.layer.set_drawable_size(CGSize {
                            width: size.width * scale,
                            height: size.height * scale,
                        });
                        self.next_update_frames_wait = UPDATE_SIZE_FRAMES_WAIT;
                    }
                }
            }
        } else {
            self.next_update_frames_wait -= 1;
        }

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
