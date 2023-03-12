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

pub(crate) enum CreateLibraryErrorKind {
    CompileError(ShaderCompileError),
}
