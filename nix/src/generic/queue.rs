bitflags::bitflags! {
    /// Flags that describe the capabilities of a queue.
    pub struct QueueFlags: u32 {
        /// The queue supports graphics operations.
        const GRAPHICS = 0x1;

        /// The queue supports compute operations.
        const COMPUTE = 0x2;

        /// The queue supports transfer operations.
        const TRANSFER = 0x4;
    }
}

pub enum QueueError {
    OutOfMemory,
    DeviceLost,
}
