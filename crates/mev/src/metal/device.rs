use std::sync::Arc;

use core_graphics_types::{base::CGFloat, geometry::CGRect};
use foreign_types::ForeignType;
use hashbrown::HashMap;
use metal::{CAMetalLayer, NSUInteger, SamplerDescriptor};
use objc::{
    class, msg_send,
    runtime::{Object, BOOL, YES},
    sel, sel_impl,
};

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    generic::{
        parse_shader, BufferDesc, BufferInitDesc, CreateLibraryError, CreatePipelineError,
        ImageDesc, ImageDimensions, LibraryDesc, LibraryInput, Memory, OutOfMemory,
        RenderPipelineDesc, SamplerDesc, ShaderCompileError, ShaderLanguage, SurfaceError,
        VertexStepMode,
    },
    ArgumentKind,
};

use super::{
    from::{IntoMetal, TryIntoMetal},
    shader::Bindings,
    Buffer, CreatePipelineErrorKind, Image, Library, RenderPipeline, Sampler, Surface,
    MAX_VERTEX_BUFFERS,
};

#[derive(Clone)]
pub struct Device {
    device: metal::Device,
}

unsafe impl Sync for Device {}
unsafe impl Send for Device {}

impl Device {
    pub(super) fn new(device: metal::Device) -> Self {
        Device { device }
    }
}

#[hidden_trait::expose]
impl crate::traits::Device for Device {
    fn new_shader_library(&self, desc: LibraryDesc) -> Result<Library, CreateLibraryError> {
        match desc.input {
            LibraryInput::Source(source) => {
                let options = metal::CompileOptions::new();
                options.set_language_version(metal::MTLLanguageVersion::V2_2);

                match source.language {
                    ShaderLanguage::Msl => {
                        let source = std::str::from_utf8(&*source.code).map_err(|err| {
                            CreateLibraryError::CompileError(ShaderCompileError::NonUtf8(err))
                        })?;

                        let library = self
                            .device
                            .new_library_with_source(&source, &options)
                            .unwrap();

                        Ok(Library::new(library))
                    }

                    src => {
                        let compiled = compile_shader(&source.code, source.filename, src)
                            .map_err(|err| CreateLibraryError::CompileError(err))?;

                        let library = self
                            .device
                            .new_library_with_source(&compiled.code, &options)
                            .unwrap();

                        Ok(Library::with_per_entry_point_bindings(
                            library,
                            compiled.per_entry_point_bindings,
                        ))
                    }
                }
            }
        }
    }

