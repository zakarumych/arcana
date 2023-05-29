//! Definitions to work with Arcana projects.

use std::{
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use hashbrown::HashMap;

mod wrapper;

/// Project dependency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    // Fetch from the crates.io
    Crates(String),

    // Fetch from the git repository
    Git { git: String, branch: Option<String> },

    // Fetch from the local path
    Path { path: String },
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
                write!(f, "{{ path = \"{}\" }}", path.escape_default())
            }
        }
    }
}

/// Project manifest.
///
/// Typically parsed from "Arcana.toml" file.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectManifest {
    pub name: String,

    /// How to fetch arcana dependency.
    /// Defaults to `Dependency::Crates(version())`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub arcana: Option<Dependency>,

    /// List of plugin libraries this project depends on.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub plugin_libs: HashMap<String, Dependency>,

    /// Enabled plugins.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub enabled: HashMap<String, Vec<String>>,
}

// pub fn default_arcana_dependency() -> Dependency {
//     // Use arcana engine with matching version by default.
//     Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
// }

/// Project object.
///
/// When this object exists, it is synced to the corresponding "Arcana.toml" file.
/// Opened project locks the "Arcana.toml" file.
pub struct Project {
    path: PathBuf,
    file: File,
    manifest: ProjectManifest,
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Project").field("path", &self.path).finish()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        if let Err(err) = self.sync() {
            tracing::error!("Failed to sync project manifest: {}", err);
        }
    }
}

impl Project {
    pub fn new(
        path: PathBuf,
        name: Option<String>,
        arcana: Option<Dependency>,
        new: bool,
    ) -> miette::Result<Self> {
        let name = match &name {
            None => {
                let Some(file_name) = path.file_name() else {
                    miette::bail!("Failed to get project name destination path");
                };

                if file_name.is_empty() || file_name == "." || file_name == ".." {
                    miette::bail!("Failed to get project name destination path");
                }

                let Some(file_name) = file_name.to_str() else {
                    miette::bail!("Failed to get project name destination path");
                };

                file_name
            }
            Some(name) => name,
        };
        if name.is_empty() {
            miette::bail!("Project name cannot be empty");
        }
        if !name.chars().next().unwrap().is_alphabetic() {
            miette::bail!("Project name must start with a letter");
        }
        if name.contains(invalid_name_character) {
            miette::bail!("Project name must contain only alphanumeric characters and underscores");
        }
        let Ok(cd) = std::env::current_dir() else {
            miette::bail!("Failed to get current directory");
        };

        let project_path = cd.join(&*path);

        let project_toml_path = project_path.join("Arcana.toml");
        let project_path_meta = project_path.metadata();
        if new {
            if project_path_meta.is_ok() {
                miette::bail!("Destination '{}' already exists", path.display());
            }
            if let Err(err) = std::fs::create_dir_all(&project_path) {
                miette::bail!(
                    "Failed to create project directory '{}'. {err}",
                    path.display()
                );
            };
        } else {
            match project_path_meta {
                Err(_) => {
                    if let Err(err) = std::fs::create_dir_all(&project_path) {
                        miette::bail!(
                            "Failed to create project directory '{}'. {err}",
                            path.display()
                        );
                    };
                }
                Ok(meta) => {
                    if !meta.is_dir() {
                        miette::bail!("Destination '{}' is not a directory", path.display());
                    }
                    if project_toml_path.exists() {
                        miette::bail!("Project already initialized");
                    }
                }
            }
        }

        let project_path = dunce::canonicalize(&project_path).map_err(|err| {
            miette::miette!(
                "Failed to canonicalize project path '{}'. {err}",
                project_path.display()
            )
        })?;

        let manifest = ProjectManifest {
            name: name.to_owned(),
            arcana,
            plugin_libs: HashMap::new(),
            enabled: HashMap::new(),
        };

        let manifest_str = match toml::to_string(&manifest) {
            Ok(s) => s,
            Err(err) => {
                miette::bail!("Failed to serialize project manifest. {err}");
            }
        };

        let mut project_file = match std::fs::File::options()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&project_toml_path)
        {
            Ok(f) => f,
            Err(err) => {
                miette::bail!(
                    "Failed to create project manifest at '{}'. {err}",
                    project_toml_path.display()
                );
            }
        };

        if let Err(err) = project_file.write_all(manifest_str.as_bytes()) {
            miette::bail!(
                "Failed to write project manifest to '{}'. {err}",
                project_toml_path.display()
            );
        };

