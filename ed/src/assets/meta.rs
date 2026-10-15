use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

use arcana::{Ident, Name, assets::AssetId, hash::HashMap, model::Value, smol_str::SmolStr};
use serde::ser::SerializeStruct;
use url::Url;

use crate::blobs::BlobId;

/// URL schemas supported by the store.
/// Matches should use this enum instead of matching on strings.
#[derive(Clone, Copy, Debug)]
enum Scheme {
    File,
    Data,
}

#[derive(Clone, Copy, Debug)]
struct UnsupportedScheme;

impl FromStr for Scheme {
    type Err = UnsupportedScheme;

    #[inline]
    fn from_str(s: &str) -> Result<Self, UnsupportedScheme> {
        match s {
            "file" => Ok(Scheme::File),
            "data" => Ok(Scheme::Data),
            _ => Err(UnsupportedScheme),
        }
    }
}

const EXTENSION: &'static str = "arc";
const DOT_EXTENSION: &'static str = ".arc";

/// Error that may be returned from [`SourceMeta`]'s and [`AssetMeta`]'s methods.
#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("Failed to read file '{meta_path}': {error}")]
    ReadError {
        error: std::io::Error,
        meta_path: PathBuf,
    },

    #[error("Failed to write file '{meta_path}': {error}")]
    WriteError {
        error: std::io::Error,
        meta_path: PathBuf,
    },

    #[error("Failed to deserialize JSON '{meta_path}': {error}")]
    DeserializeError {
        error: serde_json::Error,
        meta_path: PathBuf,
    },

    #[error("Failed to serialize JSON '{meta_path}': {error}")]
    SerializeError {
        error: serde_json::Error,
        meta_path: PathBuf,
    },
}

/// Metadata for a single asset.
///
/// Contains information about asset file, source, format and dependencies.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AssetMeta {
    #[serde(default)]
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importer: Option<arcana::assets::import::ImporterId>,
    /// Asset ID.
    pub id: AssetId,

    /// Target identifier of the asset.
    pub target: Ident,

    /// Blob ID of the asset file.
    pub blob: BlobId,

    /// Asset format if specified.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub format: Option<Name>,

    // Array of dependencies for this asset.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<AssetId>,

    // All additional sources of the asset and their last modified time when asset was imported.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub sources: HashMap<SmolStr, SystemTime>,

    /// Config value used for asset importing.
    #[serde(skip_serializing_if = "Value::is_unit", default)]
    pub config: Value,
}

impl AssetMeta {
    pub fn registry_version(&self) -> u64 {
        if self.version != 0 {
            return self.version;
        }
        self.latest_modified()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }

    pub fn needs_reimport(&self, base: &Url) -> bool {
        for (url, last_modified) in &self.sources {
            let url = match base.join(url) {
                Ok(url) => url,
                Err(error) => {
                    tracing::error!(
                        "Failed to figure out source URL from base: {} and source: {}. {:#}. Asset can be outdated",
                        base,
                        url,
                        error,
                    );
                    return true;
                }
            };

            match url.scheme().parse() {
                Ok(Scheme::File) => {
                    let path = match url.to_file_path() {
                        Err(()) => {
                            tracing::error!("Invalid file URL");
                            return true;
                        }
                        Ok(path) => path,
                    };

                    let modified = match path.metadata().and_then(|meta| meta.modified()) {
                        Err(error) => {
                            tracing::error!(
                                "Failed to check how new the source file is. {:#}",
                                error
                            );
                            return true;
                        }
                        Ok(modified) => modified,
                    };

                    if modified < *last_modified {
                        tracing::warn!(
                            "Source file is older than when asset was imported. Could be clock change. Reimort just in case"
                        );
                        return true;
                    }

                    if modified > *last_modified {
                        tracing::debug!("Source file was updated");
                        return true;
                    }
                }
                Ok(Scheme::Data) => continue,
                Err(_) => return true,
            }
        }

        false
    }

    pub fn latest_modified(&self) -> SystemTime {
        self.sources
            .values()
            .copied()
            .max()
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }
}

/// Metadata associated with asset source file.
/// Internal sources use sibling `.arc` files; external metadata lives in project storage.
pub struct SourceMeta {
    meta_path: PathBuf,
    source: PathBuf,
    external: bool,
    assets: HashMap<Ident, AssetMeta>,
    tags: Vec<String>,
}

impl SourceMeta {
    pub const EXTENSION: &'static str = EXTENSION;

    pub fn open_or_create(source: PathBuf) -> Result<Self, MetaError> {
        let meta = Self::open(source)?;
        if !meta.meta_path.exists() {
            meta.flush()?;
        }
        Ok(meta)
    }

    pub fn open(source: PathBuf) -> Result<Self, MetaError> {
        let meta_path = source_to_meta_path(source);

        SourceMeta::open_meta(meta_path)
    }

    pub fn open_meta(meta_path: PathBuf) -> Result<Self, MetaError> {
        Self::open_meta_at(meta_path, false)
    }

