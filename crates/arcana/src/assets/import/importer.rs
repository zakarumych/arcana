use std::{any::Any, path::Path};

use arcana_names::Ident;

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

pub trait ImportConfig: egui_probe::EguiProbe + Any + Send + Sync {
    fn serialize(&self, ser: toml::Serializer) -> Result<(), toml::ser::Error>;
    fn deserialize(&mut self, de: toml::Deserializer) -> Result<(), toml::de::Error>;
}

impl<T> ImportConfig for T
where
    T: egui_probe::EguiProbe
        + serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + Any
        + Send
        + Sync,
{
    fn serialize(&self, ser: toml::Serializer) -> Result<(), toml::ser::Error> {
        serde::Serialize::serialize(self, ser)
    }

    fn deserialize(&mut self, de: toml::Deserializer) -> Result<(), toml::de::Error> {
        serde::Deserialize::deserialize_in_place(de, self)
    }
}

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

/// Special kind of configuration that won't bring any info.
/// Editor will not attempt to show any configuration for this importer.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct EmptyConfig;

impl egui_probe::EguiProbe for EmptyConfig {
    fn probe(&mut self, ui: &mut egui::Ui, _style: &egui_probe::Style) -> egui::Response {
        ui.label("No configuration")
    }
}

/// Trait for an importer.
pub trait Importer: Send + Sync + 'static {
    /// Returns source formats importer works with.
    fn formats(&self) -> &[&str];

    /// Returns list of extensions for source formats.
    fn extensions(&self) -> &[&str];

    /// Returns target format importer produces.
    fn target(&self) -> Ident;

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
