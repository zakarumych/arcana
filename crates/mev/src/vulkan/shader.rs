use std::{fmt, sync::Arc};

use ash::vk;

use crate::generic::{OutOfMemory, ShaderCompileError};

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

#[derive(Debug)]
pub(crate) enum CreateLibraryErrorKind {
    OutOfMemory,
    CompileError(ShaderCompileError),
}

impl From<OutOfMemory> for CreateLibraryErrorKind {
    #[inline(always)]
    fn from(_: OutOfMemory) -> Self {
        CreateLibraryErrorKind::OutOfMemory
    }
}

impl From<ShaderCompileError> for CreateLibraryErrorKind {
    #[inline(always)]
    fn from(err: ShaderCompileError) -> Self {
        CreateLibraryErrorKind::CompileError(err)
    }
}

impl fmt::Display for CreateLibraryErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateLibraryErrorKind::OutOfMemory => write!(f, "{OutOfMemory}"),
            CreateLibraryErrorKind::CompileError(err) => fmt::Display::fmt(err, f),
        }
    }
}