    fn open_meta_at(meta_path: PathBuf, external: bool) -> Result<Self, MetaError> {
        if external {
            reject_link(meta_path.parent().unwrap())?;
        }
        reject_link(&meta_path)?;
        match std::fs::read_to_string(&meta_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SourceMeta {
                source: source_from_meta_path(&meta_path).to_owned(),
                external: false,
                meta_path,
                assets: HashMap::default(),
                tags: Vec::new(),
            }),
            Err(error) => Err(MetaError::ReadError {
                error,
                meta_path: meta_path.to_owned(),
            }),
            Ok(data) => SourceMeta::deserialize(&data, meta_path, external),
        }
    }

    pub fn external_directory(base: &Path) -> Result<PathBuf, MetaError> {
        let path = base.join(".external");
        reject_link(&path)?;
        if path.exists() && !path.is_dir() {
            return Err(invalid_meta(
                &path,
                "External metadata storage must be a directory",
            ));
        }
        Ok(path)
    }

    pub fn open_project(base: &Path, source: PathBuf) -> Result<Self, MetaError> {
        if source.starts_with(base) {
            return Self::open(source);
        }
        if !source.is_absolute() {
            return Err(invalid_meta(&source, "External source must be absolute"));
        }
        let path = Self::external_directory(base)?.join(external_filename(&source));
        let mut meta = Self::open_meta_at(path, true)?;
        if meta.external && meta.source != source {
            return Err(invalid_meta(
                &meta.meta_path,
                "External source identity mismatch",
            ));
        }
        meta.source = source;
        meta.external = true;
        Ok(meta)
    }

    pub fn open_project_meta(base: &Path, path: PathBuf) -> Result<Self, MetaError> {
        let directory = Self::external_directory(base)?;
        if path.parent() != Some(directory.as_path()) {
            return Err(invalid_meta(&path, "Invalid external metadata directory"));
        }
        let meta = Self::open_meta_at(path, true)?;
        if !meta.external
            || !meta.source.is_absolute()
            || meta.source.starts_with(base)
            || meta.meta_path.file_name() != Some(OsStr::new(&external_filename(&meta.source)))
        {
            return Err(invalid_meta(
                &meta.meta_path,
                "Invalid external source identity",
            ));
        }
        Ok(meta)
    }

    pub fn meta_path(&self) -> &Path {
        &self.meta_path
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn get_asset(&self, target: Ident) -> Option<&AssetMeta> {
        self.assets.get(&target)
    }

    pub fn assets(&self) -> impl Iterator<Item = &AssetMeta> + '_ {
        self.assets.iter().map(|(_, meta)| meta)
    }

    pub fn add_asset(&mut self, asset: AssetMeta) -> Result<(), MetaError> {
        let target = asset.target;
        let old = self.assets.insert(target, asset);

        match self.flush() {
            Ok(()) => Ok(()),
            Err(error) => {
                // Restore state on failure
                match old {
                    None => {
                        self.assets.remove(&target);
                    }
                    Some(old) => {
                        self.assets.insert(target, old);
                    }
                }
                Err(error)
            }
        }
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Removes imported targets while preserving source identity and tags.
    pub fn clear_assets(&mut self) -> Result<Vec<AssetId>, MetaError> {
        let assets = std::mem::take(&mut self.assets);
        if assets.is_empty() {
            return Ok(Vec::new());
        }
        if let Err(error) = self.flush() {
            self.assets = assets;
            return Err(error);
        }
        Ok(assets.into_values().map(|asset| asset.id).collect())
    }

    pub fn set_tags(&mut self, tags: Vec<String>) -> Result<(), MetaError> {
        let mut tags: Vec<_> = tags
            .into_iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        tags.sort();
        tags.dedup();
        let old = std::mem::replace(&mut self.tags, tags);
        if let Err(error) = self.flush() {
            self.tags = old;
            return Err(error);
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), MetaError> {
        if self.external {
            let parent = self.meta_path.parent().unwrap();
            reject_link(parent)?;
            std::fs::create_dir(parent)
                .or_else(|e| {
                    if e.kind() == std::io::ErrorKind::AlreadyExists {
                        Ok(())
                    } else {
                        Err(e)
                    }
                })
                .map_err(|error| MetaError::WriteError {
                    error,
                    meta_path: parent.to_owned(),
                })?;
            reject_link(parent)?;
        }
        reject_link(&self.meta_path)?;
        let data = self.serialize()?;
        let temporary = self.meta_path.with_file_name(format!(
            ".{}.{}.tmp",
            self.meta_path.file_name().unwrap().to_string_lossy(),
            rand::random::<u128>()
        ));
        let result = (|| {
            let mut file = std::fs::File::create_new(&temporary)?;
            file.write_all(data.as_bytes())?;
            file.sync_all()?;
            drop(file);
            // A failed replacement leaves the destination intact, including on Windows.
            std::fs::rename(&temporary, &self.meta_path)
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temporary);
            return Err(MetaError::WriteError {
                error,
                meta_path: self.meta_path.clone(),
            });
        }
        Ok(())
    }

    fn serialize(&self) -> Result<String, MetaError> {
        let source_meta_ser = SourceMetaSer {
            assets: &self.assets,
            tags: &self.tags,
            source: self.external.then_some(self.source.as_path()),
        };
        match serde_json::to_string_pretty(&source_meta_ser) {
            Ok(data) => Ok(data),
            Err(error) => Err(MetaError::SerializeError {
                error: error,
                meta_path: self.meta_path.clone(),
            }),
        }
    }

    fn deserialize(string: &str, meta_path: PathBuf, external: bool) -> Result<Self, MetaError> {
        match serde_json::from_str::<SourceMetaDe>(string) {
            Ok(source_meta) => {
                let source = if external {
                    source_meta.source.ok_or_else(|| {
                        invalid_meta(&meta_path, "External metadata needs source path")
                    })?
                } else {
                    source_from_meta_path(&meta_path).to_owned()
                };
                if external
                    && (!source.is_absolute()
                        || meta_path.file_name() != Some(OsStr::new(&external_filename(&source))))
                {
                    return Err(invalid_meta(&meta_path, "Invalid external source identity"));
                }
                Ok(SourceMeta {
                    source,
                    external,
                    meta_path,
                    assets: source_meta.assets,
                    tags: source_meta.tags,
                })
            }
            Err(error) => Err(MetaError::DeserializeError {
                error: error,
                meta_path,
            }),
        }
    }
}

fn source_to_meta_path(source: PathBuf) -> PathBuf {
    let mut path = source.into_os_string();
    path.push(DOT_EXTENSION);
    PathBuf::from(path)
}

fn source_from_meta_path(meta_path: &Path) -> &Path {
    const {
        assert!(DOT_EXTENSION.is_ascii());
    }

    let string = meta_path.as_os_str();
    match string
        .as_encoded_bytes()
        .strip_suffix(DOT_EXTENSION.as_bytes())
    {
        None => {
            panic!(
                "Invalid meta path {}, Valid meta path must end with {}",
                meta_path.display(),
                DOT_EXTENSION,
            )
        }
        Some(prefix) => {
            // SAFETY:
            // - Prefix is substring of slice obtained from `OsStr::as_encoded_bytes`
            // - Suffix stripped is non-empty UTF-8 substring
            let prefix_string = unsafe { OsStr::from_encoded_bytes_unchecked(prefix) };

            Path::new(prefix_string)
        }
    }
}

struct SourceMetaSer<'a> {
    assets: &'a HashMap<Ident, AssetMeta>,
    tags: &'a [String],
    source: Option<&'a Path>,
}

