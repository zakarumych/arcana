use std::{
    ffi, fmt,
    sync::{Arc, Weak},
};

use ash::vk::{self, Handle};
use gpu_alloc_ash::AshMemoryDevice;
use parking_lot::Mutex;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use slab::Slab;

use crate::generic::{BufferDesc, ImageDesc, ImageError, Memory, OutOfMemory};

use super::{
    from::IntoAsh,
    handle_host_oom,
    queue::{Family, Queue},
    unexpected_error, Buffer, Image, Surface, Version,
};

struct DeviceInner {
    device: ash::Device,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    version: Version,
    families: Vec<Family>,

    buffers: Mutex<Slab<vk::Buffer>>,
    images: Mutex<Slab<vk::Image>>,
    allocator: Mutex<gpu_alloc::GpuAllocator<vk::DeviceMemory>>,

    _entry: ash::Entry,

    #[cfg(any(debug_assertions, feature = "debug"))]
    debug_utils: Option<ash::extensions::ext::DebugUtils>,
}

#[derive(Clone)]
pub struct Device {
    inner: Arc<DeviceInner>,
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Device({:p}@{:p})",
            self.inner.device.handle(),
            self.inner.instance.handle()
        )
    }
}

pub(super) struct WeakDevice {
    inner: std::sync::Weak<DeviceInner>,
}

impl WeakDevice {
    pub fn drop_buffer(&self, idx: usize) {
        if let Some(inner) = self.inner.upgrade() {
            let mut buffers = inner.buffers.lock();
            let buffer = buffers.remove(idx);
            unsafe {
                inner.device.destroy_buffer(buffer, None);
            }
        }
    }

    pub fn drop_image(&self, idx: usize) {
        if let Some(inner) = self.inner.upgrade() {
            let mut images = inner.images.lock();
            let image = images.remove(idx);
            unsafe {
                inner.device.destroy_image(image, None);
            }
        }
    }
}

pub(super) trait DeviceOwned {
    fn owner(&self) -> &WeakDevice;
}

impl Device {
    pub(super) fn new(
        version: Version,
        entry: ash::Entry,
        instance: ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: ash::Device,
        families: Vec<Family>,
        allocator: gpu_alloc::GpuAllocator<vk::DeviceMemory>,

        #[cfg(any(debug_assertions, feature = "debug"))] debug_utils: Option<
            ash::extensions::ext::DebugUtils,
        >,
    ) -> Self {
        Device {
            inner: Arc::new(DeviceInner {
                device,
                instance,
                physical_device,
                version,
                families,
                buffers: Mutex::new(Slab::new()),
                images: Mutex::new(Slab::new()),
                allocator: Mutex::new(allocator),
                debug_utils,
                _entry: entry,
            }),
        }
    }

    pub(super) fn ash(&self) -> &ash::Device {
        &self.inner.device
    }

    pub(super) fn is(&self, weak: &WeakDevice) -> bool {
        Arc::as_ptr(&self.inner) == Weak::as_ptr(&weak.inner)
    }

    pub(super) fn is_owner(&self, owned: &impl DeviceOwned) -> bool {
        self.is(owned.owner())
    }

    pub(super) fn weak(&self) -> WeakDevice {
        WeakDevice {
            inner: Arc::downgrade(&self.inner),
        }
    }

    fn set_object_name(&self, ty: vk::ObjectType, handle: u64, name: &str) {
        #[cfg(any(debug_assertions, feature = "debug"))]
        {
            if !name.is_empty() {
                if let Some(debug_utils) = &self.inner.debug_utils {
                    let name_cstr = ffi::CString::new(name).unwrap();
                    let _ = unsafe {
                        debug_utils.set_debug_utils_object_name(
                            self.inner.device.handle(),
                            &vk::DebugUtilsObjectNameInfoEXT::builder()
                                .object_type(ty)
                                .object_handle(handle)
                                .object_name(&name_cstr),
                        )
                    };
                }
            }
        }
    }
}

#[hidden_trait::expose]
impl crate::generic::Device for Device {
    fn get_queue(&self, family: usize, idx: usize) -> Queue {
        let family = &self.inner.families[family];
        let handle = family.queues[idx];
        Queue::new(self.clone(), handle, family.flags)
    }

    fn new_buffer(&self, desc: BufferDesc) -> Result<Buffer, OutOfMemory> {
        let size = u64::try_from(desc.layout.size()).map_err(|_| OutOfMemory)?;
        let align = u64::try_from(desc.layout.align()).map_err(|_| OutOfMemory)?;

        let buffer = unsafe {
            self.inner.device.create_buffer(
                &vk::BufferCreateInfo::builder()
                    .size(size)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .usage(desc.usage.into_ash()),
                None,
            )
        }
        .map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
            err => unexpected_error(err),
        })?;

        let requirements = unsafe { self.inner.device.get_buffer_memory_requirements(buffer) };

        let align_mask = requirements.alignment.max(align) - 1;

        let block = unsafe {
            self.inner.allocator.lock().alloc(
                AshMemoryDevice::wrap(&self.inner.device),
                gpu_alloc::Request {
                    size: requirements.size,
                    align_mask,
                    usage: memory_to_usage_flags(desc.memory),
                    memory_types: requirements.memory_type_bits,
                },
            )
        }
        .map_err(|err| match err {
            gpu_alloc::AllocationError::OutOfDeviceMemory => OutOfMemory,
            gpu_alloc::AllocationError::OutOfHostMemory => handle_host_oom(),
            gpu_alloc::AllocationError::NoCompatibleMemoryTypes => OutOfMemory,
            gpu_alloc::AllocationError::TooManyObjects => OutOfMemory,
        })?;

        self.set_object_name(vk::ObjectType::BUFFER, buffer.as_raw(), desc.name);

        let idx = self.inner.buffers.lock().insert(buffer);

        let buffer = Buffer::new(self.weak(), buffer, desc.layout, desc.usage, block, idx);
        Ok(buffer)
    }

    fn new_image(&self, desc: ImageDesc) -> Result<Image, ImageError> {
        let image = unsafe {
            self.inner.device.create_image(
                &vk::ImageCreateInfo::builder()
                    .image_type(desc.dimensions.into_ash())
                    .format(desc.format.into_ash())
                    .array_layers(desc.layers)
                    .mip_levels(desc.levels)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage((desc.usage, desc.format).into_ash()),
                None,
            )
        }
        .map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => ImageError::OutOfMemory,
            err => unexpected_error(err),
        })?;
    }

    fn new_surface(&self, window: &RawWindowHandle, display: &RawDisplayHandle) -> Surface {
        todo!()
    }
}

fn memory_to_usage_flags(memory: Memory) -> gpu_alloc::UsageFlags {
    match memory {
        Memory::Device => gpu_alloc::UsageFlags::FAST_DEVICE_ACCESS,
        Memory::Shared => gpu_alloc::UsageFlags::HOST_ACCESS,
        Memory::Upload => gpu_alloc::UsageFlags::HOST_ACCESS | gpu_alloc::UsageFlags::UPLOAD,
        Memory::Download => gpu_alloc::UsageFlags::HOST_ACCESS | gpu_alloc::UsageFlags::DOWNLOAD,
    }
}
