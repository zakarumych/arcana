use std::fmt;

use crate::generic::ShaderCompileError;

pub struct Library {
    library: metal::Library,
}

impl Library {
    pub(super) fn new(library: metal::Library) -> Self {
        Library { library }
    }

    pub(super) fn get_function(&self, entry: &str) -> Option<metal::Function> {
        self.library.get_function(entry, None).ok()
    }
}

#[derive(Debug)]
pub(crate) enum CreateLibraryErrorKind {
    CompileError(ShaderCompileError),
}

impl fmt::Display for CreateLibraryErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateLibraryErrorKind::CompileError(err) => fmt::Display::fmt(err, f),
        }
    }
}
