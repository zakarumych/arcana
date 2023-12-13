//! Contains logic for the viewports.

use std::{any::TypeId, marker::PhantomData, process::Command, ptr::NonNull};

use edict::{
    archetype::Archetype,
    query::{Fetch, IntoQuery, WriteAlias},
    Access, Component, EntityId, Query,
};
use mev::{Extent2, Surface};
use winit::window::Window;

/// Viewport is where content of the game is displayed.
/// It is semi-opaque as users usually do not need care about what is behind it.
///
/// Viewport has a size and may be presented to by `RenderGraph`.
///
/// `RenderGraph::present` will present to main viewport which is resource in the `World`.
/// `RenderGraph::present_to` takes `EntityId` where it will look for `Viewport` component.
pub struct Viewport {
    kind: ViewportKind,
}

const SURFACE_RECREATE_TRIES: usize = 2;

enum ViewportKind {
    Window {
        // Drop it first.
        surface: Option<mev::Surface>,
        window: Window,
    },
    Texture {
        image: Option<mev::Image>,
    },
}

impl Component for Viewport {
    fn name() -> &'static str {
        "Viewport"
    }
}

impl Viewport {
    pub fn new_window(window: Window) -> Self {
        Viewport {
            kind: ViewportKind::Window {
                surface: None,
                window,
            },
        }
    }

    pub fn new_texture() -> Self {
        Viewport {
            kind: ViewportKind::Texture { image: None },
        }
    }

    pub fn extent(&self) -> Extent2 {
        match &self.kind {
            ViewportKind::Window { window, .. } => {
                let size = window.inner_size();
                Extent2::new(size.width as u32, size.height as u32)
            }
            ViewportKind::Texture { image: Some(image) } => image.dimensions().to_2d(),
            ViewportKind::Texture { .. } => Extent2::ZERO,
        }
    }

    pub fn set_image(&mut self, image: mev::Image) {
        match &mut self.kind {
            ViewportKind::Texture { image: i } => match image.dimensions() {
                mev::ImageExtent::D1(_) => panic!("Cannot set 1D image to viewport"),
                mev::ImageExtent::D2(e) => {
                    *i = Some(image);
                }
                mev::ImageExtent::D3(_) => panic!("Cannot set 3D image to viewport"),
            },
            _ => panic!("Cannot set image to window viewport"),
        }
    }

    pub fn get_image(&self) -> Option<&mev::Image> {
        match &self.kind {
            ViewportKind::Texture { image, .. } => image.as_ref(),
            _ => panic!("Cannot get image from window viewport"),
        }
    }

    #[doc(hidden)]
    pub fn next_frame(
        &mut self,
        device: &mev::Device,
        queue: &mut mev::Queue,
        before: mev::PipelineStages,
    ) -> Result<Option<(mev::Image, Option<mev::Frame>)>, mev::SurfaceError> {
        match &mut self.kind {
            ViewportKind::Window { surface, window } => {
                if window.inner_size().width == 0 || window.inner_size().height == 0 {
                    surface.take();
                    return Ok(None);
                }

                for _ in 0..SURFACE_RECREATE_TRIES {
                    let s = match surface {
                        Some(surface) => surface,
                        None => {
                            let new_surface = device.new_surface(&*window, &*window)?;
                            surface.get_or_insert(new_surface)
                        }
                    };
                    let frame = match s.next_frame() {
                        Ok(mut frame) => {
                            queue.sync_frame(&mut frame, before);
                            frame
                        }
                        Err(mev::SurfaceError::SurfaceLost) => {
                            surface.take();
                            continue;
                        }
                        Err(err) => return Err(err),
                    };
                    return Ok(Some((frame.image().clone(), Some(frame))));
                }
                Err(mev::SurfaceError::SurfaceLost)
            }
            ViewportKind::Texture { image } => match image.clone() {
                Some(image) => Ok(Some((image, None))),
                None => Ok(None),
            },
        }
    }

    #[doc(hidden)]
    pub fn get_window(&self) -> &Window {
        match &self.kind {
            ViewportKind::Window { window, .. } => window,
            _ => panic!("Cannot get window from texture viewport"),
        }
    }

    #[doc(hidden)]
    pub fn get_window_mut(&mut self) -> &mut Window {
        match &mut self.kind {
            ViewportKind::Window { window, .. } => window,
            _ => panic!("Cannot get window from texture viewport"),
        }
    }
}
