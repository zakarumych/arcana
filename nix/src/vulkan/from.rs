use ash::vk;

use crate::generic::{
    BufferUsage, FamilyCapabilities, Format, ImageDimensions, ImageUsage, QueueFlags,
};

macro_rules! from_flags {
    ($from:ty => $to:ty, [$($from_flag:ident),* $(,)?], $flags:expr) => {
        from_flags!($from => $to, [$($from_flag => $from_flag,)*], $flags)
    };
    ($from:ty => $to:ty, [$($from_flag:ident => $to_flag:ident),* $(,)?], $flags:expr) => {{
        let mut dst = <$to>::empty();
        let mut src = $flags;
        $(
            if src.contains(<$from>::$from_flag) {
                dst |= <$to>::$to_flag;
            }
        )*
        dst
    }};
}

pub trait FromAsh<A> {
    fn from_ash(ash: A) -> Self;
}

pub trait AshInto<T> {
    fn ash_into(self) -> T;
}

impl<T, A> AshInto<T> for A
where
    T: FromAsh<A>,
{
    #[inline(always)]
    fn ash_into(self) -> T {
        T::from_ash(self)
    }
}

pub trait AshFrom<T> {
    fn ash_from(ash: T) -> Self;
}

pub trait IntoAsh<A> {
    fn into_ash(self) -> A;
}

impl<A, T> IntoAsh<A> for T
where
    A: AshFrom<T>,
{
    #[inline(always)]
    fn into_ash(self) -> A {
        A::ash_from(self)
    }
}

impl FromAsh<vk::QueueFamilyProperties> for FamilyCapabilities {
    #[inline(always)]
    fn from_ash(value: vk::QueueFamilyProperties) -> Self {
        FamilyCapabilities {
            queue_flags: value.queue_flags.ash_into(),
            queue_count: value.queue_count.try_into().unwrap_or(usize::MAX), // Saturate is OK.
        }
    }
}

impl FromAsh<vk::QueueFamilyProperties2> for FamilyCapabilities {
    #[inline(always)]
    fn from_ash(value: vk::QueueFamilyProperties2) -> Self {
        value.queue_family_properties.ash_into()
    }
}

impl FromAsh<vk::QueueFlags> for QueueFlags {
    #[inline(always)]
    fn from_ash(value: vk::QueueFlags) -> Self {
        // from_flags!(vk::QueueFlags => QueueFlags, [GRAPHICS, COMPUTE, TRANSFER], value)

        let mut result = QueueFlags::empty();
        if value.contains(vk::QueueFlags::GRAPHICS) {
            result |= QueueFlags::GRAPHICS | QueueFlags::TRANSFER;
        } else if value.contains(vk::QueueFlags::COMPUTE) {
            result |= QueueFlags::COMPUTE | QueueFlags::TRANSFER;
        } else if value.contains(vk::QueueFlags::TRANSFER) {
            result |= QueueFlags::TRANSFER;
        }
        result
    }
}

impl AshFrom<BufferUsage> for vk::BufferUsageFlags {
    #[inline(always)]
    fn ash_from(value: BufferUsage) -> Self {
        from_flags!(BufferUsage => vk::BufferUsageFlags, [
            TRANSFER_SRC => TRANSFER_SRC,
            TRANSFER_DST => TRANSFER_DST,
            UNIFORM => UNIFORM_BUFFER,
            STORAGE => STORAGE_BUFFER,
            INDEX => INDEX_BUFFER,
            VERTEX => VERTEX_BUFFER,
            INDIRECT => INDIRECT_BUFFER,
        ], value)
    }
}

impl AshFrom<ImageDimensions> for vk::ImageType {
    fn ash_from(value: ImageDimensions) -> Self {
        match value {
            ImageDimensions::D1(_) => vk::ImageType::TYPE_1D,
            ImageDimensions::D2(_, _) => vk::ImageType::TYPE_2D,
            ImageDimensions::D3(_, _, _) => vk::ImageType::TYPE_3D,
        }
    }
}

