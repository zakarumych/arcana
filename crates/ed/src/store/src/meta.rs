use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use argosy_id::AssetId;
use hashbrown::HashMap;
use url::Url;

use crate::{
    content_address::{move_file_with_content_address, with_path_candidates, PREFIX_STARTING_LEN},
    scheme::Scheme,
    sha256::Sha256Hash,
};

const EXTENSION: &'static str = "argosy";
const DOT_EXTENSION: &'static str = ".argosy";

/// Metadata for single asset.
///
/// Contains information about asset file, source, format and dependencies.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetMeta {
    /// Asset ID.
    id: AssetId,

    /// Imported asset file hash.
    sha256: Sha256Hash,

    /// Asset format if specified.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    format: Option<String>,

    /// Minimal length of the hash prefix required to avoid collisions between files with same hash prefixes.
    #[serde(skip_serializing_if = "prefix_is_default", default = "default_prefix")]
    path_len: u64,

    // Array of dependencies for this asset.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    dependencies: Vec<AssetId>,

    // Maps source URL to last modified time.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    sources: HashMap<String, SystemTime>,
}

fn prefix_is_default(prefix: &u64) -> bool {
    default_prefix() == *prefix
}

const fn default_prefix() -> u64 {
    PREFIX_STARTING_LEN as u64
}

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

    #[error("Failed to rename file '{from}' to '{to}': {error}")]
    RenameError {
        error: std::io::Error,
        from: PathBuf,
        to: PathBuf,
    },

    #[error("Failed to compare files '{path1}' and '{path2}': {error}")]
    CompareError {
        error: std::io::Error,
        path1: PathBuf,
        path2: PathBuf,
    },

    #[error("Path '{path}' is occupied by non-file")]
    PathOccupiedByDirectory { path: PathBuf },

    #[error("Error: '{error}' while trying to canonicalize path '{path}'")]
    CanonError {
        #[source]
        error: std::io::Error,
        path: PathBuf,
    },

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
        error: toml::de::Error,
        path: PathBuf,
    },

    #[error("Failed to serialize TOML '{path}': {error}")]
    SerializeError {
        error: toml::ser::Error,
        path: PathBuf,
    },

    #[error("Failed to create directory '{path}': {error}")]
    CreateDirError {
        error: std::io::Error,
        path: PathBuf,
    },
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
        format: Option<String>,
        sources: Vec<(String, SystemTime)>,
        dependencies: Vec<AssetId>,
        output: &Path,
        artifacts: &Path,
    ) -> Result<Self, MetaError> {
        let sha256 = Sha256Hash::file_hash(output).map_err(|error| MetaError::HashError {
            error,
            path: output.to_owned(),
        })?;

        let hex = format!("{:x}", sha256);

        let (_, path_len) =
            move_file_with_content_address(&hex, output, artifacts).map_err(|error| {
                MetaError::SaveArtifactError {
                    path: output.to_owned(),
                    error,
                }
            })?;

        Ok(AssetMeta {
            id,
            format,
            sha256,
            path_len,
            sources: sources.into_iter().collect(),
            dependencies,
        })
    }

    pub fn id(&self) -> AssetId {
        self.id
    }

    pub fn format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    pub fn needs_reimport(&self, base: &Url) -> bool {
        for (url, last_modified) in &self.sources {
            let url = match base.join(url) {
                Err(err) => {
                    tracing::error!(
                        "Failed to figure out source URL from base: {} and source: {}. {:#}. Asset can be outdated",
                        base,
                        url,
                        err,
                    );
                    continue;
                }
                Ok(url) => url,
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

    /// Returns path to the artifact.
    pub fn artifact_path(&self, artifacts: &Path) -> PathBuf {
        let hex = format!("{:x}", self.sha256);

        if self.path_len <= hex.len() as u64 {
            let prefix = &hex[..self.path_len as usize];
            artifacts.join(prefix)
        } else {
            artifacts.join(format!("{}:{}", hex, self.path_len - hex.len() as u64))
        }
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
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SourceMeta {
    url: Url,
    assets: HashMap<String, AssetMeta>,
}

impl SourceMeta {
    /// Finds and returns meta for the source URL.
    /// Creates new file if needed.
    pub fn new(source: &Url, base: &Path, external: &Path) -> Result<SourceMeta, MetaError> {
        let (meta_path, is_external) = get_meta_path(source, base, external)?;

        if is_external {
            SourceMeta::new_external(&meta_path, source)
        } else {
            SourceMeta::new_local(&meta_path)
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn is_local_meta_path(meta_path: &Path) -> bool {
        meta_path.extension().map_or(false, |e| e == EXTENSION)
    }

    pub fn new_local(meta_path: &Path) -> Result<SourceMeta, MetaError> {
        SourceMeta::read_local(meta_path, true)
    }

    pub fn open_local(meta_path: &Path) -> Result<SourceMeta, MetaError> {
        SourceMeta::read_local(meta_path, false)
    }

    fn read_local(meta_path: &Path, allow_missing: bool) -> Result<Self, MetaError> {
        let source_path = meta_path.with_extension("");
        let url = Url::from_file_path(&source_path)
            .map_err(|()| MetaError::UrlFromPathError { path: source_path })?;

        match std::fs::read_to_string(meta_path) {
            Err(err) if allow_missing && err.kind() == std::io::ErrorKind::NotFound => {
                Ok(SourceMeta {
                    url,
                    assets: HashMap::new(),
                })
            }
            Err(error) => Err(MetaError::ReadError {
                error,
                path: meta_path.to_owned(),
            }),
            Ok(data) => {
                let assets =
                    toml::from_str(&data).map_err(|error| MetaError::DeserializeError {
                        error,
                        path: meta_path.to_owned(),
                    })?;
                Ok(SourceMeta { url, assets })
            }
        }
    }

    pub fn new_external(meta_path: &Path, source: &Url) -> Result<SourceMeta, MetaError> {
        match std::fs::read_to_string(meta_path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(SourceMeta {
                url: source.clone(),
                assets: HashMap::new(),
            }),
            Err(error) => Err(MetaError::ReadError {
                error,
                path: meta_path.to_owned(),
            }),
            Ok(data) => {
                let assets =
                    toml::from_str(&data).map_err(|error| MetaError::DeserializeError {
                        error,
                        path: meta_path.to_owned(),
                    })?;
                Ok(SourceMeta {
                    url: source.clone(),
                    assets,
                })
            }
        }
    }

    pub fn open_external(meta_path: &Path) -> Result<SourceMeta, MetaError> {
        match std::fs::read_to_string(meta_path) {
            Err(error) => Err(MetaError::ReadError {
                error,
                path: meta_path.to_owned(),
            }),
            Ok(data) => {
                let meta = toml::from_str(&data).map_err(|error| MetaError::DeserializeError {
                    error,
                    path: meta_path.to_owned(),
                })?;
                Ok(meta)
            }
        }
    }

    pub fn get_asset(&self, target: &str) -> Option<&AssetMeta> {
        self.assets.get(target)
    }

    pub fn assets(&self) -> impl Iterator<Item = (&str, &AssetMeta)> + '_ {
        self.assets.iter().map(|(target, meta)| (&**target, meta))
    }

    pub fn add_asset(
        &mut self,
        target: String,
        asset: AssetMeta,
        base: &Path,
        external: &Path,
    ) -> Result<(), MetaError> {
        self.assets.insert(target, asset);

        let (meta_path, is_external) = get_meta_path(&self.url, base, external)?;
        if is_external {
            self.write_with_url_to(&meta_path)?;
        } else {
            self.write_to(&meta_path)?;
        }
        Ok(())
    }

    fn write_to(&self, path: &Path) -> Result<(), MetaError> {
        let data =
            toml::to_string_pretty(&self.assets).map_err(|error| MetaError::SerializeError {
                error,
                path: path.to_owned(),
            })?;
        std::fs::write(path, data.as_bytes()).map_err(|error| MetaError::WriteError {
            error,
            path: path.to_owned(),
        })?;
        Ok(())
    }

    fn write_with_url_to(&self, path: &Path) -> Result<(), MetaError> {
        let data = toml::to_string_pretty(self).map_err(|error| MetaError::SerializeError {
            error,
            path: path.to_owned(),
        })?;
        std::fs::write(path, data.as_bytes()).map_err(|error| MetaError::WriteError {
            error,
            path: path.to_owned(),
        })?;
        Ok(())
    }
}

/// Finds and returns meta for the source URL.
/// Creates new file if needed.
fn get_meta_path(source: &Url, base: &Path, external: &Path) -> Result<(PathBuf, bool), MetaError> {
    if source.scheme() == "file" {
        match source.to_file_path() {
            Ok(path) => {
                let path = dunce::canonicalize(&path)
                    .map_err(|err| MetaError::CanonError { error: err, path })?;

                if path.starts_with(base) {
                    // Files inside `base` directory has meta attached to them as sibling file with `.argosy` extension added.

                    let mut filename = path.file_name().unwrap_or("".as_ref()).to_owned();
                    filename.push(DOT_EXTENSION);

                    let path = path.with_file_name(filename);
                    return Ok((path, false));
                }
            }
            Err(()) => {}
        }
    }

    std::fs::create_dir_all(external).map_err(|error| MetaError::CreateDirError {
        error,
        path: external.to_owned(),
    })?;

    let hash = Sha256Hash::hash(source.as_str());
    let hex = format!("{:x}", hash);

    let (path, _) = with_path_candidates(&hex, external, |path, _| {
        match path.metadata() {
            Err(_) => {
                // Not exists. Let's try to occupy.
                Ok(Some((path, true)))
            }
            Ok(md) => {
                if md.is_file() {
                    match SourceMeta::open_external(&path) {
                        Err(_) => {
                            tracing::error!(
                                "Failed to open existing source metadata at '{}'",
                                path.display()
                            );
                        }
                        Ok(meta) => {
                            if meta.url == *source {
                                return Ok(Some((path, true)));
                            }
                        }
                    }
                }
                Ok(None)
            }
        }
    })?;

    Ok((path, true))
}
