use std::{
    mem::{size_of, ManuallyDrop},
    sync::Arc,
};

use ash::vk;
use gpu_alloc::MemoryBlock;
use hashbrown::{hash_map::Entry, HashMap};
use parking_lot::Mutex;

use crate::generic::{
    ArgumentKind, Automatic, ImageDimensions, ImageUsage, OutOfMemory, PixelFormat, Sampled,
    Storage, Swizzle, ViewDesc,
};

use super::{
    arguments::ArgumentsField,
    device::{DeviceOwned, WeakDevice},
    refs::Refs,
    Device,
};

enum Flavor {
    Device {
        block: ManuallyDrop<MemoryBlock<vk::DeviceMemory>>,
        idx: usize,
    },
    Swapchain,
}

// Contains actual `vk::Image`
struct ImageData {
    owner: WeakDevice,
    format: PixelFormat,
    usage: ImageUsage,
    dimensions: ImageDimensions,
    layers: u32,
    levels: u32,
    flavor: Flavor,
    views: Mutex<HashMap<ViewDesc, (vk::ImageView, usize)>>,
}

impl Drop for ImageData {
    fn drop(&mut self) {
        self.owner
            .drop_image_views(self.views.get_mut().values().map(|(_, idx)| *idx));

        if let Flavor::Device { block, idx } = &mut self.flavor {
            self.owner
                .drop_image(*idx, unsafe { ManuallyDrop::take(block) });
        }
    }
}

struct Inner {
    data: Arc<ImageData>,
    desc: ViewDesc,
    usage: ImageUsage,
    dimensions: ImageDimensions,
    owner: WeakDevice,
}

#[derive(Clone)]
pub struct Image {
    handle: vk::Image,
    view: vk::ImageView,
    inner: Arc<Inner>,
}

impl DeviceOwned for Image {
    #[inline(always)]
    fn owner(&self) -> &WeakDevice {
        &self.inner.owner
    }
}

impl Image {
    fn build(
        owner: WeakDevice,
        handle: vk::Image,
        view: vk::ImageView,
        view_idx: usize,
        dimensions: ImageDimensions,
        format: PixelFormat,
        usage: ImageUsage,
        layers: u32,
        levels: u32,
        flavor: Flavor,
    ) -> Self {
        let desc = ViewDesc {
            format,
            base_layer: 0,
            layers,
            base_level: 0,
            levels,
            swizzle: Swizzle::IDENTITY,
        };

        let mut views = HashMap::new();
        views.insert(desc, (view, view_idx));

        Image {
            handle,
            view,
            inner: Arc::new(Inner {
                data: Arc::new(ImageData {
                    owner: owner.clone(),
                    dimensions,
                    format,
                    usage,
                    layers,
                    levels,
                    flavor,
                    views: Mutex::new(views),
                }),
                desc,
                dimensions,
                usage,
                owner,
            }),
        }
    }

    pub(super) fn new(
        owner: WeakDevice,
        handle: vk::Image,
        view: vk::ImageView,
        view_idx: usize,
        dimensions: ImageDimensions,
        format: PixelFormat,
        usage: ImageUsage,
        layers: u32,
        levels: u32,
        block: MemoryBlock<vk::DeviceMemory>,
        idx: usize,
    ) -> Self {
        Image::build(
            owner,
            handle,
            view,
            view_idx,
            dimensions,
            format,
            usage,
            layers,
            levels,
            Flavor::Device {
                block: ManuallyDrop::new(block),
                idx,
            },
        )
    }

    pub(super) fn from_swapchain_image(
        owner: WeakDevice,
        handle: vk::Image,
        view: vk::ImageView,
        view_idx: usize,
        dimensions: ImageDimensions,
        format: PixelFormat,
        usage: ImageUsage,
    ) -> Self {
        Image::build(
            owner,
            handle,
            view,
            view_idx,
            dimensions,
            format,
            usage,
            1,
            1,
            Flavor::Swapchain,
        )
    }

