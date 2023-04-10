use std::{fmt, ops::Deref};

use crate::generic::OutOfMemory;

use super::{device::WeakDevice, Image};

#[derive(Debug)]
pub(crate) enum SurfaceErrorKind {
    OutOfMemory,
    SurfaceLost,
}

impl From<OutOfMemory> for SurfaceErrorKind {
    #[inline(always)]
    fn from(_: OutOfMemory) -> Self {
        SurfaceErrorKind::OutOfMemory
    }
}

impl fmt::Display for SurfaceErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SurfaceErrorKind::OutOfMemory => write!(f, "{OutOfMemory}"),
            SurfaceErrorKind::SurfaceLost => write!(f, "surface lost"),
        }
    }
}

struct Swapchain {
    swapchain: ash::vk::SwapchainKHR,
    images: Vec<Image>,
}

pub struct Surface {
    owner: WeakDevice,
    surface: ash::vk::SurfaceKHR,
    current: Option<Swapchain>,
    retired: Vec<Swapchain>,
}

impl Surface {
    pub(super) fn new(owner: WeakDevice, surface: ash::vk::SurfaceKHR) -> Self {
        Self {
            owner,
            surface,
            current: None,
            retired: Vec::new(),
        }
    }

    fn init(&mut self) {
        if let Some(swapchain) = self.current.take() {
            self.retired.push(swapchain);
        }

        let device = self.owner.upgrade().unwrap();
    }
}

pub struct SurfaceImage {
    swapchain: ash::vk::SwapchainKHR,
    image: Image,
}

impl SurfaceImage {
    pub(super) fn swapchain(self) -> ash::vk::SwapchainKHR {
        self.swapchain
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
