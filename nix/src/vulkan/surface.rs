use std::{
    collections::VecDeque,
    fmt,
    ops::Deref,
    time::{Duration, Instant},
};

use ash::vk;

use crate::generic::{
    ImageDimensions, OutOfMemory, PipelineStages, SurfaceError, Swizzle, ViewDesc,
};

use super::{
    device::WeakDevice,
    from::{AshInto, TryAshInto},
    handle_host_oom, unexpected_error, Device, Image, Queue,
};

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
    handle: vk::SwapchainKHR,
    images: Vec<(Image, [vk::Semaphore; 2])>,
    next: vk::Semaphore,
}

pub struct Surface {
    owner: WeakDevice,
    surface: vk::SurfaceKHR,
    current: Option<Swapchain>,
    retired: VecDeque<Swapchain>,
    caps: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    modes: Vec<vk::PresentModeKHR>,
    family_supports: Vec<bool>,

    preferred_format: vk::SurfaceFormatKHR,
    preferred_mode: vk::PresentModeKHR,
    preferred_usage: vk::ImageUsageFlags,
    bound_queue_family: Option<u32>,

    /// Time at which the swapchain should be retired
    /// Set to near future when swapchain become suboptimal.
    /// Reset is swapchain is optimal again.
    ///
    /// This ensures that we don't keep using a suboptimal swapchain
    /// while not recreating it too often.
    retire_deadline: Option<Instant>,

    /// Signals that surface or device was lost.
    lost: bool,
}

impl Surface {
    pub(super) fn new(
        owner: WeakDevice,
        surface: vk::SurfaceKHR,
        formats: Vec<vk::SurfaceFormatKHR>,
        modes: Vec<vk::PresentModeKHR>,
        family_supports: Vec<bool>,
    ) -> Self {
        let preferred_format = pick_format(&formats);
        let preferred_mode = pick_mode(&modes);

        Surface {
            owner,
            surface,
            current: None,
            retired: VecDeque::new(),
            caps: vk::SurfaceCapabilitiesKHR::default(),
            formats,
            modes,
            family_supports,

            preferred_format,
            preferred_mode,
            preferred_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            bound_queue_family: None,

            retire_deadline: None,
            lost: false,
        }
    }

    // Initialize the swapchain.
    // Retires any old swapchain.
    fn init(&mut self, device: &Device) -> Result<(), SurfaceError> {
        self.handle_retired(device)?;

        if self.lost {
            return Err(SurfaceError(SurfaceErrorKind::SurfaceLost));
        }

        self.retire_deadline = None;

        let old = self.current.take();

        let result = unsafe {
            device
                .surface()
                .get_physical_device_surface_capabilities(device.physical_device(), self.surface)
        };
        self.caps = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => SurfaceError(OutOfMemory.into()),
            vk::Result::ERROR_SURFACE_LOST_KHR => {
                self.lost = true;
                SurfaceError(SurfaceErrorKind::SurfaceLost)
            }
            _ => unexpected_error(err),
        })?;

        let result = unsafe {
            device.swapchain().create_swapchain(
                &vk::SwapchainCreateInfoKHR::builder()
                    .surface(self.surface)
                    .min_image_count(3.clamp(self.caps.min_image_count, self.caps.max_image_count))
                    .image_format(self.preferred_format.format)
                    .image_color_space(self.preferred_format.color_space)
                    .image_extent(self.caps.current_extent)
                    .image_array_layers(1)
                    .image_usage(self.caps.supported_usage_flags & self.preferred_usage)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .present_mode(self.preferred_mode)
                    .clipped(true)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
                    .old_swapchain(old.as_ref().map_or(vk::SwapchainKHR::null(), |s| s.handle)),
                None,
            )
        };

        // Old swapchain is retired even if the creation of the new one fails.
        if let Some(old) = old {
            self.retired.push_back(old);
        }

        let handle = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => SurfaceError(OutOfMemory.into()),
            vk::Result::ERROR_DEVICE_LOST | vk::Result::ERROR_SURFACE_LOST_KHR => {
                self.lost = true;
                SurfaceError(SurfaceErrorKind::SurfaceLost)
            }
            vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR => {
                panic!("Native window is already in use.");
            }
            vk::Result::ERROR_INITIALIZATION_FAILED => {
                panic!("Failed to create swapchain due to some implementation-specific reasons");
            }
            _ => unexpected_error(err),
        })?;

        let semaphore = |device: &ash::Device| {
            let result =
                unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::builder(), None) };
            result.map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => SurfaceError(OutOfMemory.into()),
                _ => unexpected_error(err),
            })
        };

        let next = semaphore(device.ash())?;

        let result = unsafe { device.swapchain().get_swapchain_images(handle) };
        let images = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => SurfaceError(OutOfMemory.into()),
            _ => unexpected_error(err),
        })?;

        let pixel_format = self.preferred_format.format.try_ash_into().unwrap();
        let usage = self.preferred_usage.ash_into();
        let dimensions = ImageDimensions::D2(
            self.caps.current_extent.width,
            self.caps.current_extent.height,
        );

        let mut swapchain_images = Vec::new();
        for image in &images {
            let (view, view_idx) = device
                .new_image_view(
                    *image,
                    dimensions,
                    ViewDesc {
                        format: pixel_format,
                        base_layer: 0,
                        layers: 1,
                        base_level: 0,
                        levels: 1,
                        swizzle: Swizzle::IDENTITY,
                    },
                )
                .unwrap();

            let acquire = semaphore(device.ash())?;
            let present = semaphore(device.ash())?;

            let image = Image::from_swapchain_image(
                device.weak(),
                *image,
                view,
                view_idx,
                ImageDimensions::D2(
                    self.caps.current_extent.width,
                    self.caps.current_extent.height,
                ),
                pixel_format,
                usage,
            );

            swapchain_images.push((image, [acquire, present]));
        }

        self.current = Some(Swapchain {
            handle,
            images: swapchain_images,
            next,
        });
        Ok(())
    }

    fn handle_retired(&mut self, device: &Device) -> Result<(), SurfaceError> {
        self.clear_retired(device)?;

        if self.retired.len() >= 8 {
            let result = unsafe { device.ash().device_wait_idle() };
            result.map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => SurfaceError(OutOfMemory.into()),
                vk::Result::ERROR_DEVICE_LOST => SurfaceError(SurfaceErrorKind::SurfaceLost),
                _ => unexpected_error(err),
            })?;

            todo!("Notify Queues that all epochs are done");

            self.clear_retired(device)?;
            assert_eq!(
                self.retired.len(),
                0,
                "User-code should not hold on to swapchain images."
            );
        }

        Ok(())
    }

    fn clear_retired(&mut self, device: &Device) -> Result<(), SurfaceError> {
        while let Some(mut swapchain) = self.retired.pop_front() {
            let can_destroy = swapchain
                .images
                .iter_mut()
                .all(|(image, _)| image.detached());
            if can_destroy {
                for (_, [acquire, present]) in swapchain.images {
                    unsafe {
                        device.ash().destroy_semaphore(acquire, None);
                        device.ash().destroy_semaphore(present, None);
                    }
                }
                unsafe {
                    device.swapchain().destroy_swapchain(swapchain.handle, None);
                }
            } else {
                // Do this later.
                self.retired.push_front(swapchain);
                break;
            }
        }

        Ok(())
    }
}

