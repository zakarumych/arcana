use std::{fmt, sync::Arc};

use ash::vk;

use super::device::WeakDevice;

struct LibraryInner {
    owner: WeakDevice,
    idx: usize,
}

impl Drop for LibraryInner {
    fn drop(&mut self) {
        self.owner.drop_library(self.idx);
    }
}

#[derive(Clone)]
pub struct Library {
    module: vk::ShaderModule,
    inner: Arc<LibraryInner>,
}

impl Library {
    pub(super) fn new(owner: WeakDevice, module: vk::ShaderModule, idx: usize) -> Self {
        Library {
            module,
            inner: Arc::new(LibraryInner { idx, owner }),
        }
    }

    pub(super) fn module(&self) -> vk::ShaderModule {
        self.module
    }
}
