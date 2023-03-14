use std::fmt;

use crate::OutOfMemory;

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

#[derive(Debug)]
pub enum QueueError {
    OutOfMemory,
    DeviceLost,
}

impl From<OutOfMemory> for QueueError {
    #[inline]
    fn from(_: OutOfMemory) -> Self {
        QueueError::OutOfMemory
    }
}

impl fmt::Display for QueueError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::OutOfMemory => write!(f, "out of memory"),
            QueueError::DeviceLost => write!(f, "device lost"),
        }
    }
}

impl std::error::Error for QueueError {}
