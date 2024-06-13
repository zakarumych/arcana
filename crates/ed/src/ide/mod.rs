use std::path::Path;

mod vscode;

/// IDE support for Arcana
pub trait Ide {
    /// Opens the given path in the IDE.
    fn open(&self, path: &Path, line: Option<u32>) -> bool;
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    egui_probe::EguiProbe,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum IdeType {
    #[default]
    VSCode,
}

impl IdeType {
    pub fn get(&self) -> Box<dyn Ide> {
        match self {
            IdeType::VSCode => Box::new(vscode::VSCode),
        }
    }
}
