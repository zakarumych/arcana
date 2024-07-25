use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::SystemTime,
};

use argosy_id::AssetId;
use argosy_import::{loading::LoadingError, ImportError, Importer};
use futures::future::BoxFuture;
use hashbrown::{HashMap, HashSet};
use parking_lot::RwLock;
use url::Url;

use crate::{
    gen::Generator,
    importer::Importers,
    meta::{AssetMeta, MetaError, SourceMeta},
    sources::{Sources, SourcesError},
    temp::make_temporary,
};

pub const ARGOSY_META_NAME: &'static str = "argosy.toml";

const DEFAULT_AUX: &'static str = "argosy";
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
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub importers: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenStoreError {
    #[error("Failed to find current working directory. {error}")]
    NoCwd { error: std::io::Error },

    #[error("Failed to find store metadata file in ancestors of '{path}'")]
    NotFound { path: PathBuf },

    #[error("Error: '{error}' while trying to canonicalize path '{path}'")]
    CanonError {
        #[source]
        error: std::io::Error,
        path: PathBuf,
    },

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
    #[error("Failed to construct URL from base '{base}' and source '{url}'. {error}")]
    InvalidSourceUrl {
        error: url::ParseError,
        base: Url,
        url: String,
    },

    #[error(transparent)]
    MetaError(MetaError),

    #[error("Failed to find importer '{url}':'{format:?}->{target}'")]
    NoImporters {
        format: Option<String>,
        target: String,
        url: Url,
    },

    #[error(
        "Multiple importers may import '{url}' from different formats '{formats:?}' to target '{target}'"
    )]
    AmbiguousImporters {
        formats: Vec<String>,
        target: String,
        url: Url,
    },

    #[error(transparent)]
    SourcesError(SourcesError),

    #[error("Failed to import asset '{url}':'{format:?}->{target}'. {reason}")]
    ImportError {
        format: Option<String>,
        target: String,
        url: Url,
        reason: String,
    },

    #[error("Too many attempts to import asset '{url}':'{format:?}->{target}'")]
    TooManyAttempts {
        format: Option<String>,
        target: String,
        url: Url,
    },

    #[error("Failed to create directory '{path}' to store import artifacts. {error}")]
    FailedToCreateArtifactsDirectory {
        error: std::io::Error,
        path: PathBuf,
    },
}

impl Default for StoreInfo {
    fn default() -> Self {
        StoreInfo::new(None, None, None, &[])
    }
}

impl StoreInfo {
    pub fn write(&self, path: &Path) -> Result<(), SaveStoreError> {
        let meta =
            toml::to_string_pretty(self).map_err(|error| SaveStoreError::MetaSerializeError {
                error,
                path: path.to_owned(),
            })?;
        std::fs::write(path, &meta).map_err(|error| SaveStoreError::MetaWriteError {
            error,
            path: path.to_owned(),
        })?;
        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self, OpenStoreError> {
        let meta =
            std::fs::read_to_string(path).map_err(|error| OpenStoreError::MetaReadError {
                error,
                path: path.to_owned(),
            })?;
        let meta: StoreInfo =
            toml::from_str(&meta).map_err(|error| OpenStoreError::MetaDeserializeError {
                error,
                path: path.to_owned(),
            })?;
        Ok(meta)
    }

    pub fn new(
        artifacts: Option<&Path>,
        external: Option<&Path>,
        temp: Option<&Path>,
        importers: &[&Path],
    ) -> Self {
        let artifacts = artifacts.map(Path::to_owned);
        let external = external.map(Path::to_owned);
        let temp = temp.map(Path::to_owned);
        let importers = importers.iter().copied().map(|p| p.to_owned()).collect();

        StoreInfo {
            artifacts,
            external,
            temp,
            importers,
        }
    }
}

#[derive(Clone)]
struct AssetItem {
    source: Url,
    format: Option<String>,
    target: String,
}

pub struct Store {
    base: PathBuf,
    base_url: Url,
    artifacts_base: PathBuf,
    external: PathBuf,
    temp: PathBuf,
    importers: Importers,

    artifacts: RwLock<HashMap<AssetId, AssetItem>>,
    scanned: RwLock<bool>,
    id_gen: Generator,
}

impl Store {
    /// Find and open store in ancestors of specified directory.
    #[tracing::instrument]
    pub fn find(path: &Path) -> Result<Self, OpenStoreError> {
        let path = dunce::canonicalize(path).map_err(|error| OpenStoreError::CanonError {
            error,
            path: path.to_owned(),
        })?;

        let meta_path = find_argosy_info(&path).ok_or_else(|| OpenStoreError::NotFound { path })?;

        Store::open(&meta_path)
    }

    /// Find and open store in ancestors of current directory.
    #[tracing::instrument]
    pub fn find_current() -> Result<Self, OpenStoreError> {
        let cwd = std::env::current_dir().map_err(|error| OpenStoreError::NoCwd { error })?;
        Store::find(&cwd)
    }

