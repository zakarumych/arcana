//! Asset store can import assets, save imported assets on disk and load them on demand.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::SystemTime,
};

use arcana_names::Ident;
use arcana_project::real_path;
use hashbrown::{HashMap, HashSet};
use parking_lot::RwLock;
use url::Url;

use crate::{
    assets::{
        import::{AssetDependencies, AssetSources, ImportError, Importer, ImporterId},
        AssetId,
    },
    plugin::PluginsHub,
};

mod content_address;
mod generator;
mod meta;
mod scheme;
mod sources;
mod temp;

use self::{
    generator::Generator,
    meta::{AssetMeta, MetaError, SourceMeta},
    sources::{Sources, SourcesError},
    temp::make_temporary,
};

const DEFAULT_ARTIFACTS: &'static str = "artifacts";
const DEFAULT_EXTERNAL: &'static str = "external";
const MAX_ITEM_ATTEMPTS: u32 = 1024;

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
        importer: ImporterId,
        target: Ident,
        url: Url,
        reason: String,
    },

    #[error("Too many attempts to import asset '{url}' by '{importer}'")]
    TooManyAttempts {
        importer: ImporterId,
        target: Ident,
        url: Url,
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
pub struct AssetItem {
    pub source: Url,
    pub format: Option<String>,
    pub target: Ident,
}

pub struct Store {
    base: PathBuf,
    base_url: Url,
    artifacts_base: PathBuf,
    external: PathBuf,
    temp: PathBuf,

    artifacts: RwLock<HashMap<AssetId, AssetItem>>,
    scanned: RwLock<bool>,
    id_gen: Generator,
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

        Ok(Store {
            base,
            base_url,
            artifacts_base: artifacts,
            external,
            temp,
            artifacts: RwLock::new(HashMap::new()),
            scanned: RwLock::new(false),
            id_gen: Generator::new(),
        })
    }

    /// Import an asset.
    #[tracing::instrument(skip(self, hub))]
    pub fn store(
        &self,
        source: &str,
        target: Ident,
        format: Option<&str>,
        hub: &PluginsHub,
    ) -> Result<(AssetId, PathBuf, SystemTime), StoreError> {
        let source = self
            .base_url
            .join(source)
            .map_err(|error| StoreError::InvalidSourceUrl {
                error,
                base: self.base_url.clone(),
                source: source.to_owned(),
            })?;

        self.store_from_url(source, target, format, hub)
    }

    /// Import an asset.
    #[tracing::instrument(skip(self, hub))]
    pub fn store_from_url(
        &self,
        source: Url,
        target: Ident,
        format: Option<&str>,
        hub: &PluginsHub,
    ) -> Result<(AssetId, PathBuf, SystemTime), StoreError> {
        self.ensure_scanned();

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

            let importers =
                hub.select_importers(Some(item.target), item.format.as_deref(), extension);

            if importers.is_empty() {
                return Err(StoreError::NoImporters {
                    format: item.format.clone(),
                    target: item.target,
                    url: item.source.clone(),
                });
            }
            if importers.len() > 1 {
                return Err(StoreError::ManyImporters {
                    format: item.format.clone(),
                    target: item.target,
                    url: item.source.clone(),
                });
            }

            let (importer_id, importer) = importers[0];

            // Fetch source file.
            let (source_path, source_modified) = sources
                .fetch(&self.temp, &item.source)
                .map_err(StoreError::SourcesError)?;

            let source_path = source_path.to_owned();
            let output_path = make_temporary(&self.temp);

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

            let result = importer.import(
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

            match result {
                Ok(()) => {}
                Err(ImportError::Other { reason }) => {
                    return Err(StoreError::ImportError {
                        importer: importer_id,
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
                            importer: importer_id,
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

            let new_id = AssetId(self.id_gen.generate());
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
    pub fn fetch(&self, id: AssetId, hub: &PluginsHub) -> Option<(PathBuf, SystemTime)> {
        self.ensure_scanned();

        let item = self.artifacts.read().get(&id).cloned()?;

        let (_, path, modified) = self
            .store_from_url(item.source, item.target, item.format.as_deref(), hub)
            .ok()?;

        Some((path, modified))
    }

    /// Fetch asset data path.
    pub fn find_asset(
        &self,
        source: &str,
        target: Ident,
        hub: &PluginsHub,
    ) -> Result<Option<AssetId>, StoreError> {
        self.ensure_scanned();

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
                match self.store(source, target, None, hub) {
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

    /// Select imported assets.
    /// Optionally filter by target and base URL.
    pub fn select(&self, target: Option<Ident>, base: Option<&str>) -> Vec<(AssetId, AssetItem)> {
        self.ensure_scanned();

        let base_url = base.map(|base| self.base_url.join(base).expect("Base URL must be valid"));

        self.artifacts
            .read()
            .iter()
            .filter(|(_, item)| {
                target.map_or(true, |target| item.target == target)
                    && base_url
                        .as_ref()
                        .map_or(true, |base| is_base_url(base, &item.source))
            })
            .map(|(&id, item)| (id, item.clone()))
            .collect()
    }

    fn ensure_scanned(&self) {
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

#[inline]
fn modified_to_version(modified: SystemTime) -> u64 {
    modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("SystemTime must be after UNIX_EPOCH")
        .as_secs()
}

fn is_base_url(base: &Url, url: &Url) -> bool {
    if base.scheme() != url.scheme() || base.host() != url.host() || base.port() != url.port() {
        return false;
    }

    let base_path = base.path();
    let url_path = url.path();

    if !url_path.starts_with(base_path) {
        return false;
    }

    if base_path.ends_with('/') {
        return true;
    }

    if url_path[base_path.len()..].starts_with('/') {
        return true;
    }

    false
}