        Ok(Project {
            path: project_path,
            file: project_file,
            manifest,
        })
    }

    pub fn find(path: &Path) -> miette::Result<Self> {
        let Ok(cd) = std::env::current_dir() else {
            miette::bail!("Failed to get current directory");
        };

        let mut candidate = dunce::canonicalize(cd.join(path))
            .map_err(|err| miette::miette!("Failed to canonicalize {}: {err}", path.display()))?;

        loop {
            candidate.push("Arcana.toml");
            if candidate.exists() {
                return Project::open(&candidate);
            }
            if !candidate.pop() {
                break;
            }
            if !candidate.pop() {
                break;
            }
        }

        miette::bail!(
            "Cannot find project in '{}' or any parent directory",
            path.display()
        );
    }

    pub fn open(path: &Path) -> miette::Result<Self> {
        let mut file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|err| {
                miette::miette!(
                    "Cannot open project at {}, failed to open \"Arcana.toml\": {err}",
                    path.display()
                )
            })?;

        let mut arcana_toml = String::new();
        file.read_to_string(&mut arcana_toml).map_err(|err| {
            miette::miette!(
                "Cannot read project manifest from \"{}\\Arcana.toml\": {err}",
                path.display()
            )
        })?;

        let manifest: ProjectManifest = toml::from_str(&arcana_toml).map_err(|err| {
            miette::miette!("Cannot deserialize project manifest from \"Arcana.toml\": {err}")
        })?;

        let file_path = dunce::canonicalize(path).expect("existing path");
        let project_path = file_path.parent().expect("parent path");

        let project = Project {
            path: project_path.to_owned(),
            file,
            manifest,
        };

        Ok(project)
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn add_library_path(&mut self, path: &Path) -> miette::Result<()> {
        let path_str = path.to_str().ok_or_else(|| {
            miette::miette!(
                "Cannot add library path \"{}\": path is not valid UTF-8",
                path.display()
            )
        })?;

        let cargo_toml_path = path.join("Cargo.toml");

        let cargo_toml = std::fs::read_to_string(&cargo_toml_path).map_err(|err| {
            miette::miette!(
                "Cannot read Cargo.toml from \"{}\": {err}",
                cargo_toml_path.display()
            )
        })?;

        let manifest = cargo_toml::Manifest::from_str(&cargo_toml).map_err(|err| {
            miette::miette!(
                "Cannot read Cargo.toml from \"{}\": {err}",
                cargo_toml_path.display()
            )
        })?;

        let package = manifest.package.as_ref().ok_or_else(|| {
            miette::miette!("Not a package manifest: \"{}\"", cargo_toml_path.display())
        })?;

        self.manifest.plugin_libs.insert(
            package.name.clone(),
            Dependency::Path {
                path: path_str.to_owned(),
            },
        );

        self.sync()?;

        Ok(())
    }

    fn sync(&mut self) -> miette::Result<()> {
        self.manifest.enabled.retain(|_, v| !v.is_empty());

        let content = toml::to_string_pretty(&self.manifest).map_err(|err| {
            miette::miette!("Cannot serialize project manifest to \"Arcana.toml\": {err}")
        })?;

        let mut write_to_file = || {
            self.file.seek(SeekFrom::Start(0))?;
            self.file.set_len(0)?;
            self.file.write_all(content.as_bytes())
        };

        write_to_file().map_err(|err| {
            miette::miette!(
                "Cannot write project manifest to \"{}\\Arcana.toml\": {err}",
                self.path.display()
            )
        })?;

        Ok(())
    }

    /// Initializes all plugin wrapper libs and workspace.
    pub fn init_workspace(&self) -> miette::Result<()> {
        wrapper::init_workspace(
            self.manifest.arcana.as_ref(),
            self.manifest.plugin_libs.keys(),
            &self.path,
        )?;

        for (name, dep) in &self.manifest.plugin_libs {
            wrapper::init_plugin(name, dep, self.manifest.arcana.as_ref(), &self.path)?;
        }

        Ok(())
    }

    pub fn plugin_lib_paths(&self) -> miette::Result<HashMap<String, PathBuf>> {
        let mut paths = HashMap::new();

        for (name, _) in &self.manifest.plugin_libs {
            let path = wrapper::plugin_lib_path(name, &self.path)?;
            paths.insert(name.clone(), path);
        }

        Ok(paths)
    }
}

fn invalid_name_character(c: char) -> bool {
    !c.is_alphanumeric() && c != '_'
}
