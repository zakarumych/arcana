use std::path::Path;

use argosy_import::{loading::LoadingError, Importer};
use hashbrown::{hash_map::RawEntryMut, HashMap};

#[derive(Debug, thiserror::Error)]
#[error("Multiple importers may import from different formats '{formats:?}' to target '{target}'")]
pub struct CannotDecideOnImporter {
    pub formats: Vec<String>,
    pub target: String,
}

struct ToTarget {
    importers: Vec<Box<dyn Importer>>,
    formats: HashMap<String, usize>,
    extensions: HashMap<String, usize>,
}

pub struct Importers {
    targets: HashMap<String, ToTarget>,
}

impl Importers {
    pub fn new() -> Self {
        Importers {
            targets: HashMap::new(),
        }
    }

    /// Loads importers from dylib.
    ///
    /// # Safety
    ///
    /// There is no possible way to guarantee that dylib does not break safety contracts.
    /// Some measures to ensure safety are taken.
    /// Providing dylib from which importers will be successfully imported and then cause an UB should possible only on purpose.
    pub unsafe fn load_dylib_importers(&mut self, lib_path: &Path) -> Result<(), LoadingError> {
        let iter = argosy_import::loading::load_importers(lib_path)?;

        for importer in iter {
            self.add_importer(Box::new(importer));
        }

        Ok(())
    }

    /// Try to guess importer by optionally provided format and extension or by target alone.
    pub fn guess(
        &self,
        format: Option<&str>,
        extension: Option<&str>,
        target: &str,
    ) -> Result<Option<&dyn Importer>, CannotDecideOnImporter> {
        tracing::debug!("Guessing importer to '{}'", target);

        let to_target = self.targets.get(target);

        match to_target {
            None => {
                tracing::debug!("No importers to '{}' found", target);
                Ok(None)
            }
            Some(to_target) => match format {
                None => match extension {
                    None => match to_target.importers.len() {
                        0 => {
                            unreachable!()
                        }
                        1 => Ok(Some(&*to_target.importers[0])),
                        _ => {
                            tracing::debug!("Multiple importers to '{}' found", target);
                            Err(CannotDecideOnImporter {
                                target: target.to_owned(),
                                formats: to_target.formats.keys().cloned().collect(),
                            })
                        }
                    },
                    Some(extension) => match to_target.extensions.get(extension) {
                        None => Ok(None),
                        Some(&idx) => Ok(Some(&*to_target.importers[idx])),
                    },
                },
                Some(format) => match to_target.formats.get(format) {
                    None => Ok(None),
                    Some(&idx) => Ok(Some(&*to_target.importers[idx])),
                },
            },
        }
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

        match self.targets.raw_entry_mut().from_key(target) {
            RawEntryMut::Vacant(entry) => {
                let to_target = entry
                    .insert(
                        target.to_owned(),
                        ToTarget {
                            importers: Vec::new(),
                            formats: HashMap::new(),
                            extensions: HashMap::new(),
                        },
                    )
                    .1;

                for &format in &*formats {
                    to_target.formats.insert(format.to_owned(), 0);
                }

                for &extension in &*extensions {
                    to_target.extensions.insert(extension.to_owned(), 0);
                }
                to_target.importers.push(importer);
            }
            RawEntryMut::Occupied(entry) => {
                let to_target = entry.into_mut();
                let idx = to_target.importers.len();

                for &format in &*formats {
                    match to_target.formats.raw_entry_mut().from_key(format) {
                        RawEntryMut::Vacant(entry) => {
                            entry.insert(format.to_owned(), idx);
                        }
                        RawEntryMut::Occupied(entry) => {
                            tracing::error!(
                                "'{}' -> '{}' importer already registered: {:#?}",
                                format,
                                target,
                                entry.get(),
                            );
                        }
                    }
                }

                for &extension in &*extensions {
                    match to_target.extensions.raw_entry_mut().from_key(extension) {
                        RawEntryMut::Vacant(entry) => {
                            entry.insert(extension.to_owned(), idx);
                        }
                        RawEntryMut::Occupied(entry) => {
                            tracing::error!(
                                "'.{}' -> '{}' importer already registered: {:#?}",
                                extension,
                                target,
                                entry.get(),
                            );
                        }
                    }
                }

                to_target.importers.push(importer);
            }
        }
    }
}