    /// Open store database at specified path.
    #[tracing::instrument]
    pub fn open(path: &Path) -> Result<Self, OpenStoreError> {
        let meta = StoreInfo::read(path)?;
        let base = path.parent().unwrap().to_owned();

        Self::new(&base, meta)
    }

    pub fn new(base: &Path, meta: StoreInfo) -> Result<Self, OpenStoreError> {
        let base = dunce::canonicalize(base).map_err(|error| OpenStoreError::CanonError {
            error,
            path: base.to_owned(),
        })?;
        let base_url =
            Url::from_directory_path(&base).expect("Canonical path must be convertible to URL");

        let artifacts = base.join(
            meta.artifacts
                .unwrap_or_else(|| Path::new(DEFAULT_AUX).join(DEFAULT_ARTIFACTS)),
        );

        let external = base.join(
            meta.external
                .unwrap_or_else(|| Path::new(DEFAULT_AUX).join(DEFAULT_EXTERNAL)),
        );

        let temp = meta
            .temp
            .map_or_else(std::env::temp_dir, |path| base.join(path));

        let mut importers = Importers::new();

        for lib_path in &meta.importers {
            let lib_path = base.join(lib_path);

            unsafe {
                // # Safety: Nope.
                // There is no way to make this safe.
                // But it is unlikely to cause problems by accident.
                if let Err(err) = importers.load_dylib_importers(&lib_path) {
                    tracing::error!(
                        "Failed to load importers from '{}'. {:#}",
                        lib_path.display(),
                        err
                    );
                }
            }
        }

        Ok(Store {
            base,
            base_url,
            artifacts_base: artifacts,
            external,
            temp,
            importers,
            artifacts: RwLock::new(HashMap::new()),
            scanned: RwLock::new(false),
            id_gen: Generator::new(),
        })
    }

    /// Register importer.
    #[tracing::instrument(skip(self), fields(importer = %importer.name()))]
    pub fn register_importer(&mut self, importer: Box<dyn Importer>) {
        self.importers.add_importer(importer);
    }

    /// Loads importers from dylib.
    /// There is no possible way to guarantee that dylib does not break safety contracts.
    /// Some measures to ensure safety are taken.
    /// Providing dylib from which importers will be successfully loaded and then cause an UB should only be possible on purpose.
    #[tracing::instrument(skip(self))]
    pub unsafe fn register_importers_lib(&mut self, lib_path: &Path) -> Result<(), LoadingError> {
        self.importers.load_dylib_importers(lib_path)
    }

    /// Import an asset.
    #[tracing::instrument(skip(self))]
    pub async fn store(
        &self,
        source: &str,
        format: Option<&str>,
        target: &str,
    ) -> Result<(AssetId, PathBuf, SystemTime), StoreError> {
        let source = self
            .base_url
            .join(source)
            .map_err(|error| StoreError::InvalidSourceUrl {
                error,
                base: self.base_url.clone(),
                url: source.to_owned(),
            })?;

        self.store_url(source, format, target).await
    }

    /// Import an asset.
    #[tracing::instrument(skip(self))]
    pub async fn store_url(
        &self,
        source: Url,
        format: Option<&str>,
        target: &str,
    ) -> Result<(AssetId, PathBuf, SystemTime), StoreError> {
        let mut sources = Sources::new();

        let base = &self.base;
        let artifacts_base = &self.artifacts_base;
        let external = &self.external;
        let importers = &self.importers;

        struct StackItem {
            /// Source URL.
            source: Url,

            /// Source format name.
            format: Option<String>,

            /// Target format name.
            target: String,

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
            format: format.map(str::to_owned),
            target: target.to_owned(),
            attempt: 0,
            sources: HashMap::new(),
            dependencies: HashSet::new(),
        });

