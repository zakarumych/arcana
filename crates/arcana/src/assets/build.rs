use super::assets::Assets;

pub struct AssetBuilder {
    device: mev::Device,
    encoder: mev::CommandEncoder,
    needs_flush: bool,
}

impl AssetBuilder {
    pub fn device(&self) -> &mev::Device {
        &self.device
    }

    pub fn encoder(&mut self) -> &mut mev::CommandEncoder {
        self.needs_flush = true;
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

    pub fn build_assets(
        &mut self,
        assets: &Assets,
        queue: &mut mev::Queue,
    ) -> Result<(), mev::DeviceError> {
        let encoder = match self.encoder.take() {
            Some(encoder) => encoder,
            None => queue.new_command_encoder()?,
        };

        let mut builder = AssetBuilder {
            device: queue.device().clone(),
            encoder,
            needs_flush: false,
        };

        assets.build_assets(&mut builder);

        if builder.needs_flush {
            let cbuf = builder.encoder.finish()?;
            queue.submit([cbuf], false)?;
        } else {
            self.encoder = Some(builder.encoder);
        }

        Ok(())
    }
}
