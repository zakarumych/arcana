//! Provides types to crete, save, open and manipulate Arcana projects.
//!
//!
#![allow(warnings)]

use std::{
    fmt,
    fs::{File, FileType},
    io::{Read, Seek, SeekFrom, Write},
    ops::Deref,
    path::{absolute, Path, PathBuf, MAIN_SEPARATOR},
    process::Child,
};

use arcana_error::Error;
use arcana_intern::{Ident, Name};
use camino::{Utf8Path, Utf8PathBuf};

mod dependency;
mod generator;
mod manifest;
mod path;
mod plugin;
mod wrapper;

use generator::init_workspace;
use manifest::serialize_manifest;

pub use self::{
    dependency::Dependency,
    generator::new_plugin_crate,
    manifest::ProjectManifest,
    path::{is_available, make_relative},
    plugin::Plugin,
    wrapper::{game_bin_path, BuildProcess, Profile},
};

const MANIFEST_FILE_EXT: &'static str = "arcana";
const CARGO_TOML_NAME: &'static str = "Cargo.toml";
const WORKSPACE_DIR_NAME: &'static str = "crates";

/// An open project object.
///
/// It contains project manifest,
/// manifest file path
/// and project root path.
///
/// Manifest file is a TOML file and is written when project is synced.
/// When new project is created file with initial manifest is created.
///
/// If file is edited or deleted project will silently overwrite it on sync.
///
/// TODO: Figure out why not to lock the file?
pub struct Project {
    /// Actual project manifest.
    manifest: ProjectManifest,

    // Contains path assigned to the project.
    // It will sync with the manifest file at the path both ways.
    // Whenever changes happen to the manifest file, the user will be asked what to do:
    // reload or overwrite.
    // If file is deleted the user will be notified on save.
    // On save the file will be created if it doesn't exist.
    manifest_path: PathBuf,
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Project")
            .field("manifest", &self.manifest_path)
            .finish()
    }
}

impl Project {
    /// Creates new project with the given name at the given path.
    /// Path will become project root directory.
    /// Manifest file will be `<project-name>.arcana` in the root directory.
    ///
    /// # Errors
    ///
    /// * If `engine` dependency is invalid.
    ///   Path dependency is invalid if it is not a valid path to directory containing `Cargo.toml`.
    /// * If `new` is true and `path` is already exists.
    /// * If `path` already contains Arcana project.
    pub fn new(name: Ident, path: &Path, mut engine: Dependency, new: bool) -> Result<Self, Error> {
        let manifest_file_name = format!("{}.{}", name, MANIFEST_FILE_EXT);

        if let Ok(m) = path.metadata() {
            if new {
                return Err(Error::msg(format!(
                    "Cannot create new project. Path '{}' already exists",
                    path.display()
                )));
            }

            if !m.is_dir() {
                return Err(Error::msg(format!(
                    "Cannot create new project. Path '{}' is not a directory",
                    path.display()
                )));
            }

            if path.join(&manifest_file_name).exists() {
                return Err(Error::msg(format!(
                    "Cannot create new project. Path '{}' is already an Arcana project",
                    path.display()
                )));
            }
        }

        let path = match absolute(path) {
            Ok(path) => path,
            Err(err) => {
                return Err(Error::msg(format!(
                    "Cannot create new project. Failed to resolve path '{}': {err:?}",
                    path.display()
                )));
            }
        };

        /// Construct project manifest.
        let manifest = ProjectManifest {
            name,
            engine,
            plugins: Vec::new(),
        };

        let manifest_str = match toml::to_string(&manifest) {
            Ok(s) => s,
            Err(err) => {
                return Err(Error::msg(format!(
                    "Failed to serialize project manifest. {err:?}"
                )));
            }
        };

        if let Err(err) = std::fs::create_dir_all(&path) {
            return Err(Error::msg(format!(
                "Cannot create new project. Failed to create directory '{}': {err:?}",
                path.display()
            )));
        }

        let manifest_path = path.join(&manifest_file_name);
        if let Err(err) = std::fs::write(&*manifest_path, &*manifest_str) {
            return Err(Error::msg(format!(
                "Cannot create new project. Failed to write manifest to '{}': {err:?}",
                manifest_path.display()
            )));
        }

        let manifest_path = path.join(&manifest_file_name);
        if let Err(err) = std::fs::write(&*manifest_path, &*manifest_str) {
            return Err(Error::msg(format!(
                "Cannot create new project. Failed to write manifest to '{}': {err:?}",
                manifest_path.display()
            )));
        }

        tracing::info!("Created project {name} at '{}'", path.display());

        Ok(Project {
            manifest_path,
            manifest,
        })
    }

