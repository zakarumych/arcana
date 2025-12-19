use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use arcana::{
    Ident, Name,
    assets::{
        AssetId,
        import::{ImportContext, ImportError, ImporterDesc, ImporterId, Missing},
    },
    error::{Error, WithContext, fail},
    id::TimeUidGen,
    model::Value,
    plugin::PluginsHub,
};
use base64::{
    Engine,
    alphabet::URL_SAFE,
    engine::general_purpose::{GeneralPurpose, NO_PAD},
};
use rand::random;

use hashbrown::HashMap;
use url::Url;

use crate::{
    assets::{fetcher::Fetcher, meta::SourceMeta, registry::AssetRegistry},
    plugins::Plugins,
    project::Project,
};

const MAX_ITEM_ATTEMPTS: usize = 10;

#[derive(Clone)]
struct AssetItem {
    source: Url,
    format: Option<String>,
    target: Ident,
}

/// Asset repository for Arcana Editor.
pub struct AssetStore {
    /// Base path to the asset store.
    base: PathBuf,

    /// Path to temporaries
    temporaries: PathBuf,

    /// Information about available asset importers.
    /// The importers are provided by plugins and can be used to import assets from various sources.
    /// Instances of importers are provided each frame to run queued imports.
    importers: HashMap<ImporterId, ImporterDesc>,

    /// Asset ID generator.
    /// Currently initialized with a random node ID.
    id_gen: TimeUidGen,

    /// Asset registry to store and load.
    registry: AssetRegistry,

    /// Converts source which may be relative or an URL into PathBuf.
    fetcher: Fetcher,
}

impl AssetStore {
    pub fn new(project: &Project) -> Result<Self, Error> {
        let base = project.root_path().join("assets");
        let temporaries = base.join("tmp");
        let registry = base.join("registry");
        let sources = base.join("sources");

        Ok(AssetStore {
            base,
            temporaries,
            id_gen: TimeUidGen::random(),
            importers: HashMap::new(),
            registry: AssetRegistry::new(registry)?,
            fetcher: Fetcher::new(sources)
                .with_context("Failed to open temporary sources path for asset store")?,
        })
    }

    pub fn registry(&self) -> &AssetRegistry {
        &self.registry
    }

    pub fn find_importers(
        &self,
        target: Ident,
        format: Option<Name>,
        extension: Option<&str>,
    ) -> Vec<(ImporterId, &ImporterDesc)> {
        let mut found = Vec::new();
        for (id, desc) in self.importers.iter() {
            if desc.target != target {
                continue;
            }

            if format.map_or(false, |f| !desc.formats.contains(&f)) {
                continue;
            }

            if extension.map_or(false, |ext| !desc.extensions.contains(&ext)) {
                continue;
            }

            found.push((*id, desc));
        }

        found
    }

    pub fn store(
        &mut self,
        hub: &PluginsHub,
        source: &str,
        importer: ImporterId,
        config: Value,
    ) -> Result<AssetId, Error> {
        let absolute_source = self.base.join(source);

        let Ok(source_url) = Url::from_file_path(&*absolute_source) else {
            fail!(
                "Failed to convert source path '{}' into URL",
                absolute_source.display()
            );
        };

        let id = AssetId::generate(&mut self.id_gen);
        self.import_stack(
            hub,
            vec![ImportItem {
                id,
                source: absolute_source,
                source_url,
                importer,
                config,
                attempt: 0,
            }],
        )?;

        Ok(id)
    }

