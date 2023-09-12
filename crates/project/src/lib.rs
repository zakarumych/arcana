//! Definitions to work with Arcana projects.

use std::{
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use camino::Utf8PathBuf;

pub mod path;
mod wrapper;

use miette::IntoDiagnostic;
use path::{real_path, RealPath, RealPathBuf};
pub use wrapper::PluginBuild;

use crate::path::make_relative;

/// Project dependency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    // Fetch from the crates.io
    Crates(String),

    // Fetch from the git repository
    Git { git: String, branch: Option<String> },

    // Fetch from the local path
    Path { path: Utf8PathBuf },
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Plugin {
    pub name: String,
    pub dep: Dependency,
    pub enabled: bool,
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
    #[serde(skip_serializing_if = "Vec::is_empty", default, with = "plugins_serde")]
    pub plugins: Vec<Plugin>,
}

mod plugins_serde {
    use std::fmt;

    use serde::ser::{SerializeMap, Serializer};

    use super::{Dependency, Plugin};

    pub fn serialize<S>(plugins: &Vec<Plugin>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_map(Some(plugins.len()))?;

        #[derive(serde::Serialize)]
        struct PluginSer<'a> {
            dep: &'a Dependency,
            enabled: bool,
        }

        for plugin in plugins {
            serializer.serialize_entry(
                &plugin.name,
                &PluginSer {
                    dep: &plugin.dep,
                    enabled: plugin.enabled,
                },
            )?;
        }

        serializer.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Plugin>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(VisitPlugins)
    }

    struct VisitPlugins;

    impl<'de> serde::de::Visitor<'de> for VisitPlugins {
        type Value = Vec<Plugin>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("map of plugins")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            #[derive(serde::Deserialize)]
            struct PluginDe {
                dep: Dependency,
                enabled: bool,
            }

            let mut plugins = Vec::new();

            while let Some((name, plugin)) = map.next_entry::<String, PluginDe>()? {
                plugins.push(Plugin {
                    name,
                    dep: plugin.dep.clone(),
                    enabled: plugin.enabled,
                });
            }

            Ok(plugins)
        }
    }
}

/// Open project object.
///
/// Locks corresponding "Arcana.toml" file.
/// Syncs changes to the manifest.
pub struct Project {
    path: RealPathBuf,
    file: File,
    manifest: ProjectManifest,
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Project").field("path", &self.path).finish()
    }
}