    fn new_render_pipeline(
        &self,
        desc: RenderPipelineDesc,
    ) -> Result<RenderPipeline, CreatePipelineError> {
        let mdesc = metal::RenderPipelineDescriptor::new();

        let vertex_function = desc
            .vertex_shader
            .library
            .get_function(&desc.vertex_shader.entry)
            .ok_or_else(|| CreatePipelineError(CreatePipelineErrorKind::InvalidShaderEntry))?;

        mdesc.set_vertex_function(Some(&vertex_function));

        let vertex_bindings = desc
            .vertex_shader
            .library
            .get_bindings(&desc.vertex_shader.entry);

        let vertex_buffers_count = desc
            .arguments
            .iter()
            .map(|g| {
                g.arguments
                    .iter()
                    .filter(|a| {
                        matches!(
                            a.kind,
                            ArgumentKind::UniformBuffer | ArgumentKind::StorageBuffer
                        )
                    })
                    .count()
            })
            .sum::<usize>()
            + vertex_bindings
                .as_ref()
                .map_or(0, |b| b.push_constants.is_some() as usize);

        if vertex_buffers_count > MAX_VERTEX_BUFFERS as _ {
            panic!("Too many buffer arguments and attribute buffers");
        }

        let mut fragment_bindings = None;

        let vertex_desc = metal::VertexDescriptor::new();

        let layouts = vertex_desc.layouts();
        for (idx, vertex_layout) in desc.vertex_layouts.iter().enumerate() {
            let layout_desc = metal::VertexBufferLayoutDescriptor::new();
            layout_desc.set_stride(vertex_layout.stride as _);
            match vertex_layout.step_mode {
                VertexStepMode::Vertex => {
                    layout_desc.set_step_function(metal::MTLVertexStepFunction::PerVertex)
                }
                VertexStepMode::Instance { rate } => {
                    layout_desc.set_step_rate(rate as _);
                    layout_desc.set_step_function(metal::MTLVertexStepFunction::PerInstance)
                }
                VertexStepMode::Constant => {
                    layout_desc.set_step_function(metal::MTLVertexStepFunction::Constant)
                }
            }
            layouts.set_object_at((vertex_buffers_count + idx) as _, Some(&layout_desc));
        }

        let attributes = vertex_desc.attributes();
        for (idx, vertex_attribute) in desc.vertex_attributes.iter().enumerate() {
            let attribute_desc = metal::VertexAttributeDescriptor::new();
            attribute_desc.set_format(vertex_attribute.format.try_into_metal().unwrap());
            attribute_desc.set_offset(vertex_attribute.offset as _);
            attribute_desc.set_buffer_index(
                (vertex_buffers_count as u32 + vertex_attribute.buffer_index) as _,
            );
            attributes.set_object_at(idx as _, Some(&attribute_desc));
        }

        mdesc.set_vertex_descriptor(Some(&vertex_desc));
        mdesc.set_input_primitive_topology(desc.primitive_topology.into_metal());

        if let Some(raster) = desc.raster {
            if let Some(fragment_shader) = raster.fragment_shader {
                let fragment_function = fragment_shader
                    .library
                    .get_function(&fragment_shader.entry)
                    .ok_or_else(|| {
                        CreatePipelineError(CreatePipelineErrorKind::InvalidShaderEntry)
                    })?;

                mdesc.set_fragment_function(Some(&fragment_function));

                fragment_bindings = fragment_shader.library.get_bindings(&fragment_shader.entry);
            }

            let color_attachments = mdesc.color_attachments();
            for (idx, color_desc) in raster.color_targets.iter().enumerate() {
                let color_attachment = color_attachments.object_at(idx as _).unwrap();
                color_attachment.set_pixel_format(color_desc.format.try_into_metal().unwrap());

                if let Some(blend_desc) = &color_desc.blend {
                    color_attachment.set_blending_enabled(true);
                    color_attachment.set_write_mask(blend_desc.mask.into_metal());
                    color_attachment.set_rgb_blend_operation(blend_desc.color.op.into_metal());
                    color_attachment.set_source_rgb_blend_factor(blend_desc.color.src.into_metal());
                    color_attachment
                        .set_destination_rgb_blend_factor(blend_desc.color.dst.into_metal());
                    color_attachment.set_alpha_blend_operation(blend_desc.alpha.op.into_metal());
                    color_attachment
                        .set_source_alpha_blend_factor(blend_desc.alpha.src.into_metal());
                    color_attachment
                        .set_destination_alpha_blend_factor(blend_desc.alpha.dst.into_metal());
                } else {
                    color_attachment.set_blending_enabled(false);
                }
                color_attachments.set_object_at(idx as _, Some(&color_attachment));
            }

            if let Some(depth_stencil) = raster.depth_stencil {
                let format = depth_stencil.format.try_into_metal().unwrap();
                if depth_stencil.format.is_depth() {
                    mdesc.set_depth_attachment_pixel_format(format);
                }
                if depth_stencil.format.is_stencil() {
                    mdesc.set_stencil_attachment_pixel_format(format);
                }
            }
        }

        let pipeline = self
            .device
            .new_render_pipeline_state(&mdesc)
            .map_err(|err| {
                CreatePipelineError(CreatePipelineErrorKind::FailedToBuildPipeline(err))
            })?;

        Ok(RenderPipeline::new(
            pipeline,
            desc.primitive_topology.into_metal(),
            vertex_bindings,
            fragment_bindings,
            vertex_buffers_count as u32,
        ))
    }

    fn new_buffer(&self, desc: BufferDesc) -> Result<Buffer, OutOfMemory> {
        let mut options = metal::MTLResourceOptions::empty();

        match desc.memory {
            Memory::Device => options |= metal::MTLResourceOptions::StorageModePrivate,
            Memory::Shared => options |= metal::MTLResourceOptions::StorageModeShared,
            Memory::Upload => {
                options |= metal::MTLResourceOptions::StorageModeManaged
                    | metal::MTLResourceOptions::CPUCacheModeWriteCombined
            }
            Memory::Download => options |= metal::MTLResourceOptions::StorageModeManaged,
        }

        let buffer = self.device.new_buffer(desc.size as _, options);
        Ok(Buffer::new(buffer))
    }

    fn new_buffer_init(&self, desc: BufferInitDesc) -> Result<Buffer, OutOfMemory> {
        let Ok(len) = u64::try_from(desc.data.len()) else {
            return Err(OutOfMemory);
        };

        let mut options = metal::MTLResourceOptions::empty();

        match desc.memory {
            Memory::Device => options |= metal::MTLResourceOptions::StorageModePrivate,
            Memory::Shared => options |= metal::MTLResourceOptions::StorageModeShared,
            Memory::Upload => {
                options |= metal::MTLResourceOptions::StorageModeManaged
                    | metal::MTLResourceOptions::CPUCacheModeWriteCombined
            }
            Memory::Download => options |= metal::MTLResourceOptions::StorageModeManaged,
        }

        let buffer = self
            .device
            .new_buffer_with_data(desc.data.as_ptr().cast(), len, options);
        Ok(Buffer::new(buffer))
    }

