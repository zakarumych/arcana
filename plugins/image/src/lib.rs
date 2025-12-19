use std::path::Path;

use arcana::{
    Ident,
    assets::import::{AssetDependencies, AssetSources, ImportError},
    ident,
};

// This line allows this crate to function as a plugin for Arcana Engine.
arcana::plugin::declare_plugin!();

#[arcana::importer]
struct RGBAImageImporter;

impl RGBAImageImporter {
    pub fn new() -> Self {
        RGBAImageImporter
    }
}

impl arcana::assets::import::Importer for RGBAImageImporter {
    fn desc() -> ImporterDesc {
        ImporterDesc {
            name: ident!(RGBAImageImporter),
        }
    }

    fn formats(&self) -> &[&str] {
        &["png", "jpg", "bmp"]
    }

    fn extensions(&self) -> &[&str] {
        &["png", "jpg", "jpeg", "bmp"]
    }

    fn target(&self) -> Ident {
        ident!(RGBA)
    }

    fn import(
        &self,
        source: &Path,
        output: &Path,
        _sources: &mut dyn AssetSources,
        _dependencies: &mut dyn AssetDependencies,
    ) -> Result<(), ImportError> {
        let image = match image::open(source) {
            Err(error) => {
                return Err(ImportError::Other {
                    reason: format!("Failed to open image: {}", error),
                });
            }
            Ok(image) => image,
        };

        let rgba_image = image.to_rgba8();

        match std::fs::write(output, rgba_image.as_raw()) {
            Err(error) => {
                return Err(ImportError::Other {
                    reason: format!("Failed to write image: {}", error),
                });
            }
            Ok(_) => {}
        }

        Ok(())
    }
}