impl AshFrom<Format> for vk::Format {
    fn ash_from(value: Format) -> Self {
        match value {
            Format::R8Unorm => vk::Format::R8_UNORM,
            Format::R8Snorm => vk::Format::R8_SNORM,
            Format::R8Uint => vk::Format::R8_UINT,
            Format::R8Sint => vk::Format::R8_SINT,
            Format::R16Uint => vk::Format::R16_UINT,
            Format::R16Sint => vk::Format::R16_SINT,
            Format::R16Float => vk::Format::R16_SFLOAT,
            Format::R32Uint => vk::Format::R32_UINT,
            Format::R32Sint => vk::Format::R32_SINT,
            Format::R32Float => vk::Format::R32_SFLOAT,
            Format::Rg8Unorm => vk::Format::R8G8_UNORM,
            Format::Rg8Snorm => vk::Format::R8G8_SNORM,
            Format::Rg8Uint => vk::Format::R8G8_UINT,
            Format::Rg8Sint => vk::Format::R8G8_SINT,
            Format::Rg16Uint => vk::Format::R16G16_UINT,
            Format::Rg16Sint => vk::Format::R16G16_SINT,
            Format::Rg16Float => vk::Format::R16G16_SFLOAT,
            Format::Rg32Uint => vk::Format::R32G32_UINT,
            Format::Rg32Sint => vk::Format::R32G32_SINT,
            Format::Rg32Float => vk::Format::R32G32_SFLOAT,
            Format::Rgb16Uint => vk::Format::R16G16B16_UINT,
            Format::Rgb16Sint => vk::Format::R16G16B16_SINT,
            Format::Rgb16Float => vk::Format::R16G16B16_SFLOAT,
            Format::Rgb32Uint => vk::Format::R32G32B32_UINT,
            Format::Rgb32Sint => vk::Format::R32G32B32_SINT,
            Format::Rgb32Float => vk::Format::R32G32B32_SFLOAT,
            Format::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
            Format::Rgba8UnormSrgb => vk::Format::R8G8B8A8_SRGB,
            Format::Rgba8Snorm => vk::Format::R8G8B8A8_SNORM,
            Format::Rgba8Uint => vk::Format::R8G8B8A8_UINT,
            Format::Rgba8Sint => vk::Format::R8G8B8A8_SINT,
            Format::Rgba16Uint => vk::Format::R16G16B16A16_UINT,
            Format::Rgba16Sint => vk::Format::R16G16B16A16_SINT,
            Format::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
            Format::Rgba32Uint => vk::Format::R32G32B32A32_UINT,
            Format::Rgba32Sint => vk::Format::R32G32B32A32_SINT,
            Format::Rgba32Float => vk::Format::R32G32B32A32_SFLOAT,
            Format::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
            Format::Bgra8UnormSrgb => vk::Format::B8G8R8A8_SRGB,
            Format::D16Unorm => vk::Format::D16_UNORM,
            Format::D32Float => vk::Format::D32_SFLOAT,
            Format::S8Uint => vk::Format::S8_UINT,
            Format::D16UnormS8Uint => vk::Format::D16_UNORM_S8_UINT,
            Format::D24UnormS8Uint => vk::Format::D24_UNORM_S8_UINT,
            Format::D32FloatS8Uint => vk::Format::D32_SFLOAT_S8_UINT,
        }
    }
}

impl AshFrom<(ImageUsage, Format)> for vk::ImageUsageFlags {
    fn ash_from((usage, format): (ImageUsage, Format)) -> Self {
        let mut result = vk::ImageUsageFlags::empty();
        if usage.contains(ImageUsage::TRANSFER_SRC) {
            result |= vk::ImageUsageFlags::TRANSFER_SRC;
        }
        if usage.contains(ImageUsage::TRANSFER_DST) {
            result |= vk::ImageUsageFlags::TRANSFER_DST;
        }
        if usage.contains(ImageUsage::SAMPLED) {
            result |= vk::ImageUsageFlags::SAMPLED;
        }
        if usage.contains(ImageUsage::STORAGE) {
            result |= vk::ImageUsageFlags::STORAGE;
        }
        if usage.contains(ImageUsage::TARGET) {
            if format.is_color() {
                result |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
            } else {
                result |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
            }
        }
        result
    }
}
