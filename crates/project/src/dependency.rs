use std::{fmt, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserializer;

use crate::{
    path::{make_relative, normalizing_join},
    real_path,
};

/// Project dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    // Fetch from the crates.io
    Crates(String),

    // Fetch from the git repository
    Git {
        git: String,
        branch: Option<String>,
    },

    // Fetch from the local path
    Path {
        #[serde(deserialize_with = "deserialize_path_expand_home")]
        path: Utf8PathBuf,
    },
}

impl Dependency {
    pub fn from_path<P>(path: P) -> Option<Self>
    where
        P: AsRef<Path>,
    {
        let path = Utf8Path::from_path(path.as_ref())?;
        Some(Dependency::Path {
            path: path.to_owned(),
        })
    }

    /// Make path dependency relative to the given base path.
    ///
    /// Current `path` and `base` must be both relative to the same base,
    /// or both be absolute.
    pub fn make_relative<P>(self, base: P) -> miette::Result<Self>
    where
        P: AsRef<Path>,
    {
        let base = base.as_ref();

        match &self {
            Dependency::Path { path } => {
                let tmp;

                let path = if path.is_relative() {
                    match real_path(path.as_std_path()) {
                        Some(path) => {
                            tmp = path;
                            &tmp
                        }
                        None => {
                            miette::bail!("Failed to resolve dependency path: {}", path);
                        }
                    }
                } else {
                    path.as_std_path()
                };

                let rel_path = make_relative(path, base);
                let Ok(utf8_path) = Utf8PathBuf::from_path_buf(rel_path) else {
                    miette::bail!("Dependency path is not UTF-8");
                };
                Ok(Dependency::Path { path: utf8_path })
            }
            _ => Ok(self),
        }
    }

    /// Make path dependency relative to the given base path.
    /// Dependency path if relative - relative to specified `old_base`.
    pub fn make_relative_from<P1, P2>(self, old_base: P1, new_base: P2) -> miette::Result<Self>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        let old_base = old_base.as_ref();
        let new_base = new_base.as_ref();

        match &self {
            Dependency::Path { path } => {
                let tmp;

                let path = if path.is_relative() {
                    match normalizing_join(old_base.to_owned(), path.as_std_path()) {
                        None => {
                            miette::bail!("Failed to resolve dependency path: {}", path);
                        }
                        Some(path) => {
                            tmp = path;
                            &tmp
                        }
                    }
                } else {
                    path.as_std_path()
                };

                let rel_path = make_relative(path, new_base);
                let Ok(utf8_path) = Utf8PathBuf::from_path_buf(rel_path) else {
                    miette::bail!("Dependency path is not UTF-8");
                };
                Ok(Dependency::Path { path: utf8_path })
            }
            _ => Ok(self),
        }
    }
}

fn deserialize_path_expand_home<'de, D>(deserializer: D) -> Result<Utf8PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path: String = serde::Deserialize::deserialize(deserializer)?;
    let path = Utf8PathBuf::from(path);

    if let Ok(suffix) = path.strip_prefix("~") {
        match dirs::home_dir() {
            Some(home) => {
                let mut home = Utf8PathBuf::from_path_buf(home).map_err(|path| {
                    serde::de::Error::custom(format!(
                        "Home directory is not UTF-8 \"{}\"",
                        path.display()
                    ))
                })?;
                home.push(suffix);
                Ok(home)
            }
            None => Ok(path),
        }
    } else {
        Ok(path)
    }
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dependency::Crates(version) => write!(f, "\"{}\"", version),
            Dependency::Git { git, branch } => {
                if let Some(branch) = branch {
                    write!(f, "{{ git = \"{git}\", branch = \"{branch}\" }}")
                } else {
                    write!(f, "{{ git = \"{git}\" }}")
                }
            }
            Dependency::Path { path } => {
                write!(f, "{{ path = \"{}\" }}", path.as_str().escape_default())
            }
        }
    }
}
