use arcana_names::{Ident, Name};
use hashbrown::HashMap;

use crate::assets::import::Importer;

pub struct Importers {
    importers: HashMap<Name, Box<dyn Importer>>,
}

impl Importers {
    pub fn new() -> Self {
        Importers {
            importers: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.importers.clear();
    }

    /// Adds importer to the list of importers.
    pub fn add_importer(&mut self, name: Name, importer: Box<dyn Importer>) {
        if self.importers.contains_key(&name) {
            tracing::warn!("Importer '{}' already registered", name);
            return;
        }

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

        self.importers.insert(name, importer);
    }

    pub fn get(&self, name: Name) -> Option<&dyn Importer> {
        self.importers.get(&name).map(|importer| &**importer)
    }

    /// Select importers that match the given target, format and extension.
    pub fn select(
        &self,
        target: Option<Ident>,
        format: Option<&str>,
        extension: Option<&str>,
    ) -> Vec<(Name, &dyn Importer)> {
        self.importers
            .iter()
            .filter(|(_, importer)| match target {
                None => true,
                Some(target) => importer.target() == target,
            })
            .filter(|(_, importer)| match format {
                None => true,
                Some(format) => importer.formats().contains(&format),
            })
            .filter(|(_, importer)| match extension {
                None => true,
                Some(extension) => importer.extensions().contains(&extension),
            })
            .map(|(name, importer)| (*name, &**importer))
            .collect()
    }
}
