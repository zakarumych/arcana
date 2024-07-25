use std::{
    mem::size_of_val,
    path::{Path, PathBuf},
    time::SystemTime,
};

use base64::{
    alphabet::URL_SAFE,
    engine::general_purpose::{GeneralPurpose, NO_PAD},
    Engine,
};
use hashbrown::{hash_map::RawEntryMut, HashMap};
use url::Url;

use crate::{content_address::store_data_with_content_address, sha256::Sha256Hash};

#[derive(Debug, thiserror::Error)]
pub enum SourcesError {
    #[error("Failed to convert file url '{url}' to path")]
    InvalidFileUrl { url: Url },

    #[error("Failed to extract data from data url '{url}'")]
    InvalidDataUrl { url: Url },

    #[error("Failed to access file '{path}' from file URL '{url}'")]
    FileError {
        error: std::io::Error,
        url: Url,
        path: PathBuf,
    },

    #[error("Unsupported scheme '{}' in '{url}'", url.scheme())]
    UnsupportedScheme { url: Url },
}

/// Fetches and caches sources.
/// Saves remote sources to temporaries.
pub struct Sources {
    fetched: HashMap<Url, PathBuf>,
}

pub(crate) fn source_modified(url: &Url, path: &Path) -> Result<SystemTime, SourcesError> {
    match url.scheme() {
        "file" => {
            debug_assert_eq!(url.to_file_path().as_deref(), Ok(path));
            match path.metadata().and_then(|m| m.modified()) {
                Ok(modified) => Ok(modified),
                Err(error) => Err(SourcesError::FileError {
                    error,
                    url: url.clone(),
                    path: path.to_owned(),
                }),
            }
        }
        "data" => Ok(SystemTime::UNIX_EPOCH),
        _ => unreachable!(),
    }
}

impl Sources {
    pub fn new() -> Self {
        Sources {
            fetched: HashMap::new(),
        }
    }

    pub fn get(&self, source: &Url) -> Option<(&Path, SystemTime)> {
        let path = self.fetched.get(source)?;
        let modified = source_modified(source, path).ok()?;
        Some((path, modified))
    }

    pub async fn fetch(
        &mut self,
        temporaries: &Path,
        source: &Url,
    ) -> Result<(&Path, SystemTime), SourcesError> {
        match self.fetched.raw_entry_mut().from_key(source) {
            RawEntryMut::Occupied(entry) => {
                let path = &*entry.into_mut();
                let modified = source_modified(source, path)?;
                Ok((path, modified))
            }
            RawEntryMut::Vacant(entry) => match source.scheme() {
                "file" => {
                    let path =
                        source
                            .to_file_path()
                            .map_err(|()| SourcesError::InvalidFileUrl {
                                url: source.clone(),
                            })?;

                    tracing::debug!("Fetching file '{}' ('{}')", source, path.display());
                    let (_, path) = entry.insert(source.clone(), path);

                    Ok((path, source_modified(source, path)?))
                }
                "data" => {
                    let data_start = source.as_str()[size_of_val("data:")..]
                        .find(',')
                        .ok_or_else(|| SourcesError::InvalidDataUrl {
                            url: source.clone(),
                        })?
                        + 1
                        + size_of_val("data:");
                    let head = &source.as_str()[..data_start];
                    let data_str = &source.as_str()[data_start..];

                    let decoded;
                    let data = if head.ends_with(";base64,") {
                        decoded = GeneralPurpose::new(&URL_SAFE, NO_PAD)
                            .decode(data_str)
                            .map_err(|_| SourcesError::InvalidDataUrl {
                                url: source.clone(),
                            })?;
                        &decoded[..]
                    } else {
                        data_str.as_bytes()
                    };

                    let sha256 = Sha256Hash::hash(data);
                    let hex = format!("{:x}", sha256);
                    let (path, _) = store_data_with_content_address(&hex, data, temporaries)
                        .map_err(|error| SourcesError::FileError {
                            error,
                            url: source.clone(),
                            path: temporaries.to_owned(),
                        })?;

                    let (_, path) = entry.insert(source.clone(), path);
                    Ok((path, SystemTime::UNIX_EPOCH))
                }
                _ => Err(SourcesError::UnsupportedScheme {
                    url: source.clone(),
                }),
            },
        }
    }
}
