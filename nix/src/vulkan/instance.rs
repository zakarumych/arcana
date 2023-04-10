use std::{
    alloc::Layout,
    convert::identity,
    ffi::{c_void, CStr},
    fmt,
};

use ash::*;

use crate::generic::{
    Capabilities, CreateError, DeviceCapabilities, DeviceDesc, FamilyCapabilities, Features,
    LoadError, OutOfMemory,
};

use super::{
    device::Device, from::*, handle_host_oom, queue::Family, unexpected_error, Version, VERSION_1_1,
};

macro_rules! extension_name {
    ($name:literal) => {
        str::as_ptr(concat!($name, "\0")) as *const i8
    };
}

#[derive(Clone)]
pub struct Instance {
    entry: ash::Entry,
    version: Version,
    instance: ash::Instance,
    devices: Vec<vk::PhysicalDevice>,
    capabilities: Capabilities,

    #[cfg(any(debug_assertions, feature = "debug"))]
    debug_utils: Option<ash::extensions::ext::DebugUtils>,

    surface: Option<ash::extensions::khr::Surface>,

    #[cfg(target_os = "windows")]
    win32_surface: Option<ash::extensions::khr::Win32Surface>,
}

#[derive(Debug)]
pub(crate) enum LoadErrorKind {
    OutOfMemory,
    LoadingError(ash::LoadingError),
    InitializationFailed,
}

impl fmt::Display for LoadErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadErrorKind::LoadingError(err) => {
                write!(f, "failed to load Vulkan entry points: {}", err)
            }
            LoadErrorKind::OutOfMemory => write!(f, "{OutOfMemory}"),
            LoadErrorKind::InitializationFailed => write!(f, "initialization failed"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CreateErrorKind {
    OutOfMemory,
    InitializationFailed,
    TooManyObjects,
    DeviceLost,
}

impl fmt::Display for CreateErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateErrorKind::OutOfMemory => write!(f, "{OutOfMemory}"),
            CreateErrorKind::InitializationFailed => write!(f, "initialization failed"),
            CreateErrorKind::TooManyObjects => write!(f, "too many objects"),
            CreateErrorKind::DeviceLost => write!(f, "device lost"),
        }
    }
}

unsafe fn find_layer<'a>(
    layers: &'a [vk::LayerProperties],
    name: &str,
) -> Option<&'a vk::LayerProperties> {
    layers
        .iter()
        .find(|layer| CStr::from_ptr(layer.layer_name.as_ptr()).to_bytes() == name.as_bytes())
}

unsafe fn find_extension<'a>(
    extensions: &'a [vk::ExtensionProperties],
    name: &str,
) -> Option<&'a vk::ExtensionProperties> {
    extensions.iter().find(|extension| {
        CStr::from_ptr(extension.extension_name.as_ptr()).to_bytes() == name.as_bytes()
    })
}

fn engine_version() -> u32 {
    let major = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    let minor = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
    let patch = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
    vk::make_api_version(0, major, minor, patch)
}