impl Project {
    pub fn new(
        path: RealPathBuf,
        name: Option<String>,
        arcana: Option<Dependency>,
        new: bool,
    ) -> miette::Result<Self> {
        let arcana_toml_path = path.join("Arcana.toml");
        let path_meta = path.metadata();

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

        if new {
            if path_meta.is_ok() {
                miette::bail!("Destination '{}' already exists", path.display());
            }
            if is_in_cargo_workspace(path.as_ref()) {
                miette::bail!("Project cannot be created inside a cargo workspace");
            }
            if let Err(err) = std::fs::create_dir_all(&path) {
                miette::bail!(
                    "Failed to create project directory '{}'. {err}",
                    path.display()
                );
            };
        } else {
            match path_meta {
                Err(_) => {
                    if is_in_cargo_workspace(&path) {
                        miette::bail!("Project cannot be created inside a cargo workspace");
                    }
                    if let Err(err) = std::fs::create_dir_all(&path) {
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
                    if arcana_toml_path.exists() {
                        miette::bail!("Project already initialized");
                    }
                    if is_in_cargo_workspace(&path) {
                        miette::bail!("Project cannot be created inside a cargo workspace");
                    }
                }
            }
        }

        let manifest = ProjectManifest {
            name: name.to_owned(),
            arcana,
            plugins: Vec::new(),
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
            .open(&arcana_toml_path)
        {
            Ok(f) => f,
            Err(err) => {
                miette::bail!(
                    "Failed to create project manifest at '{}'. {err}",
                    arcana_toml_path.display()
                );
            }
        };

        if let Err(err) = project_file.write_all(manifest_str.as_bytes()) {
            miette::bail!(
                "Failed to write project manifest to '{}'. {err}",
                arcana_toml_path.display()
            );
        };

        if let Some(Dependency::Path { path: arcana_path }) = &manifest.arcana {
            let check = || -> miette::Result<()> {
                let cargo_toml_path = path.join(arcana_path).join("Cargo.toml");
                let manifest =
                    cargo_toml::Manifest::from_path(cargo_toml_path).into_diagnostic()?;
                let package = manifest
                    .package
                    .as_ref()
                    .ok_or_else(|| miette::miette!("not a package"))?;
                if package.name != "arcana" {
                    miette::bail!("not an arcana package");
                }
                Ok(())
            };

            if let Err(err) = check() {
                tracing::warn!("'arcana' path dependency is '{arcana_path}' broken: {err}");
            }
        }

        tracing::info!("Created project {name} at '{}'", path.display());

        Ok(Project {
            path,
            file: project_file,
            manifest,
        })
    }

    pub fn find(path: &Path) -> miette::Result<Self> {
        let mut candidate = real_path(path).into_diagnostic()?.into_path();

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
        let path = real_path(path).into_diagnostic()?;

        let mut file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(&path)
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

        let mut path = path;
        path.pop();

        let project = Project {
            path,
            file,
            manifest,
        };

        Ok(project)
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn sync(&mut self) -> miette::Result<()> {
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

    pub fn add_plugin_with_path(&mut self, path: &Path, init: bool) -> miette::Result<String> {
        let canon_path = real_path(path).into_diagnostic()?;

        let lib_path: &RealPath;
        let cargo_toml_path;

        if path.file_name() == Some("Cargo.toml".as_ref()) {
            cargo_toml_path = canon_path.into_path();
            lib_path = RealPath::wrap(cargo_toml_path.parent().unwrap());
        } else {
            cargo_toml_path = canon_path.join("Cargo.toml");
            lib_path = canon_path.as_ref();
        }

        let lib_path = make_relative(&lib_path, &self.path);

        let lib_path_str = lib_path.to_str().ok_or_else(|| {
            miette::miette!(
                "Cannot add library path \"{}\": path is not valid UTF-8",
                path.display()
            )
        })?;

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

        let package = manifest.package.ok_or_else(|| {
            miette::miette!("Not a package manifest: \"{}\"", cargo_toml_path.display())
        })?;

        if self.manifest.plugins.iter().any(|p| p.name == package.name) {
            miette::bail!(
                "Library \"{}\" is already added to the project",
                package.name
            );
        }

        let dep = Dependency::Path {
            path: lib_path_str.into(),
        };

        let plugin = Plugin {
            name: package.name.clone(),
            dep,
            enabled: true,
        };

        self.manifest.plugins.push(plugin);

        if init {
            self.init_workspace()?;
        }

        self.sync()?;
        Ok(package.name)
    }

    /// Initializes all plugin wrapper libs and workspace.
    pub fn init_workspace(&self) -> miette::Result<()> {
        wrapper::init_workspace(
            &self.manifest.name,
            self.manifest.arcana.as_ref(),
            self.manifest.plugins.iter().map(|p| &*p.name),
            &self.path,
        )?;

        for plugin in &self.manifest.plugins {
            wrapper::init_plugin(&plugin.name, &plugin.dep, &self.path)?;
        }

        Ok(())
    }

    pub fn build_plugin_library(&self, name: &str) -> miette::Result<PluginBuild> {
        wrapper::build_plugin(name, &self.path)
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut ProjectManifest {
        &mut self.manifest
    }

    pub fn run_editor(self) -> miette::Result<()> {
        let Project {
            file,
            manifest,
            path,
        } = self;

        drop(file);

        let status = wrapper::run_editor(&manifest.name, &path)
            .status()
            .map_err(|err| miette::miette!("Cannot run \"ed\" on \"{}\": {err}", path.display()))?;

        match status.code() {
            Some(0) => Ok(()),
            Some(code) => miette::bail!("\"ed\" exited with code {}", code),
            None => miette::bail!("\"ed\" terminated by signal"),
        }
    }
}

fn invalid_name_character(c: char) -> bool {
    !c.is_alphanumeric() && c != '_' && c != '-'
}

fn is_in_cargo_workspace(path: &RealPath) -> bool {
    for a in path.ancestors() {
        if a.exists() {
            let mut candidate = a.as_path().to_owned();

            loop {
                candidate.push("Cargo.toml");
                if candidate.exists() {
                    return true;
                }
                assert!(candidate.pop());
                if !candidate.pop() {
                    break;
                }
            }
            return false;
        }
    }
    false
}
