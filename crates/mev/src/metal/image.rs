use std::ops::Mul;

use metal::MTLTextureType;

use crate::generic::{
    ComponentSwizzle, ImageDimensions, OutOfMemory, PixelFormat, Swizzle, ViewDesc,
};

use super::{
    from::{TryIntoMetal, TryMetalInto},
    Device,
};

#[derive(Clone)]
pub struct Image {
    texture: metal::Texture,
}

unsafe impl Send for Image {}

impl Image {
    pub(super) fn new(texture: metal::Texture) -> Self {
        Image { texture }
    }

    pub(super) fn metal(&self) -> &metal::Texture {
        &self.texture
    }
}

#[hidden_trait::expose]
impl crate::traits::Image for Image {
    fn format(&self) -> PixelFormat {
        self.texture.pixel_format().expect_metal_into()
    }

    fn dimensions(&self) -> ImageDimensions {
        match self.texture.texture_type() {
            MTLTextureType::D1 | MTLTextureType::D1Array => {
                let width = self.texture.width();
                ImageDimensions::D1(width as u32)
            }
            MTLTextureType::D2 | MTLTextureType::D2Array => {
                let width = self.texture.width();
                let height = self.texture.height();
                ImageDimensions::D2(width as u32, height as u32)
            }
            MTLTextureType::D2Multisample => unimplemented!(),
            MTLTextureType::Cube => unimplemented!(),
            MTLTextureType::CubeArray => unimplemented!(),
            MTLTextureType::D3 => {
                let width = self.texture.width();
                let height = self.texture.height();
                let depth = self.texture.depth();
                ImageDimensions::D3(width as u32, height as u32, depth as u32)
            }
        }
    }

    fn layers(&self) -> u32 {
        self.texture.array_length() as u32
    }

    fn levels(&self) -> u32 {
        self.texture.mipmap_level_count() as u32
    }

    fn view(&self, _device: &Device, desc: ViewDesc) -> Result<Image, OutOfMemory> {
        use foreign_types::{ForeignType, ForeignTypeRef};
        use objc::*;

        let pixel_format = desc.format.expect_into_metal();
        let root_texture = self.texture.parent_texture().unwrap_or(&self.texture);

        if desc.swizzle == Swizzle::IDENTITY {
            if desc.base_layer == 0 && desc.base_level == 0 {
                let texture = root_texture.new_texture_view(desc.format.expect_into_metal());
                Ok(Image { texture })
            } else {
                let base_layer = self.texture.parent_relative_slice() as u32 + desc.base_layer;
                let base_level = self.texture.mipmap_level_count() as u32 + desc.base_level;

                let texture = root_texture.new_texture_view_from_slice(
                    pixel_format,
                    self.texture.texture_type(),
                    metal::NSRange::new(base_level.into(), desc.levels.into()),
                    metal::NSRange::new(base_layer.into(), desc.layers.into()),
                );
                Ok(Image { texture })
            }
        } else {
            let base_layer = self.texture.parent_relative_slice() as u32 + desc.base_layer;
            let base_level = self.texture.mipmap_level_count() as u32 + desc.base_level;
            let swizzle: MTLTextureSwizzleChannels =
                unsafe { msg_send![self.texture.as_ptr(), swizzle] };

            let new_swizzle = swizzle * desc.swizzle;

            let texture = unsafe {
                msg_send![root_texture.as_ptr(), newTextureViewWithPixelFormat:pixel_format
                                                textureType:self.texture.texture_type()
                                                levels:metal::NSRange::new(base_level.into(), desc.levels.into())
                                                slices:metal::NSRange::new(base_layer.into(), desc.layers.into())
                                                swizzle:new_swizzle

                ]
            };

            Ok(Image { texture })
        }
    }

    fn detached(&self) -> bool {
        use foreign_types::ForeignType;
        use metal::NSUInteger;
        use objc::*;

        let count: NSUInteger = unsafe { msg_send![(self.texture.as_ptr()), retainCount] };
        count == 1
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum MTLTextureSwizzle {
    Zero = 0,
    One = 1,
    Red = 2,
    Green = 3,
    Blue = 4,
    Alpha = 5,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct MTLTextureSwizzleChannels {
    r: MTLTextureSwizzle,
    g: MTLTextureSwizzle,
    b: MTLTextureSwizzle,
    a: MTLTextureSwizzle,
}

impl Mul<Swizzle> for MTLTextureSwizzleChannels {
    type Output = Self;

    fn mul(self, rhs: Swizzle) -> Self {
        use ComponentSwizzle::*;

        let mul = |rhs: ComponentSwizzle, i: MTLTextureSwizzle| -> MTLTextureSwizzle {
            match rhs {
                Identity => i,
                Zero => MTLTextureSwizzle::Zero,
                One => MTLTextureSwizzle::One,
                R => self.r,
                G => self.g,
                B => self.b,
                A => self.a,
            }
        };

        let r = mul(rhs.r, self.r);
        let g = mul(rhs.g, self.g);
        let b = mul(rhs.b, self.b);
        let a = mul(rhs.a, self.a);

        MTLTextureSwizzleChannels { r, g, b, a }
    }
}
