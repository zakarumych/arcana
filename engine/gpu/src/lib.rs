//! Device, submission queue, and reusable GPU helpers.

/// Shared GPU state for rendering and asset builds.
pub struct Gpu {
    pub queue: mev::Queue,
    pub uploader: jkl_mev::image::uploader::Uploader,
    pub cvt: mev::kernels::Cvt,
    pub device: mev::Device,
}

impl Gpu {
    pub fn new(queue: mev::Queue) -> Result<Self, mev::OutOfMemory> {
        let device = queue.device().clone();
        let uploader = jkl_mev::image::uploader::Uploader::new(&device)?;
        let cvt = mev::kernels::Cvt::new(&device);
        Ok(Self {
            queue,
            uploader,
            cvt,
            device,
        })
    }
}