    #[inline(always)]
    pub(super) fn get_view(&self, device: &Device, desc: ViewDesc) -> Result<Image, OutOfMemory> {
        let desc = ViewDesc {
            base_layer: desc.base_layer + self.inner.desc.base_layer,
            base_level: desc.base_level + self.inner.desc.base_level,
            ..desc
        };

        if self.inner.desc == desc {
            return Ok(self.clone());
        }

        let view = match self.inner.data.views.lock().entry(desc) {
            Entry::Occupied(entry) => entry.get().0,
            Entry::Vacant(entry) => {
                let (view, idx) =
                    device.new_image_view(self.handle, self.inner.dimensions, desc)?;
                entry.insert((view, idx)).0
            }
        };

        Ok(Image {
            handle: self.handle,
            view,
            inner: Arc::new(Inner {
                data: self.inner.data.clone(),
                desc,
                dimensions: self.inner.dimensions,
                usage: self.inner.usage,
                owner: self.inner.owner.clone(),
            }),
        })
    }

    #[inline(always)]
    pub(super) fn handle(&self) -> vk::Image {
        self.handle
    }

    #[inline(always)]
    pub(super) fn view_handle(&self) -> vk::ImageView {
        self.view
    }

    #[inline(always)]
    pub(super) fn base_layer(&self) -> u32 {
        self.inner.desc.base_layer
    }

    #[inline(always)]
    pub(super) fn base_level(&self) -> u32 {
        self.inner.desc.base_level
    }
}

#[hidden_trait::expose]
impl crate::traits::Image for Image {
    #[inline(always)]
    fn format(&self) -> PixelFormat {
        self.inner.desc.format
    }

    #[inline(always)]
    fn dimensions(&self) -> ImageDimensions {
        self.inner.dimensions
    }

    #[inline(always)]
    fn layers(&self) -> u32 {
        self.inner.desc.layers
    }

    #[inline(always)]
    fn levels(&self) -> u32 {
        self.inner.desc.levels
    }

    #[inline(always)]
    fn view(&self, device: &Device, desc: ViewDesc) -> Result<Image, OutOfMemory> {
        self.get_view(device, desc)
    }

    #[inline(always)]
    fn detached(&self) -> bool {
        // If strong is 1, it cannot be changed by another thread if called owns
        // mutable reference to self
        // since there are no weaks.
        debug_assert_eq!(Arc::weak_count(&self.inner), 0, "No weak refs allowed");
        debug_assert_eq!(Arc::weak_count(&self.inner.data), 0, "No weak refs allowed");
        Arc::strong_count(&self.inner) == 1 && Arc::strong_count(&self.inner.data) == 1
    }
}

impl ArgumentsField<Automatic> for Image {
    const KIND: ArgumentKind = <Self as ArgumentsField<Sampled>>::KIND;
    const SIZE: usize = <Self as ArgumentsField<Sampled>>::SIZE;
    const OFFSET: usize = <Self as ArgumentsField<Sampled>>::OFFSET;
    const STRIDE: usize = <Self as ArgumentsField<Sampled>>::STRIDE;

    type Update = <Self as ArgumentsField<Sampled>>::Update;

    #[inline(always)]
    fn update(&self) -> <Self as ArgumentsField<Sampled>>::Update {
        <Self as ArgumentsField<Sampled>>::update(self)
    }

    #[inline(always)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_image(self.clone());
    }
}

impl ArgumentsField<Sampled> for Image {
    const KIND: ArgumentKind = ArgumentKind::SampledImage;
    const SIZE: usize = 1;
    const OFFSET: usize = 0;
    const STRIDE: usize = size_of::<vk::DescriptorImageInfo>();

    type Update = vk::DescriptorImageInfo;

    #[inline(always)]
    fn update(&self) -> vk::DescriptorImageInfo {
        vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: self.view,
            image_layout: vk::ImageLayout::GENERAL,
        }
    }

    #[inline(always)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_image(self.clone());
    }
}

impl ArgumentsField<Storage> for Image {
    const KIND: ArgumentKind = ArgumentKind::StorageImage;
    const SIZE: usize = 1;
    const OFFSET: usize = 0;
    const STRIDE: usize = size_of::<vk::DescriptorImageInfo>();

    type Update = vk::DescriptorImageInfo;

    #[inline(always)]
    fn update(&self) -> vk::DescriptorImageInfo {
        vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: self.view,
            image_layout: vk::ImageLayout::GENERAL,
        }
    }

    #[inline(always)]
    fn add_refs(&self, refs: &mut Refs) {
        refs.add_image(self.clone());
    }
}
