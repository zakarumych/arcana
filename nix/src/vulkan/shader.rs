use std::fmt;

use crate::generic::{OutOfMemory, ShaderCompileError};

pub struct Library {
    module: ash::vk::ShaderModule,
}

impl Library {
    pub(super) fn new(module: ash::vk::ShaderModule) -> Self {
        Library { module }
    }

    pub(super) fn module(&self) -> ash::vk::ShaderModule {
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
