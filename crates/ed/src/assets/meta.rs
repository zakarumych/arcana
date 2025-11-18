use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

use arcana::{Ident, Name, assets::AssetId, model::Value, smol_str::SmolStr};
use hashbrown::HashMap;
use serde::ser::SerializeSeq;
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

    #[error("Failed to read file '{meta_path}': {error}")]
    WriteError {
        error: std::io::Error,
        meta_path: PathBuf,
    },

    #[error("Failed to deserialize TOML '{meta_path}': {error}")]
    DeserializeError {
        error: serde_json::Error,
        meta_path: PathBuf,
    },

    #[error("Failed to serialize TOML '{meta_path}': {error}")]
    SerializeError {
        error: serde_json::Error,
        meta_path: PathBuf,
    },
}

/// Metadata for a single asset.
///
/// Contains information about asset file, source, format and dependencies.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetMeta {
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
                    continue;
                }
            };

            match url.scheme().parse() {
                Ok(Scheme::File) => {
                    let path = match url.to_file_path() {
                        Err(()) => {
                            tracing::error!("Invalid file URL");
                            continue;
                        }
                        Ok(path) => path,
                    };

                    let modified = match path.metadata().and_then(|meta| meta.modified()) {
                        Err(error) => {
                            tracing::error!(
                                "Failed to check how new the source file is. {:#}",
                                error
                            );
                            continue;
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
                Err(_) => tracing::error!("Unsupported scheme: '{}'", url.scheme()),
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
/// This metadata is stored in sibling file with `.arc` extension.
pub struct SourceMeta {
    meta_path: PathBuf,
    assets: HashMap<Ident, AssetMeta>,
}

impl SourceMeta {
    pub const EXTENSION: &'static str = EXTENSION;

    pub fn open_or_create(source: PathBuf) -> Result<Self, MetaError> {
        let meta_path = source_to_meta_path(source);

        let data = match std::fs::read_to_string(&*meta_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Crete file, but only if it doesn't exist yet.
                // Avoid race condition with someone else creating it in parallel.
                match std::fs::File::create_new(&*meta_path) {
                    Ok(mut file) => {
                        let new_source_meta = SourceMeta {
                            meta_path,
                            assets: HashMap::new(),
                        };

                        let string = match new_source_meta.serialize() {
                            Ok(string) => string,
                            Err(error) => return Err(error),
                        };

                        if let Err(error) = file.write_all(string.as_bytes()) {
                            return Err(MetaError::WriteError {
                                error: error,
                                meta_path: new_source_meta.meta_path,
                            });
                        }

                        return Ok(new_source_meta);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        // Ok, try read again.
                        match std::fs::read_to_string(&*meta_path) {
                            Ok(data) => data,
                            Err(error) => {
                                return Err(MetaError::ReadError {
                                    error: error,
                                    meta_path,
                                });
                            }
                        }
                    }
                    Err(error) => {
                        return Err(MetaError::WriteError {
                            error: error,
                            meta_path,
                        });
                    }
                }
            }
            Err(error) => return Err(MetaError::ReadError { error, meta_path }),
            Ok(data) => data,
        };

        SourceMeta::deserialize(&data, meta_path)
    }

    pub fn open(source: PathBuf) -> Result<Self, MetaError> {
        let meta_path = source_to_meta_path(source);

        SourceMeta::open_meta(meta_path)
    }

    pub fn open_meta(meta_path: PathBuf) -> Result<Self, MetaError> {
        match std::fs::read_to_string(&meta_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SourceMeta {
                meta_path,
                assets: HashMap::new(),
            }),
            Err(error) => Err(MetaError::ReadError {
                error,
                meta_path: meta_path.to_owned(),
            }),
            Ok(data) => SourceMeta::deserialize(&data, meta_path),
        }
    }

    pub fn meta_path(&self) -> &Path {
        &self.meta_path
    }

    pub fn source(&self) -> &Path {
        source_from_meta_path(&*self.meta_path)
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

    // Called on each modification to keep data in file up to date.
    fn flush(&self) -> Result<(), MetaError> {
        let data = self.serialize()?;

        match std::fs::write(&*self.meta_path, data.as_bytes()) {
            Ok(()) => Ok(()),
            Err(error) => Err(MetaError::WriteError {
                error: error,
                meta_path: self.meta_path.clone(),
            }),
        }
    }

    fn serialize(&self) -> Result<String, MetaError> {
        let source_meta_ser = SourceMetaSer {
            assets: &self.assets,
        };
        match serde_json::to_string_pretty(&source_meta_ser) {
            Ok(data) => Ok(data),
            Err(error) => Err(MetaError::SerializeError {
                error: error,
                meta_path: self.meta_path.clone(),
            }),
        }
    }

    fn deserialize(string: &str, meta_path: PathBuf) -> Result<Self, MetaError> {
        match serde_json::from_str::<SourceMetaDe>(string) {
            Ok(source_meta) => Ok(SourceMeta {
                meta_path,
                assets: source_meta.assets,
            }),
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
}

impl<'a> serde::Serialize for SourceMetaSer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializer = serializer.serialize_seq(Some(self.assets.len()))?;
        for (_, asset) in self.assets {
            serializer.serialize_element(asset)?;
        }
        serializer.end()
    }
}

struct SourceMetaDe {
    assets: HashMap<Ident, AssetMeta>,
}

impl<'de> serde::de::Deserialize<'de> for SourceMetaDe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(SourceMetaVisitor)
    }
}

struct SourceMetaVisitor;

impl<'de> serde::de::Visitor<'de> for SourceMetaVisitor {
    type Value = SourceMetaDe;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "Sequence or map with two elements")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<SourceMetaDe, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut assets = HashMap::new();

        while let Some(element) = seq.next_element::<AssetMeta>()? {
            assets.insert(element.target, element);
        }

        Ok(SourceMetaDe { assets })
    }
}
