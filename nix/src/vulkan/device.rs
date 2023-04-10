use std::{
    ffi, fmt,
    sync::{Arc, Weak},
};

use ash::vk::{self, Handle};
use gpu_alloc_ash::AshMemoryDevice;
use parking_lot::Mutex;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use slab::Slab;

use crate::generic::{
    compile_shader, BufferDesc, CompareFunction, CreateLibraryError, CreatePipelineError, Features,
    ImageDesc, ImageError, LibraryDesc, LibraryInput, Memory, OutOfMemory, PrimitiveTopology,
    RenderPipelineDesc, ShaderLanguage, SurfaceError, VertexStepMode,
};

use super::{
    from::{IntoAsh, TryIntoAsh},
    handle_host_oom,
    queue::{Family, Queue},
    shader::Library,
    unexpected_error, Buffer, Image, RenderPipeline, Surface, Version,
};

struct DeviceInner {
    device: ash::Device,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    version: Version,
    families: Vec<Family>,
    features: Features,

    buffers: Mutex<Slab<vk::Buffer>>,
    images: Mutex<Slab<vk::Image>>,
    allocator: Mutex<gpu_alloc::GpuAllocator<vk::DeviceMemory>>,

    _entry: ash::Entry,

    #[cfg(any(debug_assertions, feature = "debug"))]
    debug_utils: Option<ash::extensions::ext::DebugUtils>,

    surface: Option<ash::extensions::khr::Surface>,
    swapchain: Option<ash::extensions::khr::Swapchain>,

    #[cfg(target_os = "windows")]
    win32_surface: Option<ash::extensions::khr::Win32Surface>,
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
    #[inline(always)]
    pub fn upgrade(&self) -> Option<Device> {
        self.inner.upgrade().map(|inner| Device { inner })
    }

    #[inline]
    pub fn drop_buffer(&self, idx: usize) {
        if let Some(inner) = self.inner.upgrade() {
            let mut buffers = inner.buffers.lock();
            let buffer = buffers.remove(idx);
            unsafe {
                inner.device.destroy_buffer(buffer, None);
            }
        }
    }