impl Instance {
    pub fn load() -> Result<Self, LoadError> {
        // Load the Vulkan entry points.

        let entry =
            unsafe { Entry::load() }.map_err(|err| LoadError(LoadErrorKind::LoadingError(err)))?;

        // Collect instance layers and extensions.

        let layers = entry
            .enumerate_instance_layer_properties()
            .map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                    std::alloc::handle_alloc_error(Layout::new::<()>())
                }
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => LoadError(LoadErrorKind::OutOfMemory),
                err => unexpected_error(err),
            })?;

        let mut enabled_layer_names = Vec::new();

        let extensions = entry
            .enumerate_instance_extension_properties(None)
            .map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => LoadError(LoadErrorKind::OutOfMemory),
                vk::Result::ERROR_LAYER_NOT_PRESENT => unreachable!("No layer specified"),
                err => unexpected_error(err),
            })?;

        // Enable layers and instance extensions.

        let mut enabled_extension_names = Vec::new();

        #[cfg(any(debug_assertions, feature = "debug"))]
        if let Some(layer) = unsafe { find_layer(&layers, "VK_LAYER_KHRONOS_validation") } {
            enabled_layer_names.push(layer.layer_name.as_ptr());
        }

        #[cfg(any(debug_assertions, feature = "debug"))]
        let mut debug_utils = false;

        #[cfg(any(debug_assertions, feature = "debug"))]
        if let Some(extension) = unsafe { find_extension(&extensions, "VK_EXT_debug_utils") } {
            enabled_extension_names.push(extension.extension_name.as_ptr());
            debug_utils = true;
        }

        let mut has_surface = false;
        if let Some(surface_extension) = unsafe { find_extension(&extensions, "VK_KHR_surface") } {
            #[cfg(target_os = "windows")]
            let name = "VK_KHR_win32_surface";

            if let Some(platform_extension) = unsafe { find_extension(&extensions, name) } {
                has_surface = true;
                enabled_extension_names.push(surface_extension.extension_name.as_ptr());
                enabled_extension_names.push(platform_extension.extension_name.as_ptr());
            }
        }

        // Choose latest Vulkan version.

        let api_version = entry
            .try_enumerate_instance_version()
            .map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                _ => unreachable!(),
            })
            .unwrap_or_else(identity)
            .unwrap_or(vk::make_api_version(0, 1, 0, 0));

        let version = Version {
            major: vk::api_version_major(api_version),
            minor: vk::api_version_minor(api_version),
            patch: vk::api_version_patch(api_version),
        };

        // Create the Vulkan instance.

        let result = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::builder()
                    .application_info(
                        &vk::ApplicationInfo::builder()
                            .api_version(api_version)
                            .application_version(0)
                            .engine_name(CStr::from_bytes_with_nul(b"nothing-engine\0").unwrap())
                            .engine_version(engine_version()),
                    )
                    .enabled_layer_names(&enabled_layer_names)
                    .enabled_extension_names(&enabled_extension_names)
                    .push_next(
                        &mut vk::DebugUtilsMessengerCreateInfoEXT::builder()
                            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE)
                            .message_type(
                                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                            )
                            .pfn_user_callback(Some(vulkan_debug_callback))
                            .user_data(std::ptr::null_mut()),
                    ),
                None,
            )
        };

        let instance = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => LoadError(LoadErrorKind::OutOfMemory),
            vk::Result::ERROR_INITIALIZATION_FAILED => {
                LoadError(LoadErrorKind::InitializationFailed)
            }
            vk::Result::ERROR_LAYER_NOT_PRESENT => unreachable!("Layers were checked"),
            vk::Result::ERROR_EXTENSION_NOT_PRESENT => unreachable!("Extensions were checked"),
            vk::Result::ERROR_INCOMPATIBLE_DRIVER => unreachable!("Version was checked"),
            err => unexpected_error(err),
        })?;

        // Init debug utils extension functions

        #[cfg(any(debug_assertions, feature = "debug"))]
        let debug_utils =
            debug_utils.then(|| ash::extensions::ext::DebugUtils::new(&entry, &instance));

        // Init surface extension functions
        let mut surface = None;

        #[cfg(target_os = "windows")]
        let mut win32_surface = None;
        if has_surface {
            surface = Some(ash::extensions::khr::Surface::new(&entry, &instance));

            #[cfg(target_os = "windows")]
            {
                win32_surface = Some(ash::extensions::khr::Win32Surface::new(&entry, &instance));
            }
        }

        // Collect physical devices

        let devices =
            unsafe { instance.enumerate_physical_devices() }.map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => LoadError(LoadErrorKind::OutOfMemory),
                vk::Result::ERROR_INITIALIZATION_FAILED => {
                    LoadError(LoadErrorKind::InitializationFailed)
                }
                err => unexpected_error(err),
            })?;

        let mut device_caps = Vec::with_capacity(devices.len());

        for &device in &devices {
            let result = unsafe { instance.enumerate_device_extension_properties(device) };
            let extensions = result.map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => LoadError(LoadErrorKind::OutOfMemory),
                vk::Result::ERROR_LAYER_NOT_PRESENT => unreachable!("No layer specified"),
                err => unexpected_error(err),
            })?;

            let mut features = Features::empty();

            if has_surface {
                if let Some(extension) = unsafe { find_extension(&extensions, "VK_KHR_swapchain") }
                {
                    enabled_extension_names.push(extension.extension_name.as_ptr());
                    features |= Features::SURFACE;
                }
            }

            let memory = unsafe { instance.get_physical_device_memory_properties(device) };

            let families = if version >= VERSION_1_1 {
                let count =
                    unsafe { instance.get_physical_device_queue_family_properties2_len(device) };
                let mut families = vec![vk::QueueFamilyProperties2::default(); count];
                unsafe {
                    instance.get_physical_device_queue_family_properties2(device, &mut families);
                }

                families
                    .into_iter()
                    .map(FamilyCapabilities::from_ash)
                    .collect()
            } else {
                let families =
                    unsafe { instance.get_physical_device_queue_family_properties(device) };

                families
                    .into_iter()
                    .map(FamilyCapabilities::from_ash)
                    .collect()
            };

            device_caps.push(DeviceCapabilities {
                features: Features::empty(),
                families,
            })
        }

        // Build instance instance.

        Ok(Instance {
            version,
            entry,
            instance,
            devices,
            capabilities: Capabilities {
                devices: device_caps,
            },
            debug_utils,
            surface,

            #[cfg(target_os = "windows")]
            win32_surface,
        })
    }
}