    fn new_image(&self, desc: ImageDesc) -> Result<Image, OutOfMemory> {
        let texture_descriptor = metal::TextureDescriptor::new();
        texture_descriptor.set_pixel_format(desc.format.try_into_metal().unwrap());
        match desc.dimensions {
            ImageDimensions::D1(size) => {
                texture_descriptor.set_texture_type(metal::MTLTextureType::D1);
                texture_descriptor.set_width(size as _);
            }
            ImageDimensions::D2(width, height) => {
                texture_descriptor.set_texture_type(metal::MTLTextureType::D2);
                texture_descriptor.set_width(width as _);
                texture_descriptor.set_height(height as _);
            }
            ImageDimensions::D3(width, height, depth) => {
                texture_descriptor.set_texture_type(metal::MTLTextureType::D3);
                texture_descriptor.set_width(width as _);
                texture_descriptor.set_height(height as _);
                texture_descriptor.set_depth(depth as _);
            }
        }
        texture_descriptor.set_mipmap_level_count(desc.levels as _);
        texture_descriptor.set_array_length(desc.layers as _);
        texture_descriptor.set_sample_count(1);
        texture_descriptor.set_usage(desc.usage.into_metal());
        texture_descriptor.set_storage_mode(metal::MTLStorageMode::Private);

        let texture = self.device.new_texture(&texture_descriptor);
        Ok(Image::new(texture))
    }

    fn new_sampler(&self, desc: SamplerDesc) -> Result<Sampler, OutOfMemory> {
        let sdesc = SamplerDescriptor::new();
        sdesc.set_min_filter(desc.min_filter.into_metal());
        sdesc.set_mag_filter(desc.mag_filter.into_metal());
        sdesc.set_mip_filter(desc.mip_map_mode.into_metal());
        sdesc.set_address_mode_s(desc.address_mode[0].into_metal());
        sdesc.set_address_mode_t(desc.address_mode[1].into_metal());
        sdesc.set_address_mode_r(desc.address_mode[2].into_metal());
        if let Some(anisotropy) = desc.anisotropy {
            sdesc.set_max_anisotropy((anisotropy as NSUInteger).clamp(1, 16));
        }
        sdesc.set_lod_min_clamp(desc.min_lod);
        sdesc.set_lod_max_clamp(desc.max_lod);
        sdesc.set_normalized_coordinates(desc.normalized);
        let state = self.device.new_sampler(&sdesc);
        Ok(Sampler::new(state))
    }

    fn new_surface(
        &self,
        window: &impl HasRawWindowHandle,
        display: &impl HasRawDisplayHandle,
    ) -> Result<Surface, SurfaceError> {
        let window = window.raw_window_handle();
        let display = display.raw_display_handle();
        match (window, display) {
            (RawWindowHandle::UiKit(handle), RawDisplayHandle::UiKit(_)) => unsafe {
                let layer = layer_from_view(handle.ui_view.cast());
                layer.set_device(&self.device);
                Ok(Surface::new(layer, std::ptr::null_mut()))
            },
            (RawWindowHandle::AppKit(handle), RawDisplayHandle::AppKit(_)) => unsafe {
                let layer = layer_from_view(handle.ns_view.cast());
                layer.set_device(&self.device);
                Ok(Surface::new(layer, handle.ns_view.cast()))
            },
            (RawWindowHandle::UiKit(_), _) | (RawWindowHandle::AppKit(_), _) => {
                panic!("Mismatched window and display type")
            }
            _ => unreachable!("Unsupported window type for the metal backend"),
        }
    }
}