        loop {
            let item = stack.last_mut().unwrap();
            item.attempt += 1;

            let mut meta = SourceMeta::new(&item.source, &self.base, &self.external)
                .map_err(StoreError::MetaError)?;

            if let Some(asset) = meta.get_asset(&item.target) {
                if asset.needs_reimport(&self.base_url) {
                    tracing::debug!(
                        "'{}' '{:?}' '{}' reimporting",
                        item.source,
                        item.format,
                        item.target
                    );
                } else {
                    match &item.format {
                        None => tracing::debug!("{} @ '{}'", item.target, item.source),
                        Some(format) => {
                            tracing::debug!("{} as {} @ '{}'", item.target, format, item.source)
                        }
                    }

                    stack.pop().unwrap();
                    if stack.is_empty() {
                        let path = asset.artifact_path(&self.artifacts_base);
                        return Ok((asset.id(), path, asset.latest_modified()));
                    }
                    continue;
                }
            }

            let importer = importers
                .guess(item.format.as_deref(), url_ext(&item.source), &item.target)
                .map_err(|err| StoreError::AmbiguousImporters {
                    formats: err.formats,
                    target: err.target,
                    url: item.source.clone(),
                })?;

            let importer = importer.ok_or_else(|| StoreError::NoImporters {
                format: item.format.clone(),
                target: item.target.clone(),
                url: item.source.clone(),
            })?;

            // Fetch source file.
            let (source_path, source_modified) = sources
                .fetch(&self.temp, &item.source)
                .await
                .map_err(StoreError::SourcesError)?;

            let source_path = source_path.to_owned();
            let output_path = make_temporary(&self.temp);

            struct Fn<F>(F);

            impl<F> argosy_import::Sources for Fn<F>
            where
                F: FnMut(&str) -> Option<PathBuf>,
            {
                fn get(&mut self, source: &str) -> Option<PathBuf> {
                    (self.0)(source)
                }
            }

            impl<F> argosy_import::Dependencies for Fn<F>
            where
                F: FnMut(&str, &str) -> Option<AssetId>,
            {
                fn get(&mut self, source: &str, target: &str) -> Option<AssetId> {
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
                &mut Fn(|src: &str, target: &str| {
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
                        format: item.format.clone(),
                        target: item.target.clone(),
                        url: item.source.clone(),
                        reason,
                    });
                }
                Err(ImportError::Requires {
                    sources: srcs,
                    dependencies: deps,
                }) => {
                    if item.attempt >= MAX_ITEM_ATTEMPTS {
                        return Err(StoreError::TooManyAttempts {
                            format: item.format.clone(),
                            target: item.target.clone(),
                            url: item.source.clone(),
                        });
                    }
                    let item_source = item.source.clone();

                    for src in srcs {
                        match item_source.join(&src) {
                            Err(error) => {
                                return Err(StoreError::InvalidSourceUrl {
                                    error,
                                    base: item_source,
                                    url: src.clone(),
                                });
                            }
                            Ok(url) => sources
                                .fetch(&self.temp, &url)
                                .await
                                .map_err(StoreError::SourcesError)?,
                        };
                    }

                    for dep in deps {
                        match item_source.join(&dep.source) {
                            Err(error) => {
                                return Err(StoreError::InvalidSourceUrl {
                                    error,
                                    base: item_source,
                                    url: dep.source.clone(),
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
                std::fs::create_dir_all(artifacts_base).map_err(|error| {
                    StoreError::FailedToCreateArtifactsDirectory {
                        error,
                        path: artifacts_base.to_owned(),
                    }
                })?;

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
            meta.add_asset(item.target.clone(), asset, base, external)
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
            .store_url(item.source, item.format.as_deref(), &item.target)
            .await
            .ok()?;

        Some((path, modified))
    }

    /// Fetch asset data path.
    pub async fn find_asset(
        &self,
        source: &str,
        target: &str,
    ) -> Result<Option<AssetId>, StoreError> {
        let source_url =
            self.base_url
                .join(source)
                .map_err(|error| StoreError::InvalidSourceUrl {
                    error,
                    base: self.base_url.clone(),
                    url: source.to_owned(),
                })?;

        let meta = SourceMeta::new(&source_url, &self.base, &self.external)
            .map_err(StoreError::MetaError)?;

        match meta.get_asset(target) {
            None => {
                drop(meta);
                match self.store(source, None, target).await {
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

pub fn find_argosy_info(path: &Path) -> Option<PathBuf> {
    for path in path.ancestors() {
        let candidate = path.join(ARGOSY_META_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn url_ext(url: &Url) -> Option<&str> {
    let path = url.path();
    let dot = path.rfind('.')?;
    let sep = path.rfind('/')?;
    if dot == path.len() || dot <= sep + 1 {
        None
    } else {
        Some(&path[dot + 1..])
    }
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

impl argosy::Source for Store {
    #[inline]
    fn find<'a>(&'a self, key: &'a str, asset: &'a str) -> BoxFuture<'a, Option<AssetId>> {
        Box::pin(async move {
            match self.find_asset(&key, &asset).await {
                Err(err) => {
                    tracing::error!("Error while searching for asset '{asset} @ {key}': {err}");
                    None
                }
                Ok(None) => None,
                Ok(Some(id)) => Some(id),
            }
        })
    }

    #[inline]
    fn load<'a>(
        &'a self,
        id: AssetId,
    ) -> BoxFuture<'a, Result<Option<argosy::AssetData>, argosy::Error>> {
        Box::pin(async move {
            match self.fetch(id).await {
                None => Ok(None),
                Some((path, modified)) => {
                    let bytes = std::fs::read(&path).map_err(argosy::Error::new)?;
                    Ok(Some(argosy::AssetData {
                        bytes: bytes.into_boxed_slice(),
                        version: modified_to_version(modified),
                    }))
                }
            }
        })
    }

    #[inline]
    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<argosy::AssetData>, argosy::Error>> {
        Box::pin(async move {
            match self.fetch(id).await {
                None => Ok(None),
                Some((path, modified)) => {
                    if modified_to_version(modified) <= version {
                        return Ok(None);
                    }
                    let bytes = std::fs::read(&path).map_err(argosy::Error::new)?;
                    Ok(Some(argosy::AssetData {
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
