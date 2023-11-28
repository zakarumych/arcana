//! Definitions to work with Arcana projects.
#![allow(warnings)]

use std::{
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    ops::Deref,
    path::{Path, PathBuf, MAIN_SEPARATOR},
    process::Child,
};

use camino::{Utf8Path, Utf8PathBuf};

mod dependency;
mod generator;
mod ident;
mod manifest;
mod path;
mod plugin;
mod wrapper;

use generator::init_workspace;
use manifest::serialize_manifest;
use miette::{Context, IntoDiagnostic};
use path::normalizing_join;

pub use self::{
    dependency::Dependency,
    generator::new_plugin_crate,
    ident::{Ident, IdentBuf},
    manifest::{Item, Plugin, ProjectManifest},
    path::{make_relative, real_path},
    wrapper::{game_bin_path, BuildProcess},
};

const MANIFEST_NAME: &'static str = "Arcana.toml";
const CARGO_TOML_NAME: &'static str = "Cargo.toml";
const WORKSPACE_DIR_NAME: &'static str = "crates";

/// Open project object.
///
/// When open from manifest file it locks the file and syncs changes to it.
pub struct Project {
    manifest: ProjectManifest,

    // Contains path assigned to the project.
    // It will sync with the manifest file at the path both ways.
    // Whenever changes happen to the manifest file, the user will be asked what to do:
    // reaload or overwrite.
    // If file is deleted the user will be notified on save.
    // On save the file will be created if it doesn't exist.
    manifest_path: PathBuf,

    /// Project root path.
    /// Typically it is parent directory of the manifest file.
    root_path: PathBuf,
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Project")
            .field("manifest", &self.manifest_path)
            .finish()
    }
}

impl Project {
    /// Creates new project with the given name.
    ///
    /// Associate it with the path.
    ///
    /// # Errors
    ///
    /// * If `engine` dependency is provided and it is invalid.
    ///   Path dependency is invalid if it is not a valid path to directory containing `Cargo.toml`.
    /// * If `path` is occupied.
    pub fn new(
        name: IdentBuf,
        mut engine: Dependency,
        path: PathBuf,
        new: bool,
    ) -> miette::Result<Self> {
        if let Ok(m) = path.metadata() {
            if new {
                miette::bail!(
                    "Cannot create new project. Path '{}' already exists",
                    path.display()
                );
            }

            if !m.is_dir() {
                miette::bail!(
                    "Cannot create new project. Path '{}' is not a directory",
                    path.display()
                );
            }

            if path.join(MANIFEST_NAME).exists() {
                miette::bail!(
                    "Cannot create new project. Path '{}' is already an Arcana project",
                    path.display()
                );
            }
        }

        let path = match real_path(&path) {
            Some(path) => path,
            None => {
                miette::bail!(
                    "Cannot create new project. Failed to resolve path '{}'",
                    path.display()
                );
            }
        };

        let engine = match engine {
            Dependency::Path { path: engine_path } if !engine_path.is_absolute() => {
                let real_engine_path = match real_path(engine_path.as_std_path()) {
                    Some(path) => path,
                    None => {
                        miette::bail!(
                            "Cannot create new project. Failed to resolve engine path '{engine_path}'"
                        );
                    }
                };

                let relative_engine_path = make_relative(&real_engine_path, &path);

                let relative_engine_path = match Utf8PathBuf::from_path_buf(relative_engine_path) {
                    Ok(path) => path,
                    Err(err) => {
                        miette::bail!(
                            "Cannot create new project. Resolved engine path contains non-utf8 symbols '{engine_path}'",
                        );
                    }
                };

                let cargo_toml_path = real_engine_path.join(CARGO_TOML_NAME);

                let manifest = match cargo_toml::Manifest::from_path(cargo_toml_path) {
                    Ok(manifest) => manifest,
                    Err(err) => {
                        miette::bail!(
                            "Failed to read engine manifest '{engine_path}/{CARGO_TOML_NAME}': {err}",
                        );
                    }
                };

                let package = match &manifest.package {
                    Some(package) => package,
                    None => {
                        miette::bail!(
                            "'{engine_path}/{CARGO_TOML_NAME}' does not contain package section",
                        );
                    }
                };

                if package.name != "arcana" {
                    miette::bail!("'{engine_path}' is not an Arcana engine");
                }

                // Rewrite engine dependency to relative path.
                Dependency::Path {
                    path: relative_engine_path,
                }
            }
            engine => engine,
        };
        /// Construct project manifest.
        let manifest = ProjectManifest {
            name: name.to_owned(),
            engine,
            plugins: Vec::new(),
            var_systems: Vec::new(),
            fix_systems: Vec::new(),
            filters: Vec::new(),
        };

        let manifest_str = match toml::to_string(&manifest) {
            Ok(s) => s,
            Err(err) => {
                miette::bail!("Failed to serialize project manifest. {err}");
            }
        };

        if let Err(err) = std::fs::create_dir_all(&path) {
            miette::bail!(
                "Cannot create new project. Failed to create directory '{}': {err}",
                path.display()
            );
        }

        let manifest_path = path.join(MANIFEST_NAME);
        if let Err(err) = std::fs::write(&*manifest_path, &*manifest_str) {
            miette::bail!(
                "Cannot create new project. Failed to write manifest to '{}': {err}",
                manifest_path.display()
            );
        }

        tracing::info!("Created project {name} at '{}'", path.display());

        Ok(Project {
            root_path: path,
            manifest_path,
            manifest,
        })
    }