    #[inline]
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
        features: Features,
        allocator: gpu_alloc::GpuAllocator<vk::DeviceMemory>,
        #[cfg(any(debug_assertions, feature = "debug"))] debug_utils: Option<
            ash::extensions::ext::DebugUtils,
        >,
        surface: Option<ash::extensions::khr::Surface>,
        swapchain: Option<ash::extensions::khr::Swapchain>,
        #[cfg(target_os = "windows")] win32_surface: Option<ash::extensions::khr::Win32Surface>,
    ) -> Self {
        Device {
            inner: Arc::new(DeviceInner {
                device,
                instance,
                physical_device,
                version,
                families,
                features,
                buffers: Mutex::new(Slab::new()),
                images: Mutex::new(Slab::new()),
                allocator: Mutex::new(allocator),
                debug_utils,
                surface,
                swapchain,
                win32_surface,
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

    #[cfg(any(debug_assertions, feature = "debug"))]
    fn set_object_name(&self, ty: vk::ObjectType, handle: u64, name: &str) {
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

// #[hidden_trait::expose]
impl crate::traits::Device for Device {
    fn get_queue(&self, family: usize, idx: usize) -> Queue {
        let family = &self.inner.families[family];
        let handle = family.queues[idx];
        Queue::new(self.clone(), handle, family.flags)
    }

    fn new_shader_library(&self, desc: LibraryDesc) -> Result<Library, CreateLibraryError> {
        let me = &*self.inner;
        match desc.input {
            LibraryInput::Source(source) => {
                let compied: Box<[u32]>;
                let code = match source.language {
                    ShaderLanguage::SpirV => unsafe {
                        let (left, words, right) = source.code.align_to::<u32>();

                        if left.is_empty() && right.is_empty() {
                            words
                        } else {
                            let mut code = &*source.code;
                            let mut words = Vec::with_capacity(code.len() / 4);

                            while let [a, b, c, d, tail @ ..] = code {
                                words.push(u32::from_ne_bytes([*a, *b, *c, *d]));
                                code = tail;
                            }

                            compied = words.into();
                            &*compied
                        }
                    },
                    _ => {
                        compied = compile_shader(&source.code, source.filename, source.language)
                            .map_err(|err| CreateLibraryError(err.into()))?;
                        &*compied
                    }
                };
                let result = unsafe {
                    me.device.create_shader_module(
                        &ash::vk::ShaderModuleCreateInfo::builder().code(code),
                        None,
                    )
                };
                let module = result.map_err(|err| match err {
                    vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                    vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                        CreateLibraryError(OutOfMemory.into())
                    }
                    _ => unexpected_error(err),
                })?;

                Ok(Library::new(module))
            }
        }
    }

    fn new_render_pipeline(
        &self,
        desc: RenderPipelineDesc,
    ) -> Result<RenderPipeline, CreatePipelineError> {
        let me = &*self.inner;

        let vertex_attributes = desc
            .vertex_attributes
            .iter()
            .enumerate()
            .map(|(idx, attr)| ash::vk::VertexInputAttributeDescription {
                location: idx as u32,
                binding: attr.buffer_index,
                format: attr.format.try_into_ash().expect("Unsupported on Vulkan"),
                offset: attr.offset,
            })
            .collect::<Vec<_>>();

        let vertex_bindings = desc
            .vertex_layouts
            .iter()
            .enumerate()
            .map(|(idx, attr)| ash::vk::VertexInputBindingDescription {
                binding: idx as u32,
                stride: attr.stride,
                input_rate: match attr.step_mode {
                    VertexStepMode::Vertex => ash::vk::VertexInputRate::VERTEX,
                    VertexStepMode::Instance { rate: 1 } => ash::vk::VertexInputRate::INSTANCE,
                    VertexStepMode::Instance { rate } => {
                        panic!(
                            "Instance vertex step mode with rate {rate} is not supported on Vulkan"
                        )
                    }
                    VertexStepMode::Constant => {
                        panic!("Constant vertex step mode is not supported on Vulkan")
                    }
                },
            })
            .collect::<Vec<_>>();

        let mut names = Vec::with_capacity(2);

        let mut stages = vec![ash::vk::PipelineShaderStageCreateInfo::builder()
            .stage(ash::vk::ShaderStageFlags::VERTEX)
            .module(desc.vertex_shader.library.module())
            .name({
                let entry = std::ffi::CString::new(&*desc.vertex_shader.entry).unwrap();
                names.push(entry);
                names.last().unwrap() // Here CStr will not outlive `stages`, but we only use a raw pointer from it, which will be valid until end of this function.
            })
            .build()];

        let mut raster_state = ash::vk::PipelineRasterizationStateCreateInfo::builder();
        let mut depth_state = ash::vk::PipelineDepthStencilStateCreateInfo::builder();
        let mut attachments = Vec::new();

        if let Some(raster) = &desc.raster {
            if let Some(fragment_shader) = &raster.fragment_shader {
                stages.push(
                    ash::vk::PipelineShaderStageCreateInfo::builder()
                        .stage(ash::vk::ShaderStageFlags::FRAGMENT)
                        .module(fragment_shader.library.module())
                        .name({
                            let entry = std::ffi::CString::new(&*fragment_shader.entry).unwrap();
                            names.push(entry);
                            names.last().unwrap() // Here CStr will not outlive `stages`, but we only use a raw pointer from it, which will be valid until end of this function.
                        })
                        .build(),
                );
            }

            raster_state = raster_state
                .depth_clamp_enable(true)
                .rasterizer_discard_enable(false)
                .polygon_mode(ash::vk::PolygonMode::FILL)
                .cull_mode(ash::vk::CullModeFlags::BACK)
                .front_face(ash::vk::FrontFace::CLOCKWISE);

            if let Some(depth_stencil) = raster.depth_stencil {
                depth_state = depth_state
                    .depth_test_enable(depth_stencil.format.is_depth())
                    .depth_compare_op(depth_stencil.compare.into_ash())
                    .depth_write_enable(depth_stencil.write_enabled)
                    .stencil_test_enable(depth_stencil.format.is_stencil());
            }

            for color in &raster.color_targets {
                let mut blend_state = ash::vk::PipelineColorBlendAttachmentState::builder();
                if let Some(blend) = color.blend {
                    blend_state = blend_state
                        .blend_enable(true)
                        .src_color_blend_factor(blend.color.src.into_ash())
                        .dst_color_blend_factor(blend.color.dst.into_ash())
                        .color_blend_op(blend.color.op.into_ash())
                        .src_alpha_blend_factor(blend.alpha.src.into_ash())
                        .dst_alpha_blend_factor(blend.alpha.dst.into_ash())
                        .alpha_blend_op(blend.alpha.op.into_ash())
                        .color_write_mask(blend.mask.into_ash());
                }
                attachments.push(blend_state.build());
            }
        } else {
            raster_state = raster_state.rasterizer_discard_enable(true);
        }

        let result = unsafe {
            me.device.create_graphics_pipelines(
                ash::vk::PipelineCache::null(),
                std::slice::from_ref(
                    &ash::vk::GraphicsPipelineCreateInfo::builder()
                        .stages(&stages)
                        .vertex_input_state(
                            &ash::vk::PipelineVertexInputStateCreateInfo::builder()
                                .vertex_attribute_descriptions(&vertex_attributes)
                                .vertex_binding_descriptions(&vertex_bindings),
                        )
                        .input_assembly_state(
                            &ash::vk::PipelineInputAssemblyStateCreateInfo::builder().topology(
                                match desc.primitive_topology {
                                    PrimitiveTopology::Point => {
                                        ash::vk::PrimitiveTopology::POINT_LIST
                                    }
                                    PrimitiveTopology::Line => {
                                        ash::vk::PrimitiveTopology::LINE_LIST
                                    }
                                    PrimitiveTopology::Triangle => {
                                        ash::vk::PrimitiveTopology::TRIANGLE_LIST
                                    }
                                },
                            ),
                        )
                        .rasterization_state(&raster_state)
                        .multisample_state(
                            &ash::vk::PipelineMultisampleStateCreateInfo::builder()
                                .rasterization_samples(ash::vk::SampleCountFlags::TYPE_1),
                        )
                        .depth_stencil_state(&depth_state)
                        .color_blend_state(
                            &ash::vk::PipelineColorBlendStateCreateInfo::builder()
                                .attachments(&attachments)
                                .blend_constants([0.0; 4]),
                        )
                        .dynamic_state(
                            &ash::vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&[
                                ash::vk::DynamicState::VIEWPORT,
                                ash::vk::DynamicState::SCISSOR,
                            ]),
                        ),
                ),
                None,
            )
        };

        let pipelines = result.map_err(|(_, err)| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => CreatePipelineError(OutOfMemory.into()),
            _ => unexpected_error(err),
        })?;

        Ok(RenderPipeline::new(pipelines[0]))
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

        #[cfg(any(debug_assertions, feature = "debug"))]
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
                    .format(
                        desc.format
                            .try_into_ash()
                            .ok_or(ImageError::InvalidFormat)?,
                    )
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

        let requirements = unsafe { self.inner.device.get_image_memory_requirements(image) };
        let align_mask = requirements.alignment - 1;

        let block = unsafe {
            self.inner.allocator.lock().alloc(
                AshMemoryDevice::wrap(&self.inner.device),
                gpu_alloc::Request {
                    size: requirements.size,
                    align_mask,
                    usage: memory_to_usage_flags(Memory::Device),
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

        #[cfg(any(debug_assertions, feature = "debug"))]
        self.set_object_name(vk::ObjectType::IMAGE, image.as_raw(), desc.name);

        let idx = self.inner.images.lock().insert(image);

        let image = Image::new(
            self.weak(),
            image,
            desc.dimensions,
            desc.format,
            desc.usage,
            desc.layers,
            desc.levels,
            block,
            idx,
        );
        Ok(image)
    }

    fn new_surface(
        &self,
        window: &impl HasRawWindowHandle,
        display: &impl HasRawDisplayHandle,
    ) -> Result<Surface, SurfaceError> {
        let me = &*self.inner;
        assert!(
            me.features.contains(Features::SURFACE),
            "Surface feature is not enabled"
        );

        let window = window.raw_window_handle();
        let display = display.raw_display_handle();

        match (window, display) {
            #[cfg(target_os = "windows")]
            (RawWindowHandle::Win32(window), RawDisplayHandle::Windows(_)) => unsafe {
                let win32_surface = me.win32_surface.as_ref().unwrap();

                let surface = win32_surface
                    .create_win32_surface(
                        &ash::vk::Win32SurfaceCreateInfoKHR::builder()
                            // .hinstance(hinstance)
                            .hwnd(window.hwnd),
                        None,
                    )
                    .map_err(|err| match err {
                        vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
                        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => SurfaceError(OutOfMemory.into()),
                        err => unexpected_error(err),
                    })?;

                Ok(Surface::new(self.weak(), surface))
            },
            (RawWindowHandle::Win32(_), _) => {
                panic!("Mismatched window and display type")
            }
            _ => {
                unreachable!("Unsupported window type for this platform")
            }
        }
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