#[hidden_trait::expose]
impl crate::traits::Instance for Instance {
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn create(&self, desc: DeviceDesc) -> Result<Device, CreateError> {
        let physical_device = self.devices[desc.idx];
        let device_caps = &self.capabilities.devices[desc.idx];

        // Check for duplicate queue families
        let duplicates = desc.queue_infos.iter().enumerate().any(|(idx, info)| {
            desc.queue_infos[..idx]
                .iter()
                .any(|other| other.family == info.family)
        });
        assert!(!duplicates, "Queue family specified more than once");

        // Collect queue create infos
        let mut queue_create_infos = desc
            .queue_infos
            .iter()
            .map(|queue_info| {
                u32::try_from(queue_info.queue_count).expect("Too many queues requested");
                let family_caps = &device_caps.families[queue_info.family as usize];
                let max_queue_count = family_caps.queue_count;
                assert!(
                    max_queue_count >= queue_info.queue_count,
                    "Family {} has {} queues, but {} were requested",
                    queue_info.family,
                    max_queue_count,
                    queue_info.queue_count
                );

                let priorities = vec![1.0; queue_info.queue_count];

                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_info.family)
                    .queue_priorities(&priorities)
                    .build()
            })
            .collect::<Vec<_>>();

        // Init memory allocator

        let limits = unsafe {
            self.instance
                .get_physical_device_properties(physical_device)
        }
        .limits;

