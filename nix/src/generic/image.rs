use super::{format::PixelFormat, OutOfMemory};

pub enum ImageError {
    OutOfMemory,
    InvalidFormat,
}

impl From<OutOfMemory> for ImageError {
    #[inline(always)]
    fn from(_: OutOfMemory) -> Self {
        ImageError::OutOfMemory
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageDimensions {
    D1(u32),
    D2(u32, u32),
    D3(u32, u32, u32),
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ImageUsage: u32 {
        const TRANSFER_SRC = 0x0000_0001;
        const TRANSFER_DST = 0x0000_0002;
        const SAMPLED = 0x0000_0004;
        const STORAGE = 0x0000_0008;
        const TARGET = 0x0000_0010;
    }
}

pub struct ImageDesc<'a> {
    pub dimensions: ImageDimensions,
    pub format: PixelFormat,
    pub usage: ImageUsage,
    pub layers: u32,
    pub levels: u32,

    /// Image debug name.
    pub name: &'a str,
}

impl<'a> ImageDesc<'a> {
    pub const fn new(dimensions: ImageDimensions, format: PixelFormat, usage: ImageUsage) -> Self {
        ImageDesc {
            dimensions,
            format,
            usage,
            layers: 1,
            levels: 1,
            name: "",
        }
    }

    pub const fn new_d1(width: u32, format: PixelFormat, usage: ImageUsage) -> Self {
        ImageDesc::new(ImageDimensions::D1(width), format, usage)
    }

    pub const fn new_d2(width: u32, height: u32, format: PixelFormat, usage: ImageUsage) -> Self {
        ImageDesc::new(ImageDimensions::D2(width, height), format, usage)
    }

    pub const fn new_d3(
        width: u32,
        height: u32,
        depth: u32,
        format: PixelFormat,
        usage: ImageUsage,
    ) -> Self {
        ImageDesc::new(ImageDimensions::D3(width, height, depth), format, usage)
    }

    pub fn layers(mut self, layers: u32) -> Self {
        self.layers = layers;
        self
    }

    pub fn levels(mut self, levels: u32) -> Self {
        self.levels = levels;
        self
    }

    pub const fn new_d1_texture(width: u32, format: PixelFormat) -> Self {
        ImageDesc::new_d1(
            width,
            format,
            ImageUsage::union(ImageUsage::SAMPLED, ImageUsage::TRANSFER_DST),
        )
    }

    pub const fn new_d2_texture(width: u32, height: u32, format: PixelFormat) -> Self {
        ImageDesc::new_d2(
            width,
            height,
            format,
            ImageUsage::union(ImageUsage::SAMPLED, ImageUsage::TRANSFER_DST),
        )
    }

    pub const fn new_d3_texture(width: u32, height: u32, depth: u32, format: PixelFormat) -> Self {
        ImageDesc::new_d3(
            width,
            height,
            depth,
            format,
            ImageUsage::union(ImageUsage::SAMPLED, ImageUsage::TRANSFER_DST),
        )
    }

    pub const fn new_d1_rtt(width: u32, format: PixelFormat) -> Self {
        ImageDesc::new_d1(
            width,
            format,
            ImageUsage::union(ImageUsage::SAMPLED, ImageUsage::TARGET),
        )
    }

    pub const fn new_d2_rtt(width: u32, height: u32, format: PixelFormat) -> Self {
        ImageDesc::new_d2(
            width,
            height,
            format,
            ImageUsage::union(ImageUsage::SAMPLED, ImageUsage::TARGET),
        )
    }

    pub const fn new_d3_rtt(width: u32, height: u32, depth: u32, format: PixelFormat) -> Self {
        ImageDesc::new_d3(
            width,
            height,
            depth,
            format,
            ImageUsage::union(ImageUsage::SAMPLED, ImageUsage::TARGET),
        )
    }

    pub const fn with_name(mut self, name: &'a str) -> Self {
        self.name = name;
        self
    }
}
