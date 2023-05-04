use std::{mem::ManuallyDrop, ops::Range, sync::Arc};

use ash::vk;
use gpu_alloc::MemoryBlock;
use hashbrown::HashMap;
use parking_lot::RwLock;

use crate::generic::{ImageDimensions, ImageUsage, OutOfMemory, PixelFormat};

use super::{device::WeakDevice, from::TryIntoAsh, handle_host_oom, unexpected_error};

enum Flavor {
    Device {
        block: ManuallyDrop<MemoryBlock<vk::DeviceMemory>>,
        idx: usize,
    },
    Swapchain,
}

struct Inner {
    owner: WeakDevice,
    dimensions: ImageDimensions,
    format: PixelFormat,
    usage: ImageUsage,
    layers: u32,
    levels: u32,
    flavor: Flavor,
    views: RwLock<HashMap<(Range<u32>, Range<u32>), vk::ImageView>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Flavor::Device { block, idx } = &mut self.flavor {
            self.owner
                .drop_buffer(*idx, unsafe { ManuallyDrop::take(block) });
        }
    }
}

#[derive(Clone)]
pub struct Image {
    handle: vk::Image,
    inner: Arc<Inner>,
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
            handle,
            inner: Arc::new(Inner {
                owner,
                dimensions,
                format,
                usage,
                layers,
                levels,
                flavor: Flavor::Device {
                    block: ManuallyDrop::new(block),
                    idx,
                },
                views: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub(super) fn from_swapchain_image(
        owner: WeakDevice,
        handle: vk::Image,
        dimensions: ImageDimensions,
        format: PixelFormat,
        usage: ImageUsage,
    ) -> Self {
        Image {
            handle,
            inner: Arc::new(Inner {
                owner,
                dimensions,
                format,
                usage,
                layers: 1,
                levels: 1,
                flavor: Flavor::Swapchain,
                views: RwLock::new(HashMap::new()),
            }),
        }
    }

    #[inline(always)]
    pub(super) fn extent_2d(&self) -> vk::Extent2D {
        match self.inner.dimensions {
            ImageDimensions::D1(width) => vk::Extent2D { width, height: 1 },
            ImageDimensions::D2(width, height) => vk::Extent2D { width, height },
            ImageDimensions::D3(width, height, _) => vk::Extent2D { width, height },
        }
    }

    #[inline(always)]
    pub(super) fn extent_3d(&self) -> vk::Extent3D {
        match self.inner.dimensions {
            ImageDimensions::D1(width) => vk::Extent3D {
                width,
                height: 1,
                depth: 1,
            },
            ImageDimensions::D2(width, height) => vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            ImageDimensions::D3(width, height, depth) => vk::Extent3D {
                width,
                height,
                depth,
            },
        }
    }

    #[inline(always)]
    pub(super) fn view(
        &self,
        device: &ash::Device,
        levels: Range<u32>,
        layers: Range<u32>,
    ) -> Result<vk::ImageView, OutOfMemory> {
        if let Some(&view) = self
            .inner
            .views
            .read()
            .get(&(levels.clone(), layers.clone()))
        {
            return Ok(view);
        }
        self.new_view(device, levels, layers)
    }

    #[inline(never)]
    #[cold]
    fn new_view(
        &self,
        device: &ash::Device,
        levels: Range<u32>,
        layers: Range<u32>,
    ) -> Result<vk::ImageView, OutOfMemory> {
        let result = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::builder()
                    .image(self.handle)
                    .view_type(match self.inner.dimensions {
                        ImageDimensions::D1(..) => vk::ImageViewType::TYPE_1D,
                        ImageDimensions::D2(..) => vk::ImageViewType::TYPE_2D,
                        ImageDimensions::D3(..) => vk::ImageViewType::TYPE_3D,
                    })
                    .format(self.inner.format.try_into_ash().unwrap())
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(format_aspect(self.inner.format))
                            .base_mip_level(levels.start)
                            .level_count(levels.end - levels.start)
                            .base_array_layer(layers.start)
                            .layer_count(layers.end - layers.start)
                            .build(),
                    ),
                None,
            )
        };

        let view = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
            _ => unexpected_error(err),
        })?;

        let mut views = self.inner.views.write();
        views.insert((levels, layers), view);
        Ok(view)
    }

    #[inline(always)]
    pub(super) fn handle(&self) -> vk::Image {
        self.handle
    }
}

#[hidden_trait::expose]
impl crate::traits::Image for Image {
    #[inline(always)]
    fn format(&self) -> PixelFormat {
        self.inner.format
    }

    #[inline(always)]
    fn dimensions(&self) -> ImageDimensions {
        self.inner.dimensions
    }

    #[inline(always)]
    fn layers(&self) -> u32 {
        self.inner.layers
    }

    #[inline(always)]
    fn levels(&self) -> u32 {
        self.inner.levels
    }

    #[inline(always)]
    fn detached(&self) -> bool {
        // If strong is 1, it cannot be changed by another thread
        // since there are no weaks and &mut self is exclusive.
        debug_assert_eq!(Arc::weak_count(&self.inner), 0, "No weak refs allowed");
        Arc::strong_count(&self.inner) == 1
    }
}

#[inline(always)]
fn format_aspect(format: PixelFormat) -> vk::ImageAspectFlags {
    let mut aspect = vk::ImageAspectFlags::empty();
    if format.is_color() {
        aspect |= vk::ImageAspectFlags::COLOR;
    }
    if format.is_depth() {
        aspect |= vk::ImageAspectFlags::DEPTH;
    }
    if format.is_stencil() {
        aspect |= vk::ImageAspectFlags::STENCIL;
    }
    aspect
}