        let memory_properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(physical_device)
        };

        let allocator = gpu_alloc::GpuAllocator::<vk::DeviceMemory>::new(
            gpu_alloc::Config::i_am_prototyping(),
            gpu_alloc::DeviceProperties {
                max_memory_allocation_count: limits.max_memory_allocation_count,
                max_memory_allocation_size: u64::max_value(), // FIXME: Can query this information if instance is v1.1
                non_coherent_atom_size: limits.non_coherent_atom_size,
                memory_types: memory_properties.memory_types
                    [..memory_properties.memory_type_count as usize]
                    .iter()
                    .map(|memory_type| gpu_alloc::MemoryType {
                        props: gpu_alloc_ash::memory_properties_from_ash(
                            memory_type.property_flags,
                        ),
                        heap: memory_type.heap_index,
                    })
                    .collect(),
                memory_heaps: memory_properties.memory_heaps
                    [..memory_properties.memory_heap_count as usize]
                    .iter()
                    .map(|&memory_heap| gpu_alloc::MemoryHeap {
                        size: memory_heap.size,
                    })
                    .collect(),
                buffer_device_address: false,
            },
        );

        let mut enabled_extension_names = Vec::new();
        if desc.features.contains(Features::SURFACE) {
            enabled_extension_names.push(extension_name!("VK_KHR_swapchain"));
        }

        let result = unsafe {
            self.instance.create_device(
                physical_device,
                &vk::DeviceCreateInfo::builder()
                    .enabled_extension_names(&enabled_extension_names)
                    .queue_create_infos(&queue_create_infos),
                None,
            )
        };

        let device = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => CreateError(CreateErrorKind::OutOfMemory),
            vk::Result::ERROR_INITIALIZATION_FAILED => {
                CreateError(CreateErrorKind::InitializationFailed)
            }
            vk::Result::ERROR_EXTENSION_NOT_PRESENT => unreachable!("Extensions were checked"),
            vk::Result::ERROR_FEATURE_NOT_PRESENT => unreachable!("Features were checked"),
            vk::Result::ERROR_TOO_MANY_OBJECTS => CreateError(CreateErrorKind::TooManyObjects),
            vk::Result::ERROR_DEVICE_LOST => CreateError(CreateErrorKind::DeviceLost),
            err => unexpected_error(err),
        })?;

        // Collect families from the device.
        let mut families = Vec::new();
        for family_info in desc.queue_infos {
            let family_caps = &device_caps.families[family_info.family as usize];
            let mut queues = Vec::new();
            for idx in 0..family_info.queue_count {
                let queue = unsafe { device.get_device_queue(family_info.family, idx as u32) };
                queues.push(queue);
            }
            families.push(Family {
                flags: family_caps.queue_flags,
                queues,
            });
        }

        let device = Device::new(
            self.version,
            self.entry.clone(),
            self.instance.clone(),
            physical_device,
            device,
            families,
            desc.features,
            allocator,
            #[cfg(any(debug_assertions, feature = "debug"))]
            self.debug_utils.clone(),
            self.surface.clone(),
            desc.features
                .contains(Features::SURFACE)
                .then(|| ash::extensions::khr::Swapchain::new(&self.instance, &device)),
            #[cfg(target_os = "windows")]
            self.win32_surface.clone(),
        );

        Ok(device)
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    vulkan_debug_callback_impl(message_severity, message_types, p_callback_data);
    vk::FALSE
}

unsafe fn vulkan_debug_callback_impl(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
) {
    let enabled = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
            tracing::event_enabled!(tracing::Level::TRACE)
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
            tracing::event_enabled!(tracing::Level::INFO)
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
            tracing::event_enabled!(tracing::Level::WARN)
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => {
            tracing::event_enabled!(tracing::Level::ERROR)
        }
        _ => unreachable!("Unexpected message severity"),
    };
    if !enabled {
        return;
    }

    let message_id_name = CStr::from_ptr((*p_callback_data).p_message_id_name)
        .to_str()
        .unwrap_or("<Non-UTF8>");
    let message_id_number = (*p_callback_data).message_id_number;
    let message = CStr::from_ptr((*p_callback_data).p_message)
        .to_str()
        .unwrap_or("<Non-UTF8>");

    let objects = (0..(*p_callback_data).object_count)
        .map(|idx| &*(*p_callback_data).p_objects.add(idx as usize))
        .map(|object| {
            (
                object.object_type,
                object.object_handle,
                CStr::from_ptr(object.p_object_name)
                    .to_str()
                    .unwrap_or("<Non-UTF8>"),
            )
        })
        .collect::<Vec<_>>();

    tracing::event!(
        target: "vulkan",
        tracing::Level::TRACE,
        message_id_name,
        message_id_number,
        message,
        objects = ?objects,
    );
}
