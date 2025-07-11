use std::{
    path::{absolute, Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

use arcana::{assets::AssetId, hash::sha256, smol_str::SmolStr, Ident, Name};
use hashbrown::HashMap;
use url::Url;

use crate::blobs::{BlobId, Blobs};

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

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("Failed to calculate hash of the file '{path}': {error}")]
    HashError {
        error: std::io::Error,
        path: PathBuf,
    },

    #[error("Failed to save artifact file")]
    SaveArtifactError {
        error: std::io::Error,
        path: PathBuf,
    },

    #[error("Failed to get real path of '{path}'")]
    PathError { path: PathBuf },

    #[error("Failed to convert path '{path}' to URL")]
    UrlFromPathError { path: PathBuf },

    #[error("Failed to read file '{path}': {error}")]
    ReadError {
        error: std::io::Error,
        path: PathBuf,
    },

    #[error("Failed to read file '{path}': {error}")]
    WriteError {
        error: std::io::Error,
        path: PathBuf,
    },

    #[error("Failed to deserialize TOML '{path}': {error}")]
    DeserializeError {
        error: serde_json::Error,
        path: PathBuf,
    },

    #[error("Failed to serialize TOML '{path}': {error}")]
    SerializeError {
        error: serde_json::Error,
        path: PathBuf,
    },

    #[error("Failed to create directory '{path}': {error}")]
    CreateDirError {
        error: std::io::Error,
        path: PathBuf,
    },
}

/// Metadata for a single asset.
///
/// Contains information about asset file, source, format and dependencies.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetMeta {
    /// Asset ID.
    id: AssetId,

    /// Target identifier of the asset.
    target: Ident,

    /// Blob ID of the asset file.
    blob: BlobId,

    /// Asset format if specified.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    format: Option<Name>,

    // Array of dependencies for this asset.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    dependencies: Vec<AssetId>,

    // All additional sources of the asset and their last modified time when asset was imported.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    sources: HashMap<SmolStr, SystemTime>,
}

impl AssetMeta {
    /// Creates new asset metadata.
    /// Puts output to the artifacts directory.
    ///
    /// This function is when new asset is imported.
    ///
    /// `output` contain temporary path to imported asset artifact.
    /// `artifacts` is path to artifact directory.
    ///
    /// Filename of the output gets chosen using first N characters of the sha512 hash.
    /// Where N is the minimal length required to avoid collisions between files with same hash prefixes.
    /// It can also get a suffix if there is a complete hash collision.
    ///
    /// If artifact with the same hash already exists in the `artifacts` directory,
    /// it will be shared between assets.
    pub fn new(
        id: AssetId,
        blob: BlobId,
        target: Ident,
        format: Option<Name>,
        sources: Vec<(SmolStr, SystemTime)>,
        dependencies: Vec<AssetId>,
        output: &Path,
    ) -> Self {
        AssetMeta {
            id,
            target,
            format,
            blob,
            sources: sources.into_iter().collect(),
            dependencies,
        }
    }

    pub fn id(&self) -> AssetId {
        self.id
    }

    pub fn blob(&self) -> BlobId {
        self.blob
    }

    pub fn format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    pub fn needs_reimport(&self, base: &Url) -> bool {
        for (url, last_modified) in &self.sources {
            let url = match base.join(url) {
                Ok(url) => url,
                Err(err) => {
                    tracing::error!(
                        "Failed to figure out source URL from base: {} and source: {}. {:#}. Asset can be outdated",
                        base,
                        url,
                        err,
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
                        Err(err) => {
                            tracing::error!(
                                "Failed to check how new the source file is. {:#}",
                                err
                            );
                            continue;
                        }
                        Ok(modified) => modified,
                    };

                    if modified < *last_modified {
                        tracing::warn!("Source file is older than when asset was imported. Could be clock change. Reimort just in case");
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
/// This metadata is stored in sibling file with `.argosy` extension.
/// Or in 'external' directory if source is not in the base directory or
/// one of its subdirectories. Or if source is not a file.
pub struct SourceMeta {
    source: PathBuf,
    assets: HashMap<Ident, AssetMeta>,
}

impl SourceMeta {
    pub fn open_or_create(source: PathBuf) -> Result<SourceMeta, MetaError> {
        SourceMeta::new(source, true)
    }

    pub fn open(source: PathBuf) -> Result<SourceMeta, MetaError> {
        SourceMeta::new(source, false)
    }

    fn new(source: PathBuf, allow_missing: bool) -> Result<Self, MetaError> {
        let meta_path = source_to_meta_path(source.clone());

        match std::fs::read_to_string(&meta_path) {
            Err(err) if allow_missing && err.kind() == std::io::ErrorKind::NotFound => {
                Ok(SourceMeta {
                    source,
                    assets: HashMap::new(),
                })
            }
            Err(error) => Err(MetaError::ReadError {
                error,
                path: meta_path.to_owned(),
            }),
            Ok(data) => {
                let assets =
                    serde_json::from_str(&data).map_err(|error| MetaError::DeserializeError {
                        error,
                        path: meta_path.to_owned(),
                    })?;
                Ok(SourceMeta { source, assets })
            }
        }
    }

    pub fn get_asset(&self, target: Ident) -> Option<&AssetMeta> {
        self.assets.get(&target)
    }

    pub fn assets(&self) -> impl Iterator<Item = (Ident, &AssetMeta)> + '_ {
        self.assets.iter().map(|(target, meta)| (*target, meta))
    }

    pub fn add_asset(&mut self, target: Ident, asset: AssetMeta) -> Result<(), MetaError> {
        self.assets.insert(target, asset);

        let path = &self.source;

        let data = serde_json::to_string_pretty(&self.assets).map_err(|error| {
            MetaError::SerializeError {
                error,
                path: path.to_owned(),
            }
        })?;

        std::fs::write(path, data.as_bytes()).map_err(|error| MetaError::WriteError {
            error,
            path: path.to_owned(),
        })?;

        Ok(())
    }
}

fn source_to_meta_path(source: PathBuf) -> PathBuf {
    let mut filename = source.file_name().unwrap_or("".as_ref()).to_owned();
    filename.push(DOT_EXTENSION);
    source.with_file_name(filename)
}
