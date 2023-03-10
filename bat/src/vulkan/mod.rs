use std::{alloc::Layout, fmt};

mod buffer;
mod command;
mod device;
mod from;
mod image;
mod instance;
mod queue;
mod surface;

pub use self::{
    buffer::Buffer,
    command::CommandBuffer,
    device::Device,
    image::Image,
    instance::{CreateError, Instance, LoadError},
    queue::Queue,
    surface::Surface,
};

#[track_caller]
fn handle_host_oom() -> ! {
    std::alloc::handle_alloc_error(Layout::new::<()>())
}

#[track_caller]
fn unexpected_error(err: ash::vk::Result) -> ! {
    unreachable!("unexpected error: {err:?}")
}

/// Version of the API.
/// For internal use only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    fn api_version(&self) -> u32 {
        ash::vk::make_api_version(0, self.major, self.minor, self.patch)
    }
}

impl fmt::Display for Version {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

const VERSION_1_0: Version = Version {
    major: 1,
    minor: 0,
    patch: 0,
};

const VERSION_1_1: Version = Version {
    major: 1,
    minor: 1,
    patch: 0,
};

const VERSION_1_2: Version = Version {
    major: 1,
    minor: 2,
    patch: 0,
};

const VERSION_1_3: Version = Version {
    major: 1,
    minor: 3,
    patch: 0,
};
