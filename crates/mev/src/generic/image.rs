use std::{
    error::Error,
    fmt,
    ops::{Mul, Range},
};

use super::{format::PixelFormat, Extent1, Extent2, Extent3, OutOfMemory};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::OutOfMemory => fmt::Display::fmt(&OutOfMemory, f),
            ImageError::InvalidFormat => write!(f, "invalid format"),
        }
    }
}

impl Error for ImageError {}

/// Image component swizzle
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComponentSwizzle {
    Identity,
    Zero,
    One,
    R,
    G,
    B,
    A,
}

impl Default for ComponentSwizzle {
    fn default() -> Self {
        ComponentSwizzle::Identity
    }
}

/// Image swizzle
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Swizzle {
    pub r: ComponentSwizzle,
    pub g: ComponentSwizzle,
    pub b: ComponentSwizzle,
    pub a: ComponentSwizzle,
}

impl Swizzle {
    pub const IDENTITY: Self = Swizzle {
        r: ComponentSwizzle::Identity,
        g: ComponentSwizzle::Identity,
        b: ComponentSwizzle::Identity,
        a: ComponentSwizzle::Identity,
    };

    pub const RRRR: Self = Swizzle {
        r: ComponentSwizzle::R,
        g: ComponentSwizzle::R,
        b: ComponentSwizzle::R,
        a: ComponentSwizzle::R,
    };

    pub const _111R: Self = Swizzle {
        r: ComponentSwizzle::One,
        g: ComponentSwizzle::One,
        b: ComponentSwizzle::One,
        a: ComponentSwizzle::R,
    };
}

impl Mul for Swizzle {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        use ComponentSwizzle::*;

        let mul = |rhs: ComponentSwizzle, i| match rhs {
            Identity => i,
            Zero => Zero,
            One => One,
            R => self.r,
            G => self.g,
            B => self.b,
            A => self.a,
        };

        let r = mul(rhs.r, self.r);
        let g = mul(rhs.g, self.g);
        let b = mul(rhs.b, self.b);
        let a = mul(rhs.a, self.a);

        Swizzle { r, g, b, a }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageDimensions {
    D1(u32),
    D2(u32, u32),
    D3(u32, u32, u32),
}

impl ImageDimensions {
    #[inline(always)]
    pub fn width(&self) -> u32 {
        match self {
            ImageDimensions::D1(width) => *width,
            ImageDimensions::D2(width, _) => *width,
            ImageDimensions::D3(width, _, _) => *width,
        }
    }

    #[inline(always)]
    pub fn height(&self) -> u32 {
        match self {
            ImageDimensions::D1(_) => 1,
            ImageDimensions::D2(_, height) => *height,
            ImageDimensions::D3(_, height, _) => *height,
        }
    }

    #[inline(always)]
    pub fn depth(&self) -> u32 {
        match self {
            ImageDimensions::D1(_) => 1,
            ImageDimensions::D2(_, _) => 1,
            ImageDimensions::D3(_, _, depth) => *depth,
        }
    }

    #[inline(always)]
    pub fn to_1d(self) -> Extent1<u32> {
        match self {
            ImageDimensions::D1(width) => Extent1::new(width),
            ImageDimensions::D2(width, _) => Extent1::new(width),
            ImageDimensions::D3(width, _, _) => Extent1::new(width),
        }
    }

    #[inline(always)]
    pub fn to_2d(self) -> Extent2<u32> {
        match self {
            ImageDimensions::D1(width) => Extent2::new(width, 1),
            ImageDimensions::D2(width, height) => Extent2::new(width, height),
            ImageDimensions::D3(width, height, _) => Extent2::new(width, height),
        }
    }

    #[inline(always)]
    pub fn to_3d(self) -> Extent3<u32> {
        match self {
            ImageDimensions::D1(width) => Extent3::new(width, 1, 1),
            ImageDimensions::D2(width, height) => Extent3::new(width, height, 1),
            ImageDimensions::D3(width, height, depth) => Extent3::new(width, height, depth),
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewDesc {
    pub format: PixelFormat,
    pub base_layer: u32,
    pub layers: u32,
    pub base_level: u32,
    pub levels: u32,
    pub swizzle: Swizzle,
}

impl ViewDesc {
    pub fn new(format: PixelFormat) -> Self {
        ViewDesc {
            format,
            base_layer: 0,
            layers: 1,
            base_level: 0,
            levels: 1,
            swizzle: Swizzle::IDENTITY,
        }
    }

    pub fn layers(self, range: Range<u32>) -> Self {
        Self {
            layers: range.end - range.start,
            base_layer: range.start,
            ..self
        }
    }

    pub fn levels(self, range: Range<u32>) -> Self {
        Self {
            levels: range.end - range.start,
            base_level: range.start,
            ..self
        }
    }

    pub fn swizzle(self, swizzle: Swizzle) -> Self {
        Self { swizzle, ..self }
    }
}
