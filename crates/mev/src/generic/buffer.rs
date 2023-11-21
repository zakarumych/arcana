bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct BufferUsage: u32 {
        const TRANSFER_SRC = 0x0000_0001;
        const TRANSFER_DST = 0x0000_0002;
        const UNIFORM = 0x0000_0004;
        const STORAGE = 0x0000_0008;
        const INDEX = 0x0000_0010;
        const VERTEX = 0x0000_0020;
        const INDIRECT = 0x0000_0040;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Memory {
    Device,
    Shared,
    Upload,
    Download,
}

/// Buffer description.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferDesc<'a> {
    /// Buffer size.
    pub size: usize,

    /// Buffer usage flags.
    pub usage: BufferUsage,

    /// Buffer memory type.
    pub memory: Memory,

    /// Buffer debug name.
    pub name: &'a str,
}

/// Buffer description with initial contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferInitDesc<'a> {
    /// Buffer initial contents.
    pub data: &'a [u8],

    /// Buffer usage flags.
    pub usage: BufferUsage,

    /// Buffer memory type.
    pub memory: Memory,

    /// Buffer debug name.
    pub name: &'a str,
}