    /// Opens existing Arcana project from the given path.
    ///
    /// # Errors
    ///
    /// * If `path` is not a valid path to Arcana project.
    ///   It must be either path to a directory that contains `Arcana.toml` manifest file
    ///   or path to manifest file itself.
    pub fn open(path: &Path) -> miette::Result<Self> {
        let path = match real_path(path) {
            Some(path) => path,
            None => {
                miette::bail!(
                    "Cannot open project at '{}': failed to resolve path",
                    path.display()
                );
            }
        };

        let m = match path.metadata() {
            Ok(m) => m,
            Err(err) => {
                miette::bail!("Cannot open project at '{}': {err}", path.display());
            }
        };

        if m.is_symlink() {
            miette::bail!(
                "Cannot open project at '{}': failed to follow symlink",
                path.display()
            );
        }

        let (manifest_path, root_path) = if m.is_dir() {
            (path.join(MANIFEST_NAME), path.to_owned())
        } else {
            let root_path = match path.parent() {
                Some(path) => path.to_owned(),
                None => {
                    miette::bail!(
                        "Cannot open project at '{}': failed to resolve parent directory",
                        path.display()
                    );
                }
            };
            (path.to_owned(), root_path)
        };

        let mut arcana_toml = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(err) => {
                miette::bail!(
                    "Cannot open project at '{}': failed to read project manifest: {err}",
                    path.display()
                );
            }
        };

        let manifest: ProjectManifest = match toml::from_str(&arcana_toml) {
            Ok(manifest) => manifest,
            Err(err) => {
                miette::bail!("Cannot deserialize project manifest from \"Arcana.toml\": {err}");
            }
        };

        let project = Project {
            root_path,
            manifest_path,
            manifest,
        };

