use ash::vk::{self};
use gpu_alloc::MemoryBlock;

use crate::generic::{ImageDimensions, ImageUsage, PixelFormat};

use super::device::WeakDevice;

enum Flavor {
    Device {
        block: MemoryBlock<vk::DeviceMemory>,
        idx: usize,
    },
    Surface {},
}

pub struct Image {
    handle: vk::Image,
    owner: WeakDevice,
    dimensions: ImageDimensions,
    format: PixelFormat,
    usage: ImageUsage,
    layers: u32,
    levels: u32,
    flavor: Flavor,
}

impl Drop for Image {
    fn drop(&mut self) {
        match &self.flavor {
            Flavor::Device { idx, .. } => self.owner.drop_image(*idx),
            Flavor::Surface { .. } => {}
        }
    }
}

impl Image {
    pub(super) fn new(
        owner: WeakDevice,
        handle: vk::Image,
        dimensions: ImageDimensions,
        format: PixelFormat,
        usage: ImageUsage,
        layers: u32,
        levels: u32,
        block: MemoryBlock<vk::DeviceMemory>,
        idx: usize,
    ) -> Self {
        Image {
            owner,
            handle,
            dimensions,
            format,
            usage,
            layers,
            levels,
            flavor: Flavor::Device { block, idx },
        }
    }

    pub(super) fn handle(&self) -> vk::Image {
        self.handle
    }
}
