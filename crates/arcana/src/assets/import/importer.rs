use std::{any::Any, path::Path};

use arcana_names::{Ident, Name};

use crate::make_id;

use super::{AssetDependencies, AssetDependency, AssetSources};

make_id! {
    /// Unique identifier of an importer.
    pub ImporterId;
}

/// Error of `Importer::import` method.
pub enum ImportError {
    /// Importer requires data.
    Requires {
        /// Required sources to build this asset.
        sources: Vec<String>,

        /// Assets this asset depends on.
        dependencies: Vec<AssetDependency>,
    },

    /// Importer failed to import the asset.
    Other {
        /// Failure reason.
        reason: String,
    },
}

pub trait ImportConfig: egui_probe::EguiProbe + Any + Send + Sync {}
impl<T> ImportConfig for T where T: egui_probe::EguiProbe + Any + Send + Sync {}

impl dyn ImportConfig {
    #[inline(always)]
    pub fn is<T: 'static>(&self) -> bool {
        self.type_id() == std::any::TypeId::of::<T>()
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const dyn ImportConfig as *const T)) }
        } else {
            None
        }
    }

    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut dyn ImportConfig as *mut T)) }
        } else {
            None
        }
    }

    pub fn downcast<T: 'static>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if self.is::<T>() {
            unsafe {
                let raw: *mut dyn ImportConfig = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }
}

pub struct EmptyConfig;

impl egui_probe::EguiProbe for EmptyConfig {
    fn probe(&mut self, ui: &mut egui::Ui, _style: &egui_probe::Style) -> egui::Response {
        ui.label("No configuration")
    }
}

#[derive(Clone, Debug)]
pub struct ImporterDesc {
    pub formats: Vec<String>,
    pub extensions: Vec<String>,
    pub target: Ident,
}

/// Trait for an importer.
pub trait Importer: Send + Sync + 'static {
    /// Returns name of the importer
    fn name() -> Name
    where
        Self: Sized;

    /// Returns description of the importer.
    fn desc() -> ImporterDesc
    where
        Self: Sized;

    /// Returns importer instance.
    fn new() -> Self
    where
        Self: Sized;

    /// Returns configuration value for this importer.
    fn config(&self) -> Box<dyn ImportConfig> {
        Box::new(EmptyConfig)
    }

    /// Reads data from `source` path and writes result at `output` path.
    /// Implementation may request additional sources and dependencies.
    /// If some are missing it **should** return `Err(ImportError::Requires { .. })`
    /// with as much information as possible.
    fn import(
        &self,
        source: &Path,
        output: &Path,
        sources: &mut dyn AssetSources,
        dependencies: &mut dyn AssetDependencies,
    ) -> Result<(), ImportError>;
}
