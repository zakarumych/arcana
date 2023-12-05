use std::{fmt, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserializer;

use crate::{path::make_relative, real_path};

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

    pub fn make_relative<P>(self, base: P) -> miette::Result<Self>
    where
        P: AsRef<Path>,
    {
        match &self {
            Dependency::Path { path } if !path.is_absolute() => {
                let Some(real_path) = real_path(path.as_std_path()) else {
                    miette::bail!("Failed to resolve dependency path: {}", path);
                };

                let rel_path = make_relative(&real_path, base.as_ref());
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
