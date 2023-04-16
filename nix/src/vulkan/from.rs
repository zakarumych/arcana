use ash::vk;

use crate::generic::{
    BlendFactor, BlendOp, BufferUsage, CompareFunction, FamilyCapabilities, ImageDimensions,
    ImageUsage, PixelFormat, QueueFlags, VertexFormat, WriteMask,
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

pub trait TryFromAsh<T>: Sized {
    fn try_from_ash(t: T) -> Option<Self>;
}

pub trait TryAshInto<T> {
    fn try_ash_into(self) -> Option<T>;
}

impl<T, U> TryAshInto<U> for T
where
    U: TryFromAsh<T>,
{
    #[inline(always)]
    fn try_ash_into(self) -> Option<U> {
        U::try_from_ash(self)
    }
}

pub trait TryAshFrom<T>: Sized {
    fn try_ash_from(t: T) -> Option<Self>;
}

pub trait TryIntoAsh<T> {
    fn try_into_ash(self) -> Option<T>;
}

impl<T, U> TryIntoAsh<U> for T
where
    U: TryAshFrom<T>,
{
    #[inline(always)]
    fn try_into_ash(self) -> Option<U> {
        U::try_ash_from(self)
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
    #[inline(always)]
    fn ash_from(value: ImageDimensions) -> Self {
        match value {
            ImageDimensions::D1(_) => vk::ImageType::TYPE_1D,
            ImageDimensions::D2(_, _) => vk::ImageType::TYPE_2D,
            ImageDimensions::D3(_, _, _) => vk::ImageType::TYPE_3D,
        }
    }
}

impl TryAshFrom<PixelFormat> for vk::Format {
    #[inline(always)]
    fn try_ash_from(value: PixelFormat) -> Option<Self> {
        Some(match value {
            PixelFormat::R8Unorm => vk::Format::R8_UNORM,
            PixelFormat::R8Snorm => vk::Format::R8_SNORM,
            PixelFormat::R8Uint => vk::Format::R8_UINT,
            PixelFormat::R8Sint => vk::Format::R8_SINT,
            PixelFormat::R8Srgb => vk::Format::R8_SRGB,
            PixelFormat::R16Unorm => vk::Format::R16_UNORM,
            PixelFormat::R16Snorm => vk::Format::R16_SNORM,
            PixelFormat::R16Uint => vk::Format::R16_UINT,
            PixelFormat::R16Sint => vk::Format::R16_SINT,
            PixelFormat::R16Float => vk::Format::R16_SFLOAT,
            // PixelFormat::R32Unorm => vk::Format::R32_UNORM,
            // PixelFormat::R32Snorm => vk::Format::R32_SNORM,
            PixelFormat::R32Uint => vk::Format::R32_UINT,
            PixelFormat::R32Sint => vk::Format::R32_SINT,
            PixelFormat::R32Float => vk::Format::R32_SFLOAT,
            PixelFormat::Rg8Unorm => vk::Format::R8G8_UNORM,
            PixelFormat::Rg8Snorm => vk::Format::R8G8_SNORM,
            PixelFormat::Rg8Uint => vk::Format::R8G8_UINT,
            PixelFormat::Rg8Sint => vk::Format::R8G8_SINT,
            PixelFormat::Rg8Srgb => vk::Format::R8G8_SRGB,
            PixelFormat::Rg16Unorm => vk::Format::R16G16_UNORM,
            PixelFormat::Rg16Snorm => vk::Format::R16G16_SNORM,
            PixelFormat::Rg16Uint => vk::Format::R16G16_UINT,
            PixelFormat::Rg16Sint => vk::Format::R16G16_SINT,
            PixelFormat::Rg16Float => vk::Format::R16G16_SFLOAT,
            // PixelFormat::Rg32Unorm => vk::Format::R32G32_UNORM,
            // PixelFormat::Rg32Snorm => vk::Format::R32G32_SNORM,
            PixelFormat::Rg32Uint => vk::Format::R32G32_UINT,
            PixelFormat::Rg32Sint => vk::Format::R32G32_SINT,
            PixelFormat::Rg32Float => vk::Format::R32G32_SFLOAT,
            PixelFormat::Rgb8Unorm => vk::Format::R8G8B8_UNORM,
            PixelFormat::Rgb8Snorm => vk::Format::R8G8B8_SNORM,
            PixelFormat::Rgb8Uint => vk::Format::R8G8B8_UINT,
            PixelFormat::Rgb8Sint => vk::Format::R8G8B8_SINT,
            PixelFormat::Rgb8Srgb => vk::Format::R8G8B8_SRGB,
            PixelFormat::Rgb16Unorm => vk::Format::R16G16B16_UNORM,
            PixelFormat::Rgb16Snorm => vk::Format::R16G16B16_SNORM,
            PixelFormat::Rgb16Uint => vk::Format::R16G16B16_UINT,
            PixelFormat::Rgb16Sint => vk::Format::R16G16B16_SINT,
            PixelFormat::Rgb16Float => vk::Format::R16G16B16_SFLOAT,
            // PixelFormat::Rgb32Unorm => vk::Format::R32G32B32_UNORM,
            // PixelFormat::Rgb32Snorm => vk::Format::R32G32B32_SNORM,
            PixelFormat::Rgb32Uint => vk::Format::R32G32B32_UINT,
            PixelFormat::Rgb32Sint => vk::Format::R32G32B32_SINT,
            PixelFormat::Rgb32Float => vk::Format::R32G32B32_SFLOAT,
            PixelFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
            PixelFormat::Rgba8Snorm => vk::Format::R8G8B8A8_SNORM,
            PixelFormat::Rgba8Uint => vk::Format::R8G8B8A8_UINT,
            PixelFormat::Rgba8Sint => vk::Format::R8G8B8A8_SINT,
            PixelFormat::Rgba8Srgb => vk::Format::R8G8B8A8_SRGB,
            PixelFormat::Rgba16Unorm => vk::Format::R16G16B16A16_UNORM,
            PixelFormat::Rgba16Snorm => vk::Format::R16G16B16A16_SNORM,
            PixelFormat::Rgba16Uint => vk::Format::R16G16B16A16_UINT,
            PixelFormat::Rgba16Sint => vk::Format::R16G16B16A16_SINT,
            PixelFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
            // PixelFormat::Rgba32Unorm => vk::Format::R32G32B32A32_UNORM,
            // PixelFormat::Rgba32Snorm => vk::Format::R32G32B32A32_SNORM,
            PixelFormat::Rgba32Uint => vk::Format::R32G32B32A32_UINT,
            PixelFormat::Rgba32Sint => vk::Format::R32G32B32A32_SINT,
            PixelFormat::Rgba32Float => vk::Format::R32G32B32A32_SFLOAT,
            PixelFormat::Bgr8Unorm => vk::Format::B8G8R8_UNORM,
            PixelFormat::Bgr8Snorm => vk::Format::B8G8R8_SNORM,
            PixelFormat::Bgr8Uint => vk::Format::B8G8R8_UINT,
            PixelFormat::Bgr8Sint => vk::Format::B8G8R8_SINT,
            PixelFormat::Bgr8Srgb => vk::Format::B8G8R8_SRGB,
            PixelFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
            PixelFormat::Bgra8Snorm => vk::Format::B8G8R8A8_SNORM,
            PixelFormat::Bgra8Uint => vk::Format::B8G8R8A8_UINT,
            PixelFormat::Bgra8Sint => vk::Format::B8G8R8A8_SINT,
            PixelFormat::Bgra8Srgb => vk::Format::B8G8R8A8_SRGB,
            PixelFormat::D16Unorm => vk::Format::D16_UNORM,
            PixelFormat::D32Float => vk::Format::D32_SFLOAT,
            PixelFormat::S8Uint => vk::Format::S8_UINT,
            PixelFormat::D16UnormS8Uint => vk::Format::D16_UNORM_S8_UINT,
            PixelFormat::D24UnormS8Uint => vk::Format::D24_UNORM_S8_UINT,
            PixelFormat::D32FloatS8Uint => vk::Format::D32_SFLOAT_S8_UINT,
            _ => return None,
        })
    }
}

impl TryFromAsh<vk::Format> for PixelFormat {
    #[inline(always)]
    fn try_from_ash(value: vk::Format) -> Option<Self> {
        Some(match value {
            vk::Format::R8_UNORM => PixelFormat::R8Unorm,
            vk::Format::R8_SNORM => PixelFormat::R8Snorm,
            vk::Format::R8_UINT => PixelFormat::R8Uint,
            vk::Format::R8_SINT => PixelFormat::R8Sint,
            vk::Format::R8_SRGB => PixelFormat::R8Srgb,
            vk::Format::R16_UNORM => PixelFormat::R16Unorm,
            vk::Format::R16_SNORM => PixelFormat::R16Snorm,
            vk::Format::R16_UINT => PixelFormat::R16Uint,
            vk::Format::R16_SINT => PixelFormat::R16Sint,
            vk::Format::R16_SFLOAT => PixelFormat::R16Float,
            // vk::Format::R32_UNORM => PixelFormat::R32Unorm,
            // vk::Format::R32_SNORM => PixelFormat::R32Snorm,
            vk::Format::R32_UINT => PixelFormat::R32Uint,
            vk::Format::R32_SINT => PixelFormat::R32Sint,
            vk::Format::R32_SFLOAT => PixelFormat::R32Float,
            vk::Format::R8G8_UNORM => PixelFormat::Rg8Unorm,
            vk::Format::R8G8_SNORM => PixelFormat::Rg8Snorm,
            vk::Format::R8G8_UINT => PixelFormat::Rg8Uint,
            vk::Format::R8G8_SINT => PixelFormat::Rg8Sint,
            vk::Format::R8G8_SRGB => PixelFormat::Rg8Srgb,
            vk::Format::R16G16_UNORM => PixelFormat::Rg16Unorm,
            vk::Format::R16G16_SNORM => PixelFormat::Rg16Snorm,
            vk::Format::R16G16_UINT => PixelFormat::Rg16Uint,
            vk::Format::R16G16_SINT => PixelFormat::Rg16Sint,
            vk::Format::R16G16_SFLOAT => PixelFormat::Rg16Float,
            // vk::Format::R32G32_UNORM => PixelFormat::Rg32Unorm,
            // vk::Format::R32G32_SNORM => PixelFormat::Rg32Snorm,
            vk::Format::R32G32_UINT => PixelFormat::Rg32Uint,
            vk::Format::R32G32_SINT => PixelFormat::Rg32Sint,
            vk::Format::R32G32_SFLOAT => PixelFormat::Rg32Float,
            vk::Format::R8G8B8_UNORM => PixelFormat::Rgb8Unorm,
            vk::Format::R8G8B8_SNORM => PixelFormat::Rgb8Snorm,
            vk::Format::R8G8B8_UINT => PixelFormat::Rgb8Uint,
            vk::Format::R8G8B8_SINT => PixelFormat::Rgb8Sint,
            vk::Format::R8G8B8_SRGB => PixelFormat::Rgb8Srgb,
            vk::Format::R16G16B16_UNORM => PixelFormat::Rgb16Unorm,
            vk::Format::R16G16B16_SNORM => PixelFormat::Rgb16Snorm,
            vk::Format::R16G16B16_UINT => PixelFormat::Rgb16Uint,
            vk::Format::R16G16B16_SINT => PixelFormat::Rgb16Sint,
            vk::Format::R16G16B16_SFLOAT => PixelFormat::Rgb16Float,
            // vk::Format::R32G32B32_UNORM => PixelFormat::Rgb32Unorm,
            // vk::Format::R32G32B32_SNORM => PixelFormat::Rgb32Snorm,
            vk::Format::R32G32B32_UINT => PixelFormat::Rgb32Uint,
            vk::Format::R32G32B32_SINT => PixelFormat::Rgb32Sint,
            vk::Format::R32G32B32_SFLOAT => PixelFormat::Rgb32Float,
            vk::Format::R8G8B8A8_UNORM => PixelFormat::Rgba8Unorm,
            vk::Format::R8G8B8A8_SNORM => PixelFormat::Rgba8Snorm,
            vk::Format::R8G8B8A8_UINT => PixelFormat::Rgba8Uint,
            vk::Format::R8G8B8A8_SINT => PixelFormat::Rgba8Sint,
            vk::Format::R8G8B8A8_SRGB => PixelFormat::Rgba8Srgb,
            vk::Format::R16G16B16A16_UNORM => PixelFormat::Rgba16Unorm,
            vk::Format::R16G16B16A16_SNORM => PixelFormat::Rgba16Snorm,
            vk::Format::R16G16B16A16_UINT => PixelFormat::Rgba16Uint,
            vk::Format::R16G16B16A16_SINT => PixelFormat::Rgba16Sint,
            vk::Format::R16G16B16A16_SFLOAT => PixelFormat::Rgba16Float,
            // vk::Format::R32G32B32A32_UNORM => PixelFormat::Rgba32Unorm,
            // vk::Format::R32G32B32A32_SNORM => PixelFormat::Rgba32Snorm,
            vk::Format::R32G32B32A32_UINT => PixelFormat::Rgba32Uint,
            vk::Format::R32G32B32A32_SINT => PixelFormat::Rgba32Sint,
            vk::Format::R32G32B32A32_SFLOAT => PixelFormat::Rgba32Float,
            vk::Format::B8G8R8_UNORM => PixelFormat::Bgr8Unorm,
            vk::Format::B8G8R8_SNORM => PixelFormat::Bgr8Snorm,
            vk::Format::B8G8R8_UINT => PixelFormat::Bgr8Uint,
            vk::Format::B8G8R8_SINT => PixelFormat::Bgr8Sint,
            vk::Format::B8G8R8_SRGB => PixelFormat::Bgr8Srgb,
            vk::Format::B8G8R8A8_UNORM => PixelFormat::Bgra8Unorm,
            vk::Format::B8G8R8A8_SNORM => PixelFormat::Bgra8Snorm,
            vk::Format::B8G8R8A8_UINT => PixelFormat::Bgra8Uint,
            vk::Format::B8G8R8A8_SINT => PixelFormat::Bgra8Sint,
            vk::Format::B8G8R8A8_SRGB => PixelFormat::Bgra8Srgb,
            vk::Format::D16_UNORM => PixelFormat::D16Unorm,
            vk::Format::D32_SFLOAT => PixelFormat::D32Float,
            vk::Format::S8_UINT => PixelFormat::S8Uint,
            vk::Format::D16_UNORM_S8_UINT => PixelFormat::D16UnormS8Uint,
            vk::Format::D24_UNORM_S8_UINT => PixelFormat::D24UnormS8Uint,
            vk::Format::D32_SFLOAT_S8_UINT => PixelFormat::D32FloatS8Uint,
            _ => return None,
        })
    }
}

impl AshFrom<(ImageUsage, PixelFormat)> for vk::ImageUsageFlags {
    #[inline(always)]
    fn ash_from((usage, format): (ImageUsage, PixelFormat)) -> Self {
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

impl FromAsh<vk::ImageUsageFlags> for ImageUsage {
    #[inline(always)]
    fn from_ash(usage: vk::ImageUsageFlags) -> Self {
        let mut result = ImageUsage::empty();
        if usage.contains(vk::ImageUsageFlags::TRANSFER_SRC) {
            result |= ImageUsage::TRANSFER_SRC;
        }
        if usage.contains(vk::ImageUsageFlags::TRANSFER_DST) {
            result |= ImageUsage::TRANSFER_DST;
        }
        if usage.contains(vk::ImageUsageFlags::SAMPLED) {
            result |= ImageUsage::SAMPLED;
        }
        if usage.contains(vk::ImageUsageFlags::STORAGE) {
            result |= ImageUsage::STORAGE;
        }
        if usage.intersects(
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        ) {
            result |= ImageUsage::TARGET;
        }
        result
    }
}

impl TryAshFrom<VertexFormat> for vk::Format {
    #[inline(always)]
    fn try_ash_from(value: VertexFormat) -> Option<Self> {
        Some(match value {
            VertexFormat::Uint8 => vk::Format::R8_UINT,
            VertexFormat::Uint16 => vk::Format::R16_UINT,
            VertexFormat::Uint32 => vk::Format::R32_UINT,
            VertexFormat::Sint8 => vk::Format::R8_SINT,
            VertexFormat::Sint16 => vk::Format::R16_SINT,
            VertexFormat::Sint32 => vk::Format::R32_SINT,
            VertexFormat::Unorm8 => vk::Format::R8_UNORM,
            VertexFormat::Unorm16 => vk::Format::R16_UNORM,
            // VertexFormat::Unorm32 => vk::Format::R32_UNORM,
            VertexFormat::Snorm8 => vk::Format::R8_SNORM,
            VertexFormat::Snorm16 => vk::Format::R16_SNORM,
            // VertexFormat::Snorm32 => vk::Format::R32_SNORM,
            VertexFormat::Float16 => vk::Format::R16_SFLOAT,
            VertexFormat::Float32 => vk::Format::R32_SFLOAT,
            VertexFormat::Uint8x2 => vk::Format::R8G8_UINT,
            VertexFormat::Uint16x2 => vk::Format::R16G16_UINT,
            VertexFormat::Uint32x2 => vk::Format::R32G32_UINT,
            VertexFormat::Sint8x2 => vk::Format::R8G8_SINT,
            VertexFormat::Sint16x2 => vk::Format::R16G16_SINT,
            VertexFormat::Sint32x2 => vk::Format::R32G32_SINT,
            VertexFormat::Unorm8x2 => vk::Format::R8G8_UNORM,
            VertexFormat::Unorm16x2 => vk::Format::R16G16_UNORM,
            // VertexFormat::Unorm32x2 => vk::Format::R32G32_UNORM,
            VertexFormat::Snorm8x2 => vk::Format::R8G8_SNORM,
            VertexFormat::Snorm16x2 => vk::Format::R16G16_SNORM,
            // VertexFormat::Snorm32x2 => vk::Format::R32G32_SNORM,
            VertexFormat::Float16x2 => vk::Format::R16G16_SFLOAT,
            VertexFormat::Float32x2 => vk::Format::R32G32_SFLOAT,
            VertexFormat::Uint8x3 => vk::Format::R8G8B8_UINT,
            VertexFormat::Uint16x3 => vk::Format::R16G16B16_UINT,
            VertexFormat::Uint32x3 => vk::Format::R32G32B32_UINT,
            VertexFormat::Sint8x3 => vk::Format::R8G8B8_SINT,
            VertexFormat::Sint16x3 => vk::Format::R16G16B16_SINT,
            VertexFormat::Sint32x3 => vk::Format::R32G32B32_SINT,
            VertexFormat::Unorm8x3 => vk::Format::R8G8B8_UNORM,
            VertexFormat::Unorm16x3 => vk::Format::R16G16B16_UNORM,
            // VertexFormat::Unorm32x3 => vk::Format::R32G32B32_UNORM,
            VertexFormat::Snorm8x3 => vk::Format::R8G8B8_SNORM,
            VertexFormat::Snorm16x3 => vk::Format::R16G16B16_SNORM,
            // VertexFormat::Snorm32x3 => vk::Format::R32G32B32_SNORM,
            VertexFormat::Float16x3 => vk::Format::R16G16B16_SFLOAT,
            VertexFormat::Float32x3 => vk::Format::R32G32B32_SFLOAT,
            VertexFormat::Uint8x4 => vk::Format::R8G8B8A8_UINT,
            VertexFormat::Uint16x4 => vk::Format::R16G16B16A16_UINT,
            VertexFormat::Uint32x4 => vk::Format::R32G32B32A32_UINT,
            VertexFormat::Sint8x4 => vk::Format::R8G8B8A8_SINT,
            VertexFormat::Sint16x4 => vk::Format::R16G16B16A16_SINT,
            VertexFormat::Sint32x4 => vk::Format::R32G32B32A32_SINT,
            VertexFormat::Unorm8x4 => vk::Format::R8G8B8A8_UNORM,
            VertexFormat::Unorm16x4 => vk::Format::R16G16B16A16_UNORM,
            // VertexFormat::Unorm32x4 => vk::Format::R32G32B32A32_UNORM,
            VertexFormat::Snorm8x4 => vk::Format::R8G8B8A8_SNORM,
            VertexFormat::Snorm16x4 => vk::Format::R16G16B16A16_SNORM,
            // VertexFormat::Snorm32x4 => vk::Format::R32G32B32A32_SNORM,
            VertexFormat::Float16x4 => vk::Format::R16G16B16A16_SFLOAT,
            VertexFormat::Float32x4 => vk::Format::R32G32B32A32_SFLOAT,
            _ => return None,
        })
    }
}

impl AshFrom<CompareFunction> for vk::CompareOp {
    #[inline(always)]
    fn ash_from(compare: CompareFunction) -> Self {
        match compare {
            CompareFunction::Never => vk::CompareOp::NEVER,
            CompareFunction::Less => vk::CompareOp::LESS,
            CompareFunction::Equal => vk::CompareOp::EQUAL,
            CompareFunction::LessEqual => vk::CompareOp::LESS_OR_EQUAL,
            CompareFunction::Greater => vk::CompareOp::GREATER,
            CompareFunction::NotEqual => vk::CompareOp::NOT_EQUAL,
            CompareFunction::GreaterEqual => vk::CompareOp::GREATER_OR_EQUAL,
            CompareFunction::Always => vk::CompareOp::ALWAYS,
        }
    }
}

impl AshFrom<BlendFactor> for vk::BlendFactor {
    #[inline(always)]
    fn ash_from(factor: BlendFactor) -> Self {
        match factor {
            BlendFactor::Zero => vk::BlendFactor::ZERO,
            BlendFactor::One => vk::BlendFactor::ONE,
            BlendFactor::SrcColor => vk::BlendFactor::SRC_COLOR,
            BlendFactor::OneMinusSrcColor => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
            BlendFactor::SrcAlpha => vk::BlendFactor::SRC_ALPHA,
            BlendFactor::OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            BlendFactor::DstColor => vk::BlendFactor::DST_COLOR,
            BlendFactor::OneMinusDstColor => vk::BlendFactor::ONE_MINUS_DST_COLOR,
            BlendFactor::DstAlpha => vk::BlendFactor::DST_ALPHA,
            BlendFactor::OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
            BlendFactor::SrcAlphaSaturated => vk::BlendFactor::SRC_ALPHA_SATURATE,
            BlendFactor::BlendColor => vk::BlendFactor::CONSTANT_COLOR,
            BlendFactor::OneMinusBlendColor => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
        }
    }
}

impl AshFrom<BlendOp> for vk::BlendOp {
    #[inline(always)]
    fn ash_from(op: BlendOp) -> Self {
        match op {
            BlendOp::Add => vk::BlendOp::ADD,
            BlendOp::Subtract => vk::BlendOp::SUBTRACT,
            BlendOp::ReverseSubtract => vk::BlendOp::REVERSE_SUBTRACT,
            BlendOp::Min => vk::BlendOp::MIN,
            BlendOp::Max => vk::BlendOp::MAX,
        }
    }
}

impl AshFrom<WriteMask> for vk::ColorComponentFlags {
    #[inline(always)]
    fn ash_from(mask: WriteMask) -> Self {
        let mut flags = vk::ColorComponentFlags::empty();
        if mask.contains(WriteMask::RED) {
            flags |= vk::ColorComponentFlags::R;
        }
        if mask.contains(WriteMask::GREEN) {
            flags |= vk::ColorComponentFlags::G;
        }
        if mask.contains(WriteMask::BLUE) {
            flags |= vk::ColorComponentFlags::B;
        }
        if mask.contains(WriteMask::ALPHA) {
            flags |= vk::ColorComponentFlags::A;
        }
        flags
    }
}