        Ok(project)
    }

    /// Searches for Arcana project in the given path or any parent directory.
    ///
    /// # Errors
    ///
    /// * If `path` is not a valid path.
    /// * If project is not found in `path` or any parent directory.
    /// * If project is found but cannot be opened.
    pub fn find(path: &Path) -> miette::Result<Self> {
        let mut candidate = match real_path(path) {
            Some(path) => path,
            None => {
                miette::bail!(
                    "Cannot find project at '{}': failed to resolve path",
                    path.display()
                );
            }
        };

        loop {
            candidate.push(MANIFEST_NAME);
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

    /// Returns name of the project.
    pub fn name(&self) -> &Ident {
        &self.manifest.name
    }

    /// Returns name of the project.
    pub fn set_name(&mut self, name: impl Into<IdentBuf>) {
        self.manifest.name = name.into();
    }

    /// Returns path to the project.
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn sync(&mut self) -> miette::Result<()> {
        // let serialized_manifest = toml::to_string(&self.manifest).map_err(|err| {
        //     miette::miette!("Cannot serialize project manifest to \"Arcana.toml\": {err}")
        // })?;

        let serialized_manifest = serialize_manifest(&self.manifest).map_err(|err| {
            miette::miette!("Cannot serialize project manifest to \"Arcana.toml\": {err}")
        })?;

        match std::fs::write(&self.manifest_path, serialized_manifest) {
            Ok(()) => Ok(()),
            Err(err) => {
                miette::bail!(
                    "Cannot write project manifest to \"Arcana.toml\": {err}",
                    err = err
                );
            }
        }
    }

    /// Initializes all plugin wrapper libs and workspace.
    pub fn init_workspace(&self) -> miette::Result<()> {
        init_workspace(
            &self.root_path,
            &self.manifest.name,
            &self.manifest.engine,
            &self.manifest.plugins,
        )
    }

    pub fn build_plugins_library(&self) -> miette::Result<BuildProcess> {
        self.init_workspace()?;
        wrapper::build_plugins(&self.root_path)
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut ProjectManifest {
        &mut self.manifest
    }

    pub fn run_editor(self) -> miette::Result<()> {
        self.init_workspace()?;
        let status = wrapper::run_editor(&self.root_path)
            .status()
            .map_err(|err| {
                miette::miette!(
                    "Cannot run \"ed\" on \"{}\": {err}",
                    self.root_path.display()
                )
            })?;

        match status.code() {
            Some(0) => Ok(()),
            Some(code) => miette::bail!("\"ed\" exited with code {}", code),
            None => miette::bail!("\"ed\" terminated by signal"),
        }
    }

    pub fn build_editor_non_blocking(&self) -> miette::Result<Child> {
        self.init_workspace()?;
        match wrapper::build_editor(&self.root_path).spawn() {
            Ok(child) => Ok(child),
            Err(err) => {
                miette::bail!(
                    "Cannot build \"ed\" on \"{}\": {err}",
                    self.root_path.display()
                )
            }
        }
    }

    pub fn run_editor_non_blocking(self) -> miette::Result<Child> {
        self.init_workspace()?;
        match wrapper::run_editor(&self.root_path).spawn() {
            Ok(child) => Ok(child),
            Err(err) => {
                miette::bail!(
                    "Cannot run \"ed\" on \"{}\": {err}",
                    self.root_path.display()
                )
            }
        }
    }

    pub fn build_game(self) -> miette::Result<PathBuf> {
        self.init_workspace()?;
        let status = wrapper::build_game(&self.root_path)
            .status()
            .map_err(|err| {
                miette::miette!("Cannot build game \"{}\": {err}", self.root_path.display())
            })?;

        match status.code() {
            Some(0) => {}
            Some(code) => miette::bail!("Game build exited with code {}", code),
            None => miette::bail!("Game build terminated by signal"),
        }

        Ok(game_bin_path(&self.manifest.name, &self.root_path))
    }

    pub fn run_game(self) -> miette::Result<()> {
        self.init_workspace()?;
        let status = wrapper::run_game(&self.root_path).status().map_err(|err| {
            miette::miette!("Cannot run game on \"{}\": {err}", self.root_path.display())
        })?;

        match status.code() {
            Some(0) => Ok(()),
            Some(code) => miette::bail!("Game exited with code {}", code),
            None => miette::bail!("Game terminated by signal"),
        }
    }

    pub fn add_plugin(&mut self, name: IdentBuf, dep: Dependency) -> bool {
        if self.manifest.has_plugin(&name) {
            return false;
        }

        let dep = dep.make_relative(&self.root_path);

        tracing::info!("Plugin '{} added", name);
        let plugin = Plugin {
            name,
            dep,
            enabled: true,
        };

        self.manifest.plugins.push(plugin);
        true
    }
}

fn is_in_cargo_workspace(path: &Path) -> bool {
    for a in path.ancestors() {
        if a.exists() {
            let mut candidate = a.to_owned();

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

pub fn plugin_with_path(plugin_path: &Path) -> miette::Result<(IdentBuf, Dependency)> {
    let real_plugin_path = match real_path(plugin_path) {
        Some(path) => path,
        None => {
            miette::bail!(
                "Cannot search plugin at \"{}\": failed to resolve path",
                plugin_path.display()
            );
        }
    };

    let lib_path: &Path;
    let cargo_toml_path;

    if real_plugin_path.file_name() == Some("Cargo.toml".as_ref()) {
        cargo_toml_path = real_plugin_path;
        lib_path = cargo_toml_path.parent().unwrap();
    } else {
        cargo_toml_path = real_plugin_path.join("Cargo.toml");
        lib_path = real_plugin_path.as_ref();
    }

    let lib_path = match Utf8Path::from_path(lib_path) {
        Some(path) => path,
        None => {
            miette::bail!(
                "Cannot add library path \"{}\": path is not valid UTF-8",
                plugin_path.display()
            )
        }
    };

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

    Ok((
        IdentBuf::from_string(package.name).expect("Package name is not valid ident"),
        Dependency::Path {
            path: lib_path.to_path_buf(),
        },
    ))
}
