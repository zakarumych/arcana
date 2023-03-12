use std::convert::Infallible;

use crate::generic::{
    Capabilities, CreateError, DeviceCapabilities, DeviceDesc, FamilyCapabilities, Features,
    LoadError, QueueFlags,
};

use super::Device;

pub(crate) type LoadErrorKind = Infallible;

pub(crate) enum CreateErrorKind {
    FailedToCreateDevice,
}

pub struct Instance {
    capabilities: Capabilities,
}

impl Instance {
    pub fn load() -> Result<Self, LoadError>
    where
        Self: Sized,
    {
        Ok(Instance {
            capabilities: Capabilities {
                devices: vec![DeviceCapabilities {
                    features: Features::empty(),
                    families: vec![FamilyCapabilities {
                        queue_flags: QueueFlags::GRAPHICS
                            | QueueFlags::COMPUTE
                            | QueueFlags::TRANSFER,
                        queue_count: 32,
                    }],
                }],
            },
        })
    }
}

#[hidden_trait::expose]
impl crate::traits::Instance for Instance {
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn create(&self, info: DeviceDesc) -> Result<Device, CreateError> {
        let device = metal::Device::system_default()
            .ok_or(CreateError(CreateErrorKind::FailedToCreateDevice))?;

        assert!(
            info.queue_infos.len() <= 1,
            "Only one queue family is supported"
        );

        let queue_count = info.queue_infos.first().map_or(0, |info| {
            assert_eq!(info.family, 0, "Only one queue family is supported");
            info.queue_count
        });

        let queues = (0..queue_count)
            .map(|_| device.new_command_queue())
            .collect();

        Ok(Device::new(device, queues))
    }
}