unsafe fn layer_from_view(view: *mut Object) -> metal::MetalLayer {
    let main_layer: *mut Object = msg_send![view, layer];
    let class = class!(CAMetalLayer);
    let is_valid_layer: BOOL = msg_send![main_layer, isKindOfClass: class];

    if is_valid_layer == YES {
        unsafe { ForeignType::from_ptr(main_layer.cast()) }
    } else {
        let new_layer: *mut CAMetalLayer = msg_send![class, new];
        let frame: CGRect = msg_send![main_layer, bounds];
        let () = msg_send![new_layer, setFrame: frame];
        #[cfg(target_os = "macos")]
        {
            let () = msg_send![view, setLayer: new_layer];
            let () = msg_send![view, setWantsLayer: YES];
            let () = unsafe { msg_send![new_layer, setContentsGravity: kCAGravityTopLeft] };
            let window: *mut Object = msg_send![view, window];
            if !window.is_null() {
                let scale_factor: CGFloat = msg_send![window, backingScaleFactor];
                let () = msg_send![new_layer, setContentsScale: scale_factor];
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let () = msg_send![main_layer, addSublayer: new_layer];
            let () = msg_send![main_layer, setAutoresizingMask: 0x1Fu64];
            let screen: *mut Object = msg_send![class!(UIScreen), mainScreen];
            let scale_factor: CGFloat = msg_send![screen, nativeScale];
            let () = msg_send![view, setContentScaleFactor: scale_factor];
        }
        unsafe { ForeignType::from_ptr(new_layer) }
    }
}

#[cfg(target_os = "macos")]
#[link(name = "QuartzCore", kind = "framework")]
extern "C" {
    #[allow(non_upper_case_globals)]
    static kCAGravityTopLeft: *mut Object;
}

struct CompiledMetalShader {
    code: String,
    per_entry_point_bindings: HashMap<String, Arc<Bindings>>,
}

fn compile_shader(
    code: &[u8],
    filename: Option<&str>,
    lang: ShaderLanguage,
) -> Result<CompiledMetalShader, ShaderCompileError> {
    let (module, info, _source_code) = parse_shader(code, filename, lang)?;

    let mut options = naga::back::msl::Options {
        lang_version: (2, 4),
        per_entry_point_map: Default::default(),
        inline_samplers: Vec::new(),
        spirv_cross_compatibility: false,
        fake_missing_bindings: false,
        bounds_check_policies: Default::default(),
        zero_initialize_workgroup_memory: false,
    };

    let mut per_entry_point_bindings = HashMap::new();
    for (i, entry) in module.entry_points.iter().enumerate() {
        let mut bindings = Bindings::new();
        let mut map = naga::back::msl::EntryPointResources::default();
        map.sizes_buffer = Some(30);
        let mut next_buffer_slot = 0u8;
        let mut next_texture_slot = 0u8;
        let mut next_sampler_slot = 0u8;

        for (global_handle, global_variable) in module.global_variables.iter() {
            let function_info = info.get_entry_point(i);
            let usage = function_info[global_handle];
            if usage.is_empty() {
                continue;
            }

            if let naga::AddressSpace::PushConstant = global_variable.space {
                map.push_constant_buffer = Some(next_buffer_slot);
                bindings.set_push_constants(next_buffer_slot);
                next_buffer_slot += 1;
                continue;
            }

            if let Some(binding) = global_variable.binding.clone() {
                match global_variable.space {
                    naga::AddressSpace::PushConstant => unreachable!(),
                    naga::AddressSpace::Uniform => {
                        map.resources.insert(
                            binding.clone(),
                            naga::back::msl::BindTarget {
                                buffer: Some(next_buffer_slot),
                                texture: None,
                                sampler: None,
                                binding_array_size: None,
                                mutable: false,
                            },
                        );
                        bindings.insert(binding, next_buffer_slot);
                        next_buffer_slot += 1;
                    }
                    naga::AddressSpace::Storage { access } => {
                        map.resources.insert(
                            binding.clone(),
                            naga::back::msl::BindTarget {
                                buffer: Some(next_buffer_slot),
                                texture: None,
                                sampler: None,
                                binding_array_size: None,
                                mutable: access.contains(naga::StorageAccess::STORE),
                            },
                        );
                        bindings.insert(binding, next_buffer_slot);
                        next_buffer_slot += 1;
                    }
                    naga::AddressSpace::Handle => {
                        let ty = &module.types[global_variable.ty];
                        match ty.inner {
                            naga::TypeInner::Sampler { .. } => {
                                map.resources.insert(
                                    binding.clone(),
                                    naga::back::msl::BindTarget {
                                        buffer: None,
                                        texture: None,
                                        sampler: Some(
                                            naga::back::msl::BindSamplerTarget::Resource(
                                                next_sampler_slot,
                                            ),
                                        ),
                                        binding_array_size: None,
                                        mutable: false,
                                    },
                                );
                                bindings.insert(binding, next_sampler_slot);
                                next_sampler_slot += 1;
                            }
                            naga::TypeInner::Image { class, .. } => {
                                map.resources.insert(
                                    binding.clone(),
                                    naga::back::msl::BindTarget {
                                        buffer: None,
                                        texture: Some(next_texture_slot),
                                        sampler: None,
                                        binding_array_size: None,
                                        mutable: matches!(class, naga::ImageClass::Storage { .. }),
                                    },
                                );
                                bindings.insert(binding, next_texture_slot);
                                next_texture_slot += 1;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        per_entry_point_bindings.insert(entry.name.clone(), Arc::new(bindings));
        options.per_entry_point_map.insert(entry.name.clone(), map);
    }

    let (code, translation) = naga::back::msl::write_string(
        &module,
        &info,
        &options,
        &naga::back::msl::PipelineOptions {
            allow_and_force_point_size: false,
        },
    )
    .map_err(ShaderCompileError::GenMsl)?;

    eprintln!("{}", code);
    eprintln!("{:?}", translation.entry_point_names);

    Ok(CompiledMetalShader {
        code,
        per_entry_point_bindings,
    })
}
