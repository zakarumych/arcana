//! Asset store can import assets, save imported assets on disk and load them on demand.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use arcana_names::{Ident, Name};
use arcana_project::real_path;
use futures::future::BoxFuture;
use hashbrown::{HashMap, HashSet};
use parking_lot::{Mutex, RwLock};
use url::Url;

use crate::{
    assets::{
        import::{AssetDependencies, AssetSources, ImportError, Importer, ImporterDesc},
        AssetData, AssetId, Error, Loader, NotFound,
    },
    id::TimeUidGen,
};

mod content_address;
mod importer;
mod meta;
mod scheme;
mod sources;
mod temp;

use self::{
    importer::Importers,
    meta::{AssetMeta, MetaError, SourceMeta},
    sources::{Sources, SourcesError},
    temp::make_temporary,
};

const DEFAULT_ARTIFACTS: &'static str = "artifacts";
const DEFAULT_EXTERNAL: &'static str = "external";
const MAX_ITEM_ATTEMPTS: u32 = 1024;

const DEFAULT_EPOCH: u64 = 1073741824;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct StoreInfo {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub artifacts: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub external: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub temp: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenStoreError {
    #[error("Failed to get real path of '{path}'")]
    PathError { path: PathBuf },

    #[error("Failed to read store metadata file '{path}'. {error}")]
    MetaReadError {
        error: std::io::Error,
        path: PathBuf,
    },

    #[error("Failed to deserialize store metadata file '{path}'. {error}")]
    MetaDeserializeError {
        error: toml::de::Error,
        path: PathBuf,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SaveStoreError {
    #[error("Failed to write store metadata file '{path}'. {error}")]
    MetaWriteError {
        error: std::io::Error,
        path: PathBuf,
    },

    #[error("Failed to serialize store metadata file '{path}'. {error}")]
    MetaSerializeError {
        error: toml::ser::Error,
        path: PathBuf,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Failed to construct asset URL from base '{base}' and source '{source}'. {error}")]
    InvalidSourceUrl {
        #[source]
        error: url::ParseError,
        base: Url,
        source: String,
    },

    #[error(transparent)]
    MetaError(MetaError),

    #[error("No importer found for '{url}' : '{format:?}->{target}'")]
    NoImporters {
        target: Ident,
        format: Option<String>,
        url: Url,
    },

    #[error("Many importer found for '{url}' : '{format:?}->{target}'")]
    ManyImporters {
        target: Ident,
        format: Option<String>,
        url: Url,
    },

    #[error(transparent)]
    SourcesError(SourcesError),

    #[error("Failed to import asset '{url}' by '{importer}'. {reason}")]
    ImportError {
        importer: Name,
        target: Ident,
        url: Url,
        reason: String,
    },

    #[error("Too many attempts to import asset '{url}' by '{importer}'")]
    TooManyAttempts {
        importer: Name,
        target: Ident,
        url: Url,
    },

    #[error("Failed to save artifact '{path}'. {error}")]
    FailedToSaveArtifact {
        error: std::io::Error,
        path: PathBuf,
    },
}

impl Default for StoreInfo {
    fn default() -> Self {
        StoreInfo::new(None, None, None)
    }
}

impl StoreInfo {
    pub fn new(artifacts: Option<&Path>, external: Option<&Path>, temp: Option<&Path>) -> Self {
        let artifacts = artifacts.map(Path::to_owned);
        let external = external.map(Path::to_owned);
        let temp = temp.map(Path::to_owned);

        StoreInfo {
            artifacts,
            external,
            temp,
        }
    }
}

#[derive(Clone)]
struct AssetItem {
    source: Url,
    format: Option<String>,
    target: Ident,
}

pub struct Store {
    base: PathBuf,
    base_url: Url,
    artifacts_base: PathBuf,
    external: PathBuf,
    temp: PathBuf,
    importers: RwLock<Importers>,

    artifacts: RwLock<HashMap<AssetId, AssetItem>>,
    scanned: RwLock<bool>,
    id_gen: Mutex<TimeUidGen>,
}

impl Store {
    pub fn new(base: &Path, meta: StoreInfo) -> Result<Self, OpenStoreError> {
        let base = real_path(base).ok_or_else(|| OpenStoreError::PathError {
            path: base.to_owned(),
        })?;

        let base_url =
            Url::from_directory_path(&base).expect("Absolute path must be convertible to URL");

        let artifacts = base.join(
            meta.artifacts
                .as_deref()
                .unwrap_or_else(|| DEFAULT_ARTIFACTS.as_ref()),
        );

        let external = base.join(
            meta.external
                .as_deref()
                .unwrap_or_else(|| DEFAULT_EXTERNAL.as_ref()),
        );

        let temp = meta
            .temp
            .map_or_else(std::env::temp_dir, |path| base.join(path));

        let importers = Importers::new();

        Ok(Store {
            base,
            base_url,
            artifacts_base: artifacts,
            external,
            temp,
            importers: RwLock::new(importers),
            artifacts: RwLock::new(HashMap::new()),
            scanned: RwLock::new(false),
            id_gen: Mutex::new(TimeUidGen::random_with_start(
                SystemTime::UNIX_EPOCH + Duration::from_secs(DEFAULT_EPOCH),
            )),
        })
    }

    /// Register importer.
    #[tracing::instrument(skip(self, importer))]
    pub fn register_importer(&self, name: Name, desc: ImporterDesc, importer: Box<dyn Importer>) {
        self.importers.write().add_importer(name, desc, importer);
    }

    #[tracing::instrument(skip(self))]
    pub fn purge_importers(&self) {
        self.importers.write().clear();
    }

    /// Import an asset.
    #[tracing::instrument(skip(self))]
    pub async fn store(
        &self,
        source: &str,
        target: Ident,
        format: Option<&str>,
    ) -> Result<(AssetId, PathBuf, SystemTime), StoreError> {
        let source = self
            .base_url
            .join(source)
            .map_err(|error| StoreError::InvalidSourceUrl {
                error,
                base: self.base_url.clone(),
                source: source.to_owned(),
            })?;

        self.store_from_url(source, target, format).await
    }

    /// Import an asset.
    #[tracing::instrument(skip(self))]
    pub async fn store_from_url(
        &self,
        source: Url,
        target: Ident,
        format: Option<&str>,
    ) -> Result<(AssetId, PathBuf, SystemTime), StoreError> {
        let mut sources = Sources::new();

        let base = &self.base;
        let artifacts_base = &self.artifacts_base;
        let external = &self.external;

        struct StackItem {
            /// Source URL.
            source: Url,

            target: Ident,

            format: Option<String>,

            /// Attempt counter to break infinite loops.
            attempt: u32,

            /// Sources requested by importer.
            /// Relative to `source`.
            sources: HashMap<Url, SystemTime>,

            /// Dependencies requested by importer.
            dependencies: HashSet<AssetId>,
        }

        let mut stack = Vec::new();
        stack.push(StackItem {
            source,
            target,
            format: format.map(ToOwned::to_owned),
            attempt: 0,
            sources: HashMap::new(),
            dependencies: HashSet::new(),
        });

        loop {
            let item = stack.last_mut().unwrap();
            item.attempt += 1;

            let mut meta = SourceMeta::new(&item.source, &self.base, &self.external)
                .map_err(StoreError::MetaError)?;

            if let Some(asset) = meta.get_asset(item.target) {
                if asset.needs_reimport(&self.base_url) {
                    tracing::debug!("'{}' as '{}' reimporting", item.source, item.target);
                } else {
                    tracing::debug!("Found '{}' as '{}'", item.source, item.target);
                    stack.pop().unwrap();

                    if stack.is_empty() {
                        let path = asset.artifact_path(&self.artifacts_base);
                        return Ok((asset.id(), path, asset.latest_modified()));
                    }
                    continue;
                }
            }

            let extension = url_ext(&item.source);

            // Fetch source file.
            let (source_path, source_modified) = sources
                .fetch(&self.temp, &item.source)
                .await
                .map_err(StoreError::SourcesError)?;

            let source_path = source_path.to_owned();
            let output_path = make_temporary(&self.temp);
            let selected_importer_name;
            let result;

            {
                let importers = self.importers.read();
                let selected_importers =
                    importers.select(Some(item.target), item.format.as_deref(), extension);

                if selected_importers.is_empty() {
                    return Err(StoreError::NoImporters {
                        format: item.format.clone(),
                        target: item.target,
                        url: item.source.clone(),
                    });
                }
                if selected_importers.len() > 1 {
                    return Err(StoreError::ManyImporters {
                        format: item.format.clone(),
                        target: item.target,
                        url: item.source.clone(),
                    });
                }

                let (name, selected_importer) = selected_importers[0];
                selected_importer_name = name;

                struct Fn<F>(F);

                impl<F> AssetSources for Fn<F>
                where
                    F: FnMut(&str) -> Option<PathBuf>,
                {
                    fn get(&mut self, source: &str) -> Option<PathBuf> {
                        (self.0)(source)
                    }
                }

                impl<F> AssetDependencies for Fn<F>
                where
                    F: FnMut(&str, Ident) -> Option<AssetId>,
                {
                    fn get(&mut self, source: &str, target: Ident) -> Option<AssetId> {
                        (self.0)(source, target)
                    }
                }

                result = selected_importer.import(
                    &source_path,
                    &output_path,
                    &mut Fn(|src: &str| {
                        let src = item.source.join(src).ok()?; // If parsing fails - source will be listed in `ImportResult::RequireSources`.
                        let (path, modified) = sources.get(&src)?;
                        item.sources.insert(src, modified);
                        Some(path.to_owned())
                    }),
                    &mut Fn(|src: &str, target: Ident| {
                        let src = item.source.join(src).ok()?;

                        match SourceMeta::new(&src, base, external) {
                            Ok(meta) => {
                                let asset = meta.get_asset(target)?;
                                item.dependencies.insert(asset.id());
                                Some(asset.id())
                            }
                            Err(err) => {
                                tracing::error!("Fetching dependency failed. {:#}", err);
                                None
                            }
                        }
                    }),
                );
            }

            match result {
                Ok(()) => {}
                Err(ImportError::Other { reason }) => {
                    return Err(StoreError::ImportError {
                        importer: selected_importer_name,
                        target: item.target,
                        url: item.source.clone(),
                        reason,
                    });
                }
                Err(ImportError::Requires {
                    sources: srcs,
                    dependencies: deps,
                }) => {
                    // If we have too many attempts, we should stop.
                    if item.attempt >= MAX_ITEM_ATTEMPTS {
                        return Err(StoreError::TooManyAttempts {
                            importer: selected_importer_name,
                            target: item.target,
                            url: item.source.clone(),
                        });
                    }

                    // Try to fulfill requirements.

                    // Fetch required sources.
                    for src in srcs {
                        match item.source.join(&src) {
                            Err(error) => {
                                return Err(StoreError::InvalidSourceUrl {
                                    error,
                                    base: item.source.clone(),
                                    source: src.clone(),
                                });
                            }
                            Ok(url) => sources
                                .fetch(&self.temp, &url)
                                .await
                                .map_err(StoreError::SourcesError)?,
                        };
                    }

                    // Import dependencies.
                    let item_source = item.source.clone();
                    for dep in deps {
                        match item_source.join(&dep.source) {
                            Err(error) => {
                                return Err(StoreError::InvalidSourceUrl {
                                    error,
                                    base: item_source.clone(),
                                    source: dep.source.clone(),
                                });
                            }
                            Ok(url) => {
                                stack.push(StackItem {
                                    source: url,
                                    format: None,
                                    target: dep.target,
                                    attempt: 0,
                                    sources: HashMap::new(),
                                    dependencies: HashSet::new(),
                                });
                            }
                        };
                    }
                    continue;
                }
            }

            if !artifacts_base.exists() {
                if let Err(err) = std::fs::create_dir_all(artifacts_base) {
                    tracing::error!("Failed to create artifacts directory. {:#}", err);
                }

                if let Err(err) = std::fs::write(artifacts_base.join(".gitignore"), "*") {
                    tracing::error!(
                        "Failed to place .gitignore into artifacts directory. {:#}",
                        err
                    );
                }
            }

            let new_id = AssetId::generate(&mut *self.id_gen.lock());
            let item = stack.pop().unwrap();

            let make_relative_source = |source| match self.base_url.make_relative(source) {
                None => source.to_string(),
                Some(source) => source,
            };

            let mut sources = Vec::new();

            sources.push((make_relative_source(&item.source), source_modified));

            sources.extend(
                item.sources
                    .iter()
                    .map(|(url, modified)| (make_relative_source(url), (*modified))),
            );

            let asset = AssetMeta::new(
                new_id,
                item.format.clone(),
                sources,
                item.dependencies.into_iter().collect(),
                &output_path,
                artifacts_base,
            )
            .map_err(StoreError::MetaError)?;

            let artifact_path = asset.artifact_path(artifacts_base);

            let latest_modified = asset.latest_modified();
            meta.add_asset(item.target, asset, base, external)
                .map_err(StoreError::MetaError)?;

            self.artifacts.write().insert(
                new_id,
                AssetItem {
                    source: item.source,
                    format: item.format,
                    target: item.target,
                },
            );

            if stack.is_empty() {
                return Ok((new_id, artifact_path, latest_modified));
            }
        }
    }

    /// Fetch asset data path.
    pub async fn fetch(&self, id: AssetId) -> Option<(PathBuf, SystemTime)> {
        let scanned = *self.scanned.read();

        if !scanned {
            let existing_artifacts: HashSet<_> = self.artifacts.read().keys().copied().collect();

            let mut new_artifacts = Vec::new();
            let mut scanned = self.scanned.write();

            if !*scanned {
                scan_local(&self.base, &existing_artifacts, &mut new_artifacts);
                scan_external(&self.external, &existing_artifacts, &mut new_artifacts);

                let mut artifacts = self.artifacts.write();
                for (id, item) in new_artifacts {
                    artifacts.insert(id, item);
                }

                *scanned = true;

                drop(artifacts);
                drop(scanned);
            }
        }

        let item = self.artifacts.read().get(&id).cloned()?;

        let (_, path, modified) = self
            .store_from_url(item.source, item.target, item.format.as_deref())
            .await
            .ok()?;

        Some((path, modified))
    }

    /// Fetch asset data path.
    pub async fn find_asset(
        &self,
        source: &str,
        target: Ident,
    ) -> Result<Option<AssetId>, StoreError> {
        let source_url =
            self.base_url
                .join(source)
                .map_err(|error| StoreError::InvalidSourceUrl {
                    error,
                    base: self.base_url.clone(),
                    source: source.to_owned(),
                })?;

        let meta = SourceMeta::new(&source_url, &self.base, &self.external)
            .map_err(StoreError::MetaError)?;

        match meta.get_asset(target) {
            None => {
                drop(meta);
                match self.store(source, target, None).await {
                    Err(err) => {
                        tracing::warn!(
                            "Failed to store '{}' as '{}' on lookup. {:#}",
                            source,
                            target,
                            err
                        );
                        Ok(None)
                    }
                    Ok((id, _, _)) => Ok(Some(id)),
                }
            }
            Some(asset) => Ok(Some(asset.id())),
        }
    }
}

fn url_ext(url: &Url) -> Option<&str> {
    let path = url.path();
    let dot = path.rfind('.')?;
    if dot >= path.len() - 1 {
        return None;
    }

    if let Some(sep) = path.rfind('/') {
        if dot < sep {
            return None;
        }
    }

    Some(&path[dot + 1..])
}

fn scan_external(
    external: &Path,
    existing_artifacts: &HashSet<AssetId>,
    artifacts: &mut Vec<(AssetId, AssetItem)>,
) {
    let dir = match std::fs::read_dir(&external) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!("External directory does not exists");
            return;
        }
        Err(err) => {
            tracing::error!(
                "Failed to scan directory '{}'. {:#}",
                external.display(),
                err
            );
            return;
        }
        Ok(dir) => dir,
    };
    for e in dir {
        let e = match e {
            Err(err) => {
                tracing::error!(
                    "Failed to read entry in directory '{}'. {:#}",
                    external.display(),
                    err,
                );
                return;
            }
            Ok(e) => e,
        };
        let name = e.file_name();
        let path = external.join(&name);
        let ft = match e.file_type() {
            Err(err) => {
                tracing::error!("Failed to check '{}'. {:#}", path.display(), err);
                continue;
            }
            Ok(ft) => ft,
        };
        if ft.is_file() && !SourceMeta::is_local_meta_path(&path) {
            let meta = match SourceMeta::open_external(&path) {
                Err(err) => {
                    tracing::error!("Failed to scan meta file '{}'. {:#}", path.display(), err);
                    continue;
                }
                Ok(meta) => meta,
            };

            let source = meta.url();

            for (target, asset) in meta.assets() {
                if !existing_artifacts.contains(&asset.id()) {
                    artifacts.push((
                        asset.id(),
                        AssetItem {
                            source: source.clone(),
                            format: asset.format().map(ToOwned::to_owned),
                            target: target.to_owned(),
                        },
                    ));
                }
            }
        }
    }
}

fn scan_local(
    base: &Path,
    existing_artifacts: &HashSet<AssetId>,
    artifacts: &mut Vec<(AssetId, AssetItem)>,
) {
    debug_assert!(base.is_absolute());

    if !base.exists() {
        tracing::info!("Local artifacts directory does not exists");
        return;
    }

    let mut queue = VecDeque::new();
    queue.push_back(base.to_owned());

    while let Some(dir_path) = queue.pop_front() {
        let dir = match std::fs::read_dir(&dir_path) {
            Err(err) => {
                tracing::error!(
                    "Failed to scan directory '{}'. {:#}",
                    dir_path.display(),
                    err
                );
                continue;
            }
            Ok(dir) => dir,
        };
        for e in dir {
            let e = match e {
                Err(err) => {
                    tracing::error!(
                        "Failed to read entry in directory '{}'. {:#}",
                        dir_path.display(),
                        err,
                    );
                    continue;
                }
                Ok(e) => e,
            };
            let name = e.file_name();
            let path = dir_path.join(&name);
            let ft = match e.file_type() {
                Err(err) => {
                    tracing::error!("Failed to check '{}'. {:#}", path.display(), err);
                    continue;
                }
                Ok(ft) => ft,
            };

            if ft.is_dir() {
                queue.push_back(path);
            } else if ft.is_file() && SourceMeta::is_local_meta_path(&path) {
                let meta = match SourceMeta::open_local(&path) {
                    Err(err) => {
                        tracing::error!("Failed to scan meta file '{}'. {:#}", path.display(), err);
                        continue;
                    }
                    Ok(meta) => meta,
                };

                let source = meta.url();
                for (target, asset) in meta.assets() {
                    if !existing_artifacts.contains(&asset.id()) {
                        artifacts.push((
                            asset.id(),
                            AssetItem {
                                source: source.clone(),
                                format: asset.format().map(ToOwned::to_owned),
                                target: target.to_owned(),
                            },
                        ));
                    }
                }
            }
        }
    }
}

impl Loader for Store {
    #[inline]
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, Error>> {
        Box::pin(async move {
            match self.fetch(id).await {
                None => Err(Error::new(NotFound)),
                Some((path, modified)) => {
                    let bytes = std::fs::read(&path).map_err(Error::new)?;
                    Ok(AssetData {
                        bytes: bytes.into_boxed_slice(),
                        version: modified_to_version(modified),
                    })
                }
            }
        })
    }

    #[inline]
    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, Error>> {
        Box::pin(async move {
            match self.fetch(id).await {
                None => Ok(None),
                Some((path, modified)) => {
                    if modified_to_version(modified) <= version {
                        return Ok(None);
                    }
                    let bytes = std::fs::read(&path).map_err(Error::new)?;
                    Ok(Some(AssetData {
                        bytes: bytes.into_boxed_slice(),
                        version: modified_to_version(modified),
                    }))
                }
            }
        })
    }
}

#[inline]
fn modified_to_version(modified: SystemTime) -> u64 {
    modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("SystemTime must be after UNIX_EPOCH")
        .as_secs()
}