const SUBOPTIMAL_WAIT: Duration = Duration::from_secs(1);

#[hidden_trait::expose]
impl crate::traits::Surface for Surface {
    fn next_frame(
        &mut self,
        queue: &mut Queue,
        before: PipelineStages,
    ) -> Result<Frame, SurfaceError> {
        let device = self
            .owner
            .upgrade()
            .ok_or(SurfaceError(SurfaceErrorKind::SurfaceLost))?;

        self.clear_retired(&device)?;

        let now = Instant::now();

        loop {
            if let Some(deadline) = self.retire_deadline {
                if now >= deadline {
                    self.init(&device)?;
                }
            }

            if self.current.is_none() {
                self.init(&device)?;
            }

            let current = self.current.as_mut().unwrap();

            let result = unsafe {
                device.swapchain().acquire_next_image(
                    current.handle,
                    u64::MAX,
                    current.next,
                    vk::Fence::null(),
                )
            };
            let idx = match result {
                Ok((idx, false)) => idx,
                Ok((idx, true)) => {
                    self.retire_deadline = Some(now + SUBOPTIMAL_WAIT);
                    idx
                }
                Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => handle_host_oom(),
                Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                    return Err(SurfaceError(OutOfMemory.into()))
                }
                Err(
                    vk::Result::ERROR_DEVICE_LOST
                    | vk::Result::ERROR_SURFACE_LOST_KHR
                    | vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT,
                ) => {
                    self.lost = true;
                    return Err(SurfaceError(SurfaceErrorKind::SurfaceLost));
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.retire_deadline = Some(now);
                    continue;
                }
                Err(err) => unexpected_error(err),
            };

            let (image, [acquire, present]) = &mut current.images[idx as usize];
            std::mem::swap(&mut current.next, acquire);

            queue.add_wait(*acquire, before);

            return Ok(Frame {
                swapchain: current.handle,
                image: image.clone(),
                idx,
                present: *present,
            });
        }
    }
}

pub struct Frame {
    swapchain: vk::SwapchainKHR,
    image: Image,
    idx: u32,
    present: vk::Semaphore,
}

impl Frame {
    pub(super) fn swapchain(&self) -> vk::SwapchainKHR {
        self.swapchain
    }

    pub(super) fn image_idx(&self) -> u32 {
        self.idx
    }

    pub(super) fn present(&self) -> vk::Semaphore {
        self.present
    }
}

impl Deref for Frame {
    type Target = Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

#[hidden_trait::expose]
impl crate::traits::Frame for Frame {
    fn image(&self) -> &Image {
        &self.image
    }
}

fn pick_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    for &format in formats {
        if format.format == vk::Format::B8G8R8A8_SRGB {
            return format;
        }
    }
    for &format in formats {
        if format.format == vk::Format::R8G8B8A8_SRGB {
            return format;
        }
    }
    for &format in formats {
        if format.format == vk::Format::R8G8B8A8_UNORM {
            return format;
        }
    }
    for &format in formats {
        if format.format == vk::Format::B8G8R8A8_UNORM {
            return format;
        }
    }
    panic!("Can't pick present mode");
}

fn pick_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    for &mode in modes {
        if mode == vk::PresentModeKHR::MAILBOX {
            return mode;
        }
    }
    for &mode in modes {
        if mode == vk::PresentModeKHR::FIFO {
            return mode;
        }
    }
    for &mode in modes {
        if mode == vk::PresentModeKHR::IMMEDIATE {
            return mode;
        }
    }
    panic!("Can't pick present mode");
}
