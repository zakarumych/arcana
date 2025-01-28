use std::path::Path;

use arcana::assets::import::{
    AssetDependencies, AssetSources, EmptyConfig, ImportError, Importer, ImporterDesc,
};

// This line allows this crate to function as a plugin for Arcana Engine.
arcana::declare_plugin!();

struct ImageImporter;

#[arcana::importer]
impl Importer for ImageImporter {
    fn name() -> arcana::Name {
        arcana::name!(Image)
    }

    fn desc() -> ImporterDesc {
        ImporterDesc {
            formats: ["png", "jpeg", "tga", "tiff", "bmp", "gif", "qoi", "webp"]
                .map(ToOwned::to_owned)
                .to_vec(),
            extensions: [
                "png", "jpg", "jpeg", "tga", "tiff", "bmp", "gif", "qoi", "webp",
            ]
            .map(ToOwned::to_owned)
            .to_vec(),
            target: arcana::ident!(Texture),
        }
    }

    fn new() -> Self {
        ImageImporter
    }

    fn import(
        &self,
        source: &Path,
        output: &Path,
        sources: &mut dyn AssetSources,
        dependencies: &mut dyn AssetDependencies,
    ) -> Result<(), ImportError> {
        todo!()
    }
}