    fn import_stack(&mut self, hub: &PluginsHub, mut stack: Vec<ImportItem>) -> Result<(), Error> {
        while let Some(top) = stack.pop() {
            let Some(desc) = self.importers.get(&top.importer) else {
                fail!("Importer {} is not found", top.importer);
            };

            let Some(importer) = hub.importers.get(&top.importer) else {
                fail!("Importer {} is not found", top.importer);
            };

            let source_meta = SourceMeta::open_or_create(top.source.clone())
                .with_context("Failed to store asset")?;

            if let Some(asset_meta) = source_meta.get_asset(desc.target)
                && asset_meta.config == top.config
            {
                // Already imported with required config.
                continue;
            }

            let mut modified = match top.source.metadata().and_then(|md| md.modified()) {
                Ok(modified) => modified,
                Err(error) => {
                    fail!(
                        "Failed to fetch asset source '{}' modified time: {}",
                        top.source.display(),
                        error
                    );
                }
            };

            let tmp = make_temporary(&self.temporaries);
            let mut ctx = StoreContext {
                base: &self.base,
                temporaries: &self.temporaries,
                source_url: &top.source_url,
                sources: &mut self.fetcher,
                latest: &mut modified,
            };

            if let Err(error) = importer.import(&*top.source, &*tmp, top.config.clone(), &mut ctx) {
                match error {
                    ImportError::Requires {
                        sources,
                        dependencies,
                    } => {
                        if top.attempt >= MAX_ITEM_ATTEMPTS {
                            fail!(
                                "Failed to import asset '{}' -> {}: Max attempts reached",
                                top.source.display(),
                                desc.target
                            );
                        }

                        // Analyze missing sources and try to fetch them for re-try.
                        for extra_source in sources {
                            let extra_source = match top.source_url.join(&extra_source) {
                                Ok(url) => url,
                                Err(error) => {
                                    fail!(
                                        "Failed to convert extra source '{}' into URL when importing '{}' with URL '{}': {}",
                                        extra_source,
                                        top.source.display(),
                                        top.source_url,
                                        error
                                    );
                                }
                            };

                            if let Err(error) = self.fetcher.fetch(&extra_source) {
                                fail!(
                                    "Failed to fetch extra source '{}' when importing '{}': {}",
                                    extra_source,
                                    top.source.display(),
                                    error
                                );
                            }
                        }

                        stack.push(top.clone());

                        // Analyze missing dependencies and put their import on top.
                        for dep in dependencies {
                            let dep_id = AssetId::generate(&mut self.id_gen);
                            let importers = self.find_importers(dep.target, dep.format, None);
                            let (dep_importer_id, dep_desc) =
                                exactly_one_importer(importers, dep.target, dep.format, None)?;

                            let dep_source = top.source.join(&*dep.source);

                            let Ok(dep_source_url) = Url::from_file_path(&*dep_source) else {
                                fail!(
                                    "Failed to convert source path '{}' into URL",
                                    dep_source.display()
                                );
                            };

                            let config = dep_desc.config.1.clone();

                            stack.push(ImportItem {
                                id: dep_id,
                                source: dep_source,
                                source_url: dep_source_url,
                                importer: dep_importer_id,
                                config,
                                attempt: 0,
                            });
                        }
                    }
                    ImportError::Other { reason } => {
                        fail!(
                            "Failed to import asset '{}' -> {}: {}",
                            top.source.display(),
                            desc.target,
                            reason
                        );
                    }
                }
            }

            // Asset successfully imported.
            let version = modified_to_version(modified);

            if let Err(error) = self
                .registry
                .add_asset_from_tmp_file(top.id, version, &*tmp)
            {
                fail!(
                    "Failed to register imported asset '{}' -> {}: {}",
                    top.source.display(),
                    desc.target,
                    error
                );
            }
        }

        Ok(())
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        self.importers.clear();
        for (_, plugin) in plugins.iter() {
            for importer in plugin.importers() {
                self.importers.insert(importer.id, importer.desc.clone());
            }
        }
    }
}

#[derive(Clone)]
struct ImportItem {
    id: AssetId,
    source: PathBuf,
    source_url: Url,
    importer: ImporterId,
    config: Value,
    attempt: usize,
}

struct StoreContext<'a> {
    base: &'a Path,
    temporaries: &'a Path,
    source_url: &'a Url,
    sources: &'a mut Fetcher,
    latest: &'a mut SystemTime,
}

impl ImportContext for StoreContext<'_> {
    fn get_dependency(&mut self, source: &str, target: Ident) -> Result<AssetId, Missing> {
        let absolute_source = self.base.join(source);
        match SourceMeta::open(absolute_source) {
            Ok(source_meta) => match source_meta.get_asset(target) {
                Some(asset_meta) => Ok(asset_meta.id),
                None => Err(Missing),
            },
            Err(_) => Err(Missing),
        }
    }

    fn get_source(&mut self, source: &str) -> Result<PathBuf, Missing> {
        let Ok(source_url) = self.source_url.join(source) else {
            return Err(Missing);
        };
        match self.sources.get(&source_url) {
            Some((path, modified)) => {
                *self.latest = (*self.latest).max(modified);
                Ok(path.to_path_buf())
            }
            None => Err(Missing),
        }
    }
}

pub struct DependencyError;

pub fn make_temporary(base: &Path) -> PathBuf {
    loop {
        let key: u128 = random();
        let key_bytes = key.to_le_bytes();
        let mut filename = [0; 26];
        let len = GeneralPurpose::new(&URL_SAFE, NO_PAD)
            .encode_slice(&key_bytes, &mut filename[..22])
            .unwrap();
        debug_assert_eq!(len, 22);
        filename[22..].copy_from_slice(b".tmp");
        let path = base.join(std::str::from_utf8(&filename).unwrap());
        if !path.exists() {
            return path;
        }
    }
}

#[inline]
fn modified_to_version(modified: SystemTime) -> u64 {
    modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("SystemTime must be after UNIX_EPOCH")
        .as_secs()
}

fn exactly_one_importer<'a>(
    importers: Vec<(ImporterId, &'a ImporterDesc)>,
    target: Ident,
    format: Option<Name>,
    extension: Option<&str>,
) -> Result<(ImporterId, &'a ImporterDesc), Error> {
    if importers.len() > 1 {
        match (format, extension) {
            (None, None) => fail!(
                "Multiple importers found for target '{target}', try to narrow down with input format"
            ),
            (Some(format), None) => {
                fail!("Multiple importers found for target '{target}' and format '{format}'");
            }
            (None, Some(extension)) => {
                fail!("Multiple importers found for target '{target}' and extension '{extension}'");
            }
            (Some(format), Some(extension)) => {
                fail!(
                    "Multiple importers found for target '{target}', format {format} and extension '{extension}'"
                );
            }
        }
    }

    if importers.is_empty() {
        match (format, extension) {
            (None, None) => fail!(
                "No importers found for target '{target}', try to narrow down with input format"
            ),
            (Some(format), None) => {
                fail!("No importers found for target '{target}' and format '{format}'");
            }
            (None, Some(extension)) => {
                fail!("No importers found for target '{target}' and extension '{extension}'");
            }
            (Some(format), Some(extension)) => {
                fail!(
                    "No importers found for target '{target}', format {format} and extension '{extension}'"
                );
            }
        }
    }

    Ok(importers[0])
}
