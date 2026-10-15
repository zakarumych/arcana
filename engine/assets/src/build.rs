use std::ops::{Deref, DerefMut};

use arcana_gpu::Gpu;

use super::assets::Assets;

pub struct BuilderEncoder {
    inner: mev::CommandEncoder,
    needs_flush: bool,
}

impl Deref for BuilderEncoder {
    type Target = mev::CommandEncoder;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for BuilderEncoder {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.needs_flush = true;
        &mut self.inner
    }
}

pub struct AssetBuilder<'a> {
    pub gpu: &'a mut Gpu,
    pub encoder: BuilderEncoder,
}

impl AssetBuilder<'_> {
    pub fn device(&self) -> &mev::Device {
        &self.gpu.device
    }

    pub fn encoder(&mut self) -> &mut mev::CommandEncoder {
        &mut self.encoder
    }
}

#[doc(hidden)]
pub struct AssetBuildContext {
    encoder: Option<mev::CommandEncoder>,
}

impl AssetBuildContext {
    pub fn new() -> Self {
        AssetBuildContext { encoder: None }
    }

    /// Submits the cached encoder before releasing its originating GPU.
    ///
    /// Use the same GPU supplied to `build_assets`. This does not build queued
    /// assets or wait for GPU completion; an absent cached encoder is a no-op.
    pub fn flush(&mut self, gpu: &mut Gpu) -> Result<(), mev::DeviceError> {
        if let Some(encoder) = self.encoder.take() {
            gpu.queue.submit([encoder.finish()])?;
        }
        Ok(())
    }

    /// Use the same GPU until the cached encoder is flushed.
    pub fn build_assets(&mut self, assets: &Assets, gpu: &mut Gpu) -> Result<(), mev::DeviceError> {
        let encoder = match self.encoder.take() {
            Some(encoder) => encoder,
            None => gpu.queue.new_command_encoder(),
        };

        let mut builder = AssetBuilder {
            encoder: BuilderEncoder {
                inner: encoder,
                needs_flush: false,
            },
            gpu,
        };

        assets.build_assets(&mut builder);

        if builder.encoder.needs_flush {
            let cbuf = builder.encoder.inner.finish();
            builder.gpu.queue.submit([cbuf])?;
        } else {
            self.encoder = Some(builder.encoder.inner);
        }

        Ok(())
    }
}
