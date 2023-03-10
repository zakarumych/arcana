use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::backend::{Buffer, Image, Queue, Surface};

use super::{buffer::BufferDesc, image::ImageError, ImageDesc, OutOfMemory};

pub trait Device {
    fn get_queue(&self, family: usize, idx: usize) -> Queue;
    fn create_buffer(&self, desc: BufferDesc) -> Result<Buffer, OutOfMemory>;
    fn create_image(&self, desc: ImageDesc) -> Result<Image, ImageError>;
    fn create_surface(&self, window: &RawWindowHandle, display: &RawDisplayHandle) -> Surface;
}
