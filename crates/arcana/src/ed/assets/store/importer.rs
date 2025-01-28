use arcana_names::{Ident, Name};

use crate::assets::import::Importer;

pub struct Importers {
    importers: Vec<Box<dyn Importer>>,
}

impl Importers {
    pub fn new() -> Self {
        Importers {
            importers: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.importers.clear();
    }

    /// Adds importer to the list of importers.
    pub fn add_importer(&mut self, importer: Box<dyn Importer>) {
        let name = importer.name();
        let target = importer.target();
        let formats = importer.formats();
        let extensions = importer.extensions();

        tracing::info!(
            "Registering importer '{}'. '{:?}' -> '{}' {:?}",
            name,
            formats,
            target,
            extensions,
        );

        self.importers.push(importer);
    }

    pub fn get(&self, name: Name) -> Option<&dyn Importer> {
        self.importers
            .iter()
            .find(|importer| importer.name() == name)
            .map(|i| &**i)
    }

    /// Select importers that match the given target, format and extension.
    pub fn select(
        &self,
        target: Option<Ident>,
        format: Option<&str>,
        extension: Option<&str>,
    ) -> Vec<&dyn Importer> {
        self.importers
            .iter()
            .filter(|importer| match target {
                None => true,
                Some(target) => importer.target() == target,
            })
            .filter(|importer| match format {
                None => true,
                Some(format) => importer.formats().contains(&format),
            })
            .filter(|importer| match extension {
                None => true,
                Some(extension) => importer.extensions().contains(&extension),
            })
            .map(|importer| &**importer)
            .collect()
    }
}