impl serde::Serialize for SourceMetaSer<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut serializer =
            serializer.serialize_struct("SourceMeta", if self.source.is_some() { 3 } else { 2 })?;
        let assets: Vec<_> = self.assets.values().collect();
        serializer.serialize_field("assets", &assets)?;
        serializer.serialize_field("tags", self.tags)?;
        if let Some(source) = self.source {
            serializer.serialize_field("source", source)?;
        }
        serializer.end()
    }
}

struct SourceMetaDe {
    source: Option<PathBuf>,
    assets: HashMap<Ident, AssetMeta>,
    tags: Vec<String>,
}

impl<'de> serde::Deserialize<'de> for SourceMetaDe {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Representation {
            Legacy(Vec<AssetMeta>),
            Current {
                #[serde(default)]
                source: Option<PathBuf>,
                #[serde(default)]
                assets: Vec<AssetMeta>,
                #[serde(default)]
                tags: Vec<String>,
            },
        }
        let (assets, mut tags, source) = match Representation::deserialize(deserializer)? {
            Representation::Legacy(assets) => (assets, Vec::new(), None),
            Representation::Current {
                assets,
                tags,
                source,
            } => (assets, tags, source),
        };
        tags = tags
            .into_iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        tags.sort();
        tags.dedup();
        Ok(SourceMetaDe {
            source,
            assets: assets
                .into_iter()
                .map(|asset| (asset.target, asset))
                .collect(),
            tags,
        })
    }
}

fn reject_link(path: &Path) -> Result<(), MetaError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if is_link(&metadata) => Err(MetaError::ReadError {
            error: std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Asset metadata cannot be a symbolic link or reparse point",
            ),
            meta_path: path.to_owned(),
        }),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(MetaError::ReadError {
            error,
            meta_path: path.to_owned(),
        }),
    }
}

pub(crate) fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn external_filename(source: &Path) -> String {
    format!(
        "{:x}.arc",
        arcana::hash::sha256(source.as_os_str().as_encoded_bytes())
    )
}
fn invalid_meta(path: &Path, message: &str) -> MetaError {
    MetaError::ReadError {
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, message),
        meta_path: path.to_owned(),
    }
}
