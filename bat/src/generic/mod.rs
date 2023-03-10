mod buffer;
mod device;
mod feature;
mod format;
mod image;
mod instance;
mod queue;
mod surface;

pub use self::{
    buffer::{BufferDesc, BufferUsage, Memory},
    device::Device,
    feature::Features,
    format::Format,
    image::{ImageDesc, ImageDimensions, ImageError, ImageUsage},
    instance::{Capabilities, DeviceCapabilities, DeviceDesc, FamilyCapabilities, Instance},
    queue::{Queue, QueueFlags},
    // surface::Surface,
};

/// Error that can happen when device's memory is exhausted.
pub struct OutOfMemory;
