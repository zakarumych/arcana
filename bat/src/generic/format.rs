pub enum Format {
    R8Unorm,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Uint,
    R16Sint,
    R16Float,
    R32Uint,
    R32Sint,
    R32Float,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    Rg16Uint,
    Rg16Sint,
    Rg16Float,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgb16Uint,
    Rgb16Sint,
    Rgb16Float,
    Rgb32Uint,
    Rgb32Sint,
    Rgb32Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    D16Unorm,
    D32Float,
    S8Uint,
    D16UnormS8Uint,
    D24UnormS8Uint,
    D32FloatS8Uint,
}

impl Format {
    pub fn is_color(&self) -> bool {
        match self {
            Format::R8Unorm
            | Format::R8Snorm
            | Format::R8Uint
            | Format::R8Sint
            | Format::R16Uint
            | Format::R16Sint
            | Format::R16Float
            | Format::R32Uint
            | Format::R32Sint
            | Format::R32Float
            | Format::Rg8Unorm
            | Format::Rg8Snorm
            | Format::Rg8Uint
            | Format::Rg8Sint
            | Format::Rg16Uint
            | Format::Rg16Sint
            | Format::Rg16Float
            | Format::Rg32Uint
            | Format::Rg32Sint
            | Format::Rg32Float
            | Format::Rgb16Uint
            | Format::Rgb16Sint
            | Format::Rgb16Float
            | Format::Rgb32Uint
            | Format::Rgb32Sint
            | Format::Rgb32Float
            | Format::Rgba8Unorm
            | Format::Rgba8UnormSrgb
            | Format::Rgba8Snorm
            | Format::Rgba8Uint
            | Format::Rgba8Sint
            | Format::Rgba16Uint
            | Format::Rgba16Sint
            | Format::Rgba16Float
            | Format::Rgba32Uint
            | Format::Rgba32Sint
            | Format::Rgba32Float
            | Format::Bgra8Unorm
            | Format::Bgra8UnormSrgb => true,
            Format::D16Unorm
            | Format::D32Float
            | Format::S8Uint
            | Format::D16UnormS8Uint
            | Format::D24UnormS8Uint
            | Format::D32FloatS8Uint => false,
        }
    }

    pub fn is_depth(&self) -> bool {
        match self {
            Format::R8Unorm
            | Format::R8Snorm
            | Format::R8Uint
            | Format::R8Sint
            | Format::R16Uint
            | Format::R16Sint
            | Format::R16Float
            | Format::R32Uint
            | Format::R32Sint
            | Format::R32Float
            | Format::Rg8Unorm
            | Format::Rg8Snorm
            | Format::Rg8Uint
            | Format::Rg8Sint
            | Format::Rg16Uint
            | Format::Rg16Sint
            | Format::Rg16Float
            | Format::Rg32Uint
            | Format::Rg32Sint
            | Format::Rg32Float
            | Format::Rgb16Uint
            | Format::Rgb16Sint
            | Format::Rgb16Float
            | Format::Rgb32Uint
            | Format::Rgb32Sint
            | Format::Rgb32Float
            | Format::Rgba8Unorm
            | Format::Rgba8UnormSrgb
            | Format::Rgba8Snorm
            | Format::Rgba8Uint
            | Format::Rgba8Sint
            | Format::Rgba16Uint
            | Format::Rgba16Sint
            | Format::Rgba16Float
            | Format::Rgba32Uint
            | Format::Rgba32Sint
            | Format::Rgba32Float
            | Format::Bgra8Unorm
            | Format::Bgra8UnormSrgb => false,
            Format::S8Uint => false,
            Format::D16Unorm
            | Format::D32Float
            | Format::D16UnormS8Uint
            | Format::D24UnormS8Uint
            | Format::D32FloatS8Uint => true,
        }
    }

    pub fn is_stencil(&self) -> bool {
        match self {
            Format::R8Unorm
            | Format::R8Snorm
            | Format::R8Uint
            | Format::R8Sint
            | Format::R16Uint
            | Format::R16Sint
            | Format::R16Float
            | Format::R32Uint
            | Format::R32Sint
            | Format::R32Float
            | Format::Rg8Unorm
            | Format::Rg8Snorm
            | Format::Rg8Uint
            | Format::Rg8Sint
            | Format::Rg16Uint
            | Format::Rg16Sint
            | Format::Rg16Float
            | Format::Rg32Uint
            | Format::Rg32Sint
            | Format::Rg32Float
            | Format::Rgb16Uint
            | Format::Rgb16Sint
            | Format::Rgb16Float
            | Format::Rgb32Uint
            | Format::Rgb32Sint
            | Format::Rgb32Float
            | Format::Rgba8Unorm
            | Format::Rgba8UnormSrgb
            | Format::Rgba8Snorm
            | Format::Rgba8Uint
            | Format::Rgba8Sint
            | Format::Rgba16Uint
            | Format::Rgba16Sint
            | Format::Rgba16Float
            | Format::Rgba32Uint
            | Format::Rgba32Sint
            | Format::Rgba32Float
            | Format::Bgra8Unorm
            | Format::Bgra8UnormSrgb => false,
            Format::D16Unorm | Format::D32Float => false,
            Format::S8Uint
            | Format::D16UnormS8Uint
            | Format::D24UnormS8Uint
            | Format::D32FloatS8Uint => true,
        }
    }
}
