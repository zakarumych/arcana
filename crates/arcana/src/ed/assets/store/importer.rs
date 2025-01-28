use arcana_names::{Ident, Name};
use hashbrown::HashMap;

use crate::assets::import::{Importer, ImporterDesc};

pub struct Importers {
    importers: Vec<(Name, ImporterDesc, Box<dyn Importer>)>,
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
    pub fn add_importer(&mut self, name: Name, desc: ImporterDesc, importer: Box<dyn Importer>) {
        tracing::info!(
            "Registering importer '{}'. '{:?}' -> '{}' {:?}",
            name,
            desc.formats,
            desc.target,
            desc.extensions,
        );

        self.importers.push((name, desc, importer));
    }

    pub fn get(&self, name: Name) -> Option<&dyn Importer> {
        self.importers
            .iter()
            .find(|(importer_name, _, _)| *importer_name == name)
            .map(|(_, _, i)| &**i)
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
            .filter(|(_, desc, _)| match target {
                None => true,
                Some(target) => desc.target == target,
            })
            .filter(|(_, desc, _)| match format {
                None => true,
                Some(format) => desc.formats.iter().any(|x| x == format),
            })
            .filter(|(_, desc, _)| match extension {
                None => true,
                Some(extension) => desc.extensions.iter().any(|x| x == extension),
            })
            .map(|(name, _, importer)| (*name, &**importer))
            .collect()
    }
}