    /// Opens existing Arcana project from the given path.
    /// The path must a manifest file.
    ///
    /// # Errors
    ///
    /// * If `path` is not a valid path to Arcana project.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let manifest_path = match absolute(path) {
            Ok(path) => path,
            Err(err) => {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}': {err:?}",
                    path.display()
                )));
            }
        };

        let Some(manifest_file_name) = manifest_path.file_name() else {
            if manifest_path != path {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}'(resolved to '{}'): no file name",
                    path.display(),
                    manifest_path.display(),
                )));
            } else {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}': no file name",
                    path.display()
                )));
            }
        };

        let m = match manifest_path.metadata() {
            Ok(m) => m,
            Err(err) => {
                if manifest_path != path {
                    return Err(Error::msg(format!(
                        "Cannot open project file at '{}'(resolved to '{}'): {err:?}",
                        path.display(),
                        manifest_path.display()
                    )));
                } else {
                    return Err(Error::msg(format!(
                        "Cannot open project file at '{}': {err:?}",
                        path.display()
                    )));
                }
            }
        };

        if m.is_symlink() {
            if manifest_path != path {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}'(resolved to '{}'): failed to follow symlink",
                    path.display(),
                    manifest_path.display()
                )));
            } else {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}': failed to follow symlink",
                    path.display()
                )));
            }
        }

        if m.is_dir() {
            if manifest_path != path {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}'(resolved to '{}'): is a directory",
                    path.display(),
                    manifest_path.display()
                )));
            } else {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}': is a directory",
                    path.display()
                )));
            }
        }

        let mut arcana_toml = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(err) => {
                return Err(Error::msg(format!(
                    "Cannot open project at '{}': failed to read project manifest: {err:?}",
                    path.display()
                )));
            }
        };

        let manifest: ProjectManifest = match toml::from_str(&arcana_toml) {
            Ok(manifest) => manifest,
            Err(err) => {
                if manifest_path != path {
                    return Err(Error::msg(format!(
                        "Cannot deserialize project manifest from '{}'(resolved to '{}'): {err:?}",
                        path.display(),
                        manifest_path.display()
                    )));
                } else {
                    return Err(Error::msg(format!(
                        "Cannot deserialize project manifest from '{}': {err:?}",
                        path.display()
                    )));
                }
            }
        };

        let project = Project {
            manifest_path,
            manifest,
        };

        Ok(project)
    }

    pub fn root_path(&self) -> &Path {
        self.manifest_path
            .parent()
            .expect("File path must have parent")
    }

    /// Returns path to the manifest file.
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn sync(&mut self) -> Result<(), Error> {
        let serialized_manifest = serialize_manifest(&self.manifest)
            .map_err(|err| Error::msg(format!("Cannot serialize project manifest: {err:?}")))?;

        match std::fs::write(&self.manifest_path, serialized_manifest) {
            Ok(()) => Ok(()),
            Err(err) => {
                return Err(Error::msg(format!(
                    "Cannot write project manifest to '{}': {:?}",
                    self.manifest_path.display(),
                    err,
                )));
            }
        }
    }

    /// Initializes all plugin wrapper libs and workspace.
    pub fn init_workspace(&self) -> Result<(), Error> {
        init_workspace(
            self.root_path(),
            &self.manifest.name,
            &self.manifest.engine,
            &self.manifest.plugins,
        )
    }

    pub fn build_plugins_library(&self, profile: Profile) -> Result<BuildProcess, Error> {
        self.init_workspace()?;
        wrapper::build_plugins(self.root_path(), profile)
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut ProjectManifest {
        &mut self.manifest
    }

    /// Returns name of the project.
    pub fn name(&self) -> Ident {
        self.manifest.name
    }

    pub fn engine(&self) -> &Dependency {
        &self.manifest.engine
    }

    pub fn engine_mut(&mut self) -> &mut Dependency {
        &mut self.manifest.engine
    }

    pub fn plugins(&self) -> &[Plugin] {
        &self.manifest.plugins
    }

    pub fn plugins_mut(&mut self) -> &mut [Plugin] {
        &mut self.manifest.plugins
    }

    pub fn run_editor(self, profile: Profile) -> Result<(), Error> {
        self.init_workspace()?;
        let status = wrapper::run_editor(self.root_path(), &self.manifest_path, profile)
            .status()
            .map_err(|err| {
                Error::msg(format!(
                    "Cannot run \"ed\" on \"{}\": {err:?}",
                    self.manifest_path.display()
                ))
            })?;

        match status.code() {
            Some(0) => Ok(()),
            Some(code) => return Err(Error::msg(format!("\"ed\" exited with code {}", code))),
            None => return Err(Error::msg(format!("\"ed\" terminated by signal"))),
        }
    }

    pub fn build_editor_non_blocking(&self, profile: Profile) -> Result<Child, Error> {
        self.init_workspace()?;
        match wrapper::build_editor(self.root_path(), profile).spawn() {
            Ok(child) => Ok(child),
            Err(err) => {
                return Err(Error::msg(format!(
                    "Cannot build \"ed\" on \"{}\": {err:?}",
                    self.manifest_path.display()
                )));
            }
        }
    }

    pub fn run_editor_non_blocking(&self, profile: Profile) -> Result<Child, Error> {
        self.init_workspace()?;
        match wrapper::run_editor(self.root_path(), &self.manifest_path, profile).spawn() {
            Ok(child) => Ok(child),
            Err(err) => {
                return Err(Error::msg(format!(
                    "Cannot run \"ed\" on \"{}\": {err:?}",
                    self.manifest_path.display()
                )));
            }
        }
    }

    pub fn build_game(self, profile: Profile) -> Result<PathBuf, Error> {
        self.init_workspace()?;
        let status = wrapper::build_game(self.root_path(), profile)
            .status()
            .map_err(|err| {
                Error::msg(format!(
                    "Cannot build game \"{}\": {err:?}",
                    self.manifest_path.display(),
                ))
            })?;

        match status.code() {
            Some(0) => {}
            Some(code) => return Err(Error::msg(format!("Game build exited with code {}", code))),
            None => return Err(Error::msg(format!("Game build terminated by signal"))),
        }

        Ok(game_bin_path(&self.manifest.name, self.root_path()))
    }

    pub fn run_game(self, profile: Profile) -> Result<(), Error> {
        self.init_workspace()?;
        let status = wrapper::run_game(self.root_path(), profile)
            .status()
            .map_err(|err| {
                Error::msg(format!(
                    "Cannot run game on \"{}\": {err:?}",
                    self.manifest_path.display()
                ))
            })?;

        match status.code() {
            Some(0) => Ok(()),
            Some(code) => return Err(Error::msg(format!("Game exited with code {}", code))),
            None => return Err(Error::msg(format!("Game terminated by signal"))),
        }
    }

    pub fn has_plugin(&self, name: Ident) -> bool {
        self.manifest.has_plugin(name)
    }

    pub fn add_plugin(&mut self, mut plugin: Plugin) -> Result<bool, Error> {
        if self.manifest.has_plugin(plugin.name) {
            return Ok(false);
        }

        plugin.dependency = plugin.dependency.make_relative(self.root_path())?;

        tracing::info!("Plugin '{}' added", plugin.name);

        self.manifest.plugins.push(plugin);
        Ok(true)
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

pub fn process_path_ident(path: &Path, name: Option<Ident>) -> Result<(PathBuf, Ident), Error> {
    let path = match absolute(path) {
        Ok(path) => path,
        Err(err) => {
            return Err(Error::msg(format!(
                "Failed to get project destination path from {}: {err:?}",
                path.display()
            )))
        }
    };

    let name = match name {
        None => {
            let Some(file_name) = path.file_name() else {
                return Err(Error::msg(format!(
                    "Failed to get project name destination path"
                )));
            };

            if file_name.is_empty() || file_name == "." || file_name == ".." {
                return Err(Error::msg(format!(
                    "Failed to get project name destination path"
                )));
            }

            let Some(file_name) = file_name.to_str() else {
                return Err(Error::msg(format!(
                    "Failed to get project name destination path"
                )));
            };

            let Ok(file_name) = Ident::from_str(file_name) else {
                return Err(Error::msg(format!(
                    "Project's directory name cannot be used as project name is it is not valid identifier. Specify name manually"
                )));
            };

            file_name.to_owned()
        }
        Some(name) => name,
    };

    Ok((path, name))
}

pub fn validate_engine_path(engine_path: &Path) -> Result<Dependency, Error> {
    if !engine_path.is_absolute() {
        return Err(Error::msg(format!("Engine path must be absolute")));
    }

    let mut engine_path = match Utf8Path::from_path(engine_path) {
        Some(path) => path,
        None => {
            return Err(Error::msg(format!(
                "Engine path is not UTF-8: {}",
                engine_path.display()
            )));
        }
    };

    match engine_path.metadata() {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::msg(format!(
                "Engine path '{engine_path}' does not exist"
            )));
        }
        Err(err) => {
            return Err(Error::msg(format!(
                "Failed to read engine path '{engine_path}': {err:?}"
            )));
        }
        Ok(metadata) => match metadata.file_type() {
            ft if ft.is_file() => match engine_path.file_name() {
                Some(file_name) if file_name == "Cargo.toml" => {
                    let mut engine_path = engine_path.to_owned();
                    validate_engine_manifest(&engine_path)?;

                    assert!(
                        engine_path.pop(),
                        "Absolute path with filename must have a parent"
                    );

                    Ok(Dependency::Path { path: engine_path })
                }
                _ => {
                    return Err(Error::msg(format!("Engine path '{engine_path}' is a file, but is not a crate manifest Cargo.toml")));
                }
            },
            ft if ft.is_dir() => {
                let cargo_toml_path = engine_path.join(CARGO_TOML_NAME);

                if !cargo_toml_path.exists() {
                    return Err(Error::msg(format!(
                        "Engine path '{engine_path}' is a directory, but is not a crate"
                    )));
                }

                validate_engine_manifest(&cargo_toml_path)?;

                let engine_path = engine_path.to_owned();
                Ok(Dependency::Path { path: engine_path })
            }
            _ => {
                return Err(Error::msg(format!(
                    "Engine path '{engine_path}' is not a file or directory"
                )));
            }
        },
    }
}

pub fn validate_engine_manifest(cargo_toml_path: &Utf8Path) -> Result<(), Error> {
    let manifest: cargo_toml::Manifest = match cargo_toml::Manifest::from_path(cargo_toml_path) {
        Ok(manifest) => manifest,
        Err(err) => {
            return Err(Error::msg(format!(
                "Failed to read crate manifest '{cargo_toml_path}': {err:?}"
            )));
        }
    };

    let package = match &manifest.package {
        Some(package) => package,
        None => {
            return Err(Error::msg(format!(
                "'{cargo_toml_path}' is not an Arcana engine crate"
            )));
        }
    };

    if package.name != "arcana" {
        return Err(Error::msg(format!(
            "'{cargo_toml_path}' is not an Arcana engine crate"
        )));
    }

    Ok(())
}
