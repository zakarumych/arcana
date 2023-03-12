use super::format::PixelFormat;

pub enum ImageError {
    OutOfMemory,
    InvalidFormat,
}

pub enum ImageDimensions {
    D1(u32),
    D2(u32, u32),
    D3(u32, u32, u32),
}

bitflags::bitflags! {
    pub struct ImageUsage: u32 {
        const TRANSFER_SRC = 0x0000_0001;
        const TRANSFER_DST = 0x0000_0002;
        const SAMPLED = 0x0000_0004;
        const STORAGE = 0x0000_0008;
        const TARGET = 0x0000_0010;
    }
}

pub struct ImageDesc {
    pub dimensions: ImageDimensions,
    pub format: PixelFormat,
    pub usage: ImageUsage,
    pub layers: u32,
    pub levels: u32,
}
