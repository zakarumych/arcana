//! Editor hot-loads plugins.
//! This presents a challenge because references to plugin data must not survive after the plugin is unloaded.
//!
//! To solve this the editor places anything that may reference plugin data into a `Container`.
//!
//! When container is created it loads all plugins and imports everything plugins exports.
//!
//! Metadata can be fetched from the container.
//! And it can be used to run game instances.
//! Container will also handle communication with the game instances, ensuring that references
//! from the plugins are not leaked.
//!
//! To load new plugins a new container must be created and old one can be dropped when it's no longer needed.

use core::fmt;
use std::{
    borrow::Borrow,
    collections::VecDeque,
    fs::File,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

use arcana::{
    plugin::{check_arcana_instance, ArcanaPlugin},
    project::Dependency,
    Ident,
};
use hashbrown::{hash_map::RawEntryMut, HashMap, HashSet};
use miette::{Context, Diagnostic, Severity};
use thiserror::Error;

use crate::error::{FileCopyError, FileOpenError, FileReadError};

#[derive(Diagnostic, Error, Debug)]
#[error("Plugin not found")]
#[diagnostic(code(ed::container::plugin_not_found), url(docsrs))]
pub struct PluginNotFound {
    #[source]
    source: libloading::Error,
    path: PathBuf,
}

#[derive(Diagnostic, Error, Debug)]
#[error("Dynamic lib is not a plugins library")]
#[diagnostic(code(ed::container::plugin_not_found), url(docsrs))]
pub struct NotPluginsLibrary {
    #[source]
    source: libloading::Error,
    path: PathBuf,
}

#[derive(Diagnostic, Error, Debug)]
#[error("Plugins library engine version mismatch. Expected: {expected}, found: {found}")]
#[diagnostic(
    code(ed::container::version_mismatch),
    help("update engine version in plugins lib"),
    url(docsrs)
)]
pub struct PluginsLibraryEngineVersionMismatch {
    expected: &'static str,
    found: &'static str,
}

#[derive(Diagnostic, Error, Debug)]
#[error("Plugins library engine is not linked")]
#[diagnostic(
    code(ed::container::engine_not_linked),
    help("investigate why plugins library linked to a different instance of the engine"),
    url(docsrs)
)]
pub struct PluginsLibraryEngineUnlinked;

#[derive(Diagnostic, Error, Debug)]
#[error("Circular dependency between plugins: {0} <-> {1}")]
#[diagnostic(
    code(ed::container::circular_dependency),
    help("Break circular dependency"),
    url(docsrs)
)]
pub struct CircularDependency(pub Ident, pub Ident);

#[derive(Diagnostic, Error, Debug)]
#[error("Missing dependency: {dependency} for plugin {plugin}")]
#[diagnostic(
    code(ed::container::missing_dependency),
    help("Add missing dependency"),
    url(docsrs)
)]
pub struct MissingDependency {
    pub plugin: Ident,
    pub dependency: Dependency,
}

#[derive(Diagnostic, Error, Debug)]
#[error("Failed to load plugins")]
#[diagnostic(
    code(ed::container::plugins_error),
    help("Fix related errors"),
    url(docsrs)
)]
pub struct PluginsError {
    #[related]
    pub circular_dependencies: Vec<CircularDependency>,

    #[related]
    pub missing_dependencies: Vec<MissingDependency>,
}

/// Container holds an instance of plugin library and must be supplied to the game instance to use plugins.
/// This ensures that no references to plugin data are leaked beyond the lifetime of the plugin library.
struct Loaded {
    /// List of plugins loaded from the library.
    /// In dependency-first order.
    plugins: Vec<(Ident, &'static dyn ArcanaPlugin)>,

    /// Linked library.
    /// It is only used to keep the library loaded.
    /// It must be last member of the struct to ensure it is dropped last.
    _lib: libloading::Library,

    /// Remove the temporary file after library is unloaded.
    _tmp: TmpPath,
}

impl Drop for Loaded {
    fn drop(&mut self) {
        tracing::info!("Dropping loaded library");
    }
}

#[derive(Clone)]
pub struct Container {
    active_plugins: Arc<[(Ident, &'static dyn ArcanaPlugin)]>,

    // Unload library last.
    loaded: Arc<Loaded>,
}

impl fmt::Debug for Container {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct Plugins<'a> {
            plugins: &'a [(Ident, &'a dyn ArcanaPlugin)],
        }

        impl fmt::Debug for Plugins<'_> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut list = f.debug_list();

                for (name, _) in self.plugins {
                    list.entry(name);
                }

                list.finish()
            }
        }

        f.debug_struct("Container")
            .field(
                "plugins",
                &Plugins {
                    plugins: &self.active_plugins,
                },
            )
            .finish()
    }
}

impl Container {
    /// Create a new container from same library with the given plugins enabled.
    pub fn with_plugins(&self, enabled_plugins: &HashSet<Ident>) -> Self {
        let active_plugins = get_active_plugins(&self.loaded, enabled_plugins);
        Container {
            loaded: self.loaded.clone(),
            active_plugins: active_plugins.into(),
        }
    }

    pub fn has(&self, name: Ident) -> bool {
        self.loaded.plugins.iter().any(|(n, _)| *n == name)
    }

    pub fn is_active(&self, name: Ident) -> bool {
        self.active_plugins.iter().any(|(n, _)| *n == name)
    }

    // pub fn get(&self, name: Ident) -> Option<&dyn ArcanaPlugin> {
    //     let (_, p) = self.loaded.plugins.iter().find(|(n, _)| *n == name)?;
    //     Some(*p)
    // }

    pub fn plugins<'a>(&'a self) -> impl Iterator<Item = (Ident, &'a dyn ArcanaPlugin)> + 'a {
        self.active_plugins.iter().copied()
    }
}

impl PartialEq for Container {
    fn eq(&self, other: &Self) -> bool {
        if !Arc::ptr_eq(&self.loaded, &other.loaded) {
            return false;
        }

        if self.active_plugins.len() != other.active_plugins.len() {
            return false;
        }

        for ((name, _), (other_name, _)) in
            self.active_plugins.iter().zip(other.active_plugins.iter())
        {
            if name != other_name {
                return false;
            }
        }

        true
    }
}

impl Eq for Container {}

/// Sort plugins placing dependencies first.
/// Errors if there are circular dependencies or missing dependencies.
fn sort_plugins<'a>(
    plugins: &[(Ident, &'a dyn ArcanaPlugin)],
) -> Result<Vec<(Ident, &'a dyn ArcanaPlugin)>, PluginsError> {
    let mut queue = VecDeque::new();

    for (name, _) in plugins {
        queue.push_back(*name);
    }

    let has = |name: Ident| -> bool { plugins.iter().any(|(n, _)| *n == name) };

    let get = |name: Ident| -> &'a dyn ArcanaPlugin {
        plugins.iter().find(|(n, _)| *n == name).unwrap().1
    };

    let get_pair = |name: Ident| -> (Ident, &'a dyn ArcanaPlugin) {
        *plugins.iter().find(|(n, _)| *n == name).unwrap()
    };

    let mut circular_dependencies = Vec::new();
    let mut missing_dependencies = Vec::new();

    let mut pending = HashSet::new();
    let mut sorted = HashSet::new();
    let mut result = Vec::new();

    while let Some(name) = queue.pop_front() {
        if sorted.contains(&name) {
            continue;
        }
        pending.insert(name);

        let plugin = get(name);

        let mut defer = false;
        for (dep_name, dependency) in plugin.dependencies() {
            if sorted.contains(&dep_name) {
                continue;
            }

            if pending.contains(&dep_name) {
                circular_dependencies.push(CircularDependency(name, dep_name));
                continue;
            }

            if !has(dep_name) {
                missing_dependencies.push(MissingDependency {
                    plugin: dep_name,
                    dependency: dependency.clone(),
                });
                continue;
            };

            if !defer {
                defer = true;
                queue.push_front(name);
            }

            queue.push_front(dep_name);
        }

        if !defer {
            sorted.insert(name);
            result.push(name);
        }
    }

    if !circular_dependencies.is_empty() || !missing_dependencies.is_empty() {
        return Err(PluginsError {
            circular_dependencies,
            missing_dependencies,
        });
    }

    let mut sorted_plugins = Vec::new();

    for name in result {
        let (name, plugin) = get_pair(name);
        sorted_plugins.push((name, plugin));
    }

    assert_eq!(plugins.len(), sorted_plugins.len());
    Ok(sorted_plugins)
}

struct TmpPath {
    path: PathBuf,
    remove: bool,
}

impl Borrow<Path> for TmpPath {
    fn borrow(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpPath {
    fn drop(&mut self) {
        if self.remove {
            if let Err(err) = std::fs::remove_file(&self.path) {
                tracing::warn!(
                    "Failed to remove temp file '{}': {}",
                    self.path.display(),
                    err
                );
            }
        }
    }
}

/// Find new appropriate name for the dylib at the given path.
/// Copies the dylib to the new path and returns the new path.
fn copy_dylib(path: &Path, new_path: PathBuf) -> miette::Result<TmpPath> {
    let mut copied = false;
    if !new_path.exists() {
        std::fs::copy(&path, &new_path).map_err(|source| FileCopyError {
            from: path.to_owned(),
            to: new_path.to_owned(),
            source,
        })?;

        tracing::info!(
            "Copied dylib from '{}' to '{}'",
            path.display(),
            new_path.display()
        );

        copied = true;
    }

    Ok(TmpPath {
        path: new_path,
        remove: copied,
    })
}

/// Find new appropriate name for the dylib at the given path.
/// Copies the dylib to the new path and returns the new path.
fn find_tmp_path(path: &Path) -> miette::Result<PathBuf> {
    let Some(file_stem) = path.file_stem() else {
        return Err(miette::miette! {
            severity = Severity::Error,
            code = "copy_dylib::filename",
            help = "Dylib path must have a filename",
            "Bad dylib path: {}", path.display()
        });
    };

    let ext = path.extension();

    let file = File::open(path)
        .map_err(|source| FileOpenError {
            path: path.to_owned(),
            source,
        })
        .wrap_err("Failed to open dylib file")?;

    let hash = arcana::hash::stable_hash_read(file)
        .map_err(|source| FileReadError {
            path: path.to_owned(),
            source,
        })
        .wrap_err("Failed to hash dylib file")?;

    let mut new_filename = file_stem.to_owned();
    new_filename.push(format!("-{}", hash));

    if let Some(ext) = ext {
        new_filename.push(".");
        new_filename.push(ext);
    }

    let new_path = path.with_file_name(new_filename);
    Ok(new_path)
}

pub struct Loader {
    loaded: HashMap<PathBuf, Arc<Loaded>>,
}

impl Loader {
    pub fn new() -> Self {
        Loader {
            loaded: HashMap::new(),
        }
    }

    /// Load plugins from the given path and create a new container.
    ///
    /// This function checks that the library exists, can be linked, contains necessary symbols
    /// and verify version compatibility.
    ///
    /// This makes it improbable to load bad library by accident, yet easy to do so intentionally.
    /// At the end this function is technically unsound, but it is the best we can do.
    ///
    /// It also checks that plugin dependencies are satisfied and no circular dependencies exist.
    pub fn load(
        &mut self,
        path: &Path,
        enabled_plugins: &HashSet<Ident>,
    ) -> miette::Result<Container> {
        let new_path = find_tmp_path(path).wrap_err("Failed to find temp path for dylib")?;

        let loaded = match self.loaded.raw_entry_mut().from_key(&*new_path) {
            RawEntryMut::Occupied(entry) => entry.get().clone(),
            RawEntryMut::Vacant(entry) => {
                let loaded = load_lib(path, new_path.clone())?;
                let loaded = Arc::new(loaded);
                entry.insert(new_path, loaded.clone());
                loaded
            }
        };

        let active_plugins = get_active_plugins(&loaded, enabled_plugins);

        Ok(Container {
            loaded,
            active_plugins: active_plugins.into(),
        })
    }
}

/// Activate plugins based on enabled plugins.
///
/// Plugin is activated if it is enabled and all its dependencies are active.
fn get_active_plugins(
    loaded: &Loaded,
    enabled_plugins: &HashSet<Ident>,
) -> Vec<(Ident, &'static dyn ArcanaPlugin)> {
    let mut active_plugins = Vec::new();
    let mut active_set = HashSet::new();

    'a: for &(name, plugin) in &loaded.plugins {
        if !enabled_plugins.contains(&name) {
            continue;
        }

        for (dep_name, _) in plugin.dependencies() {
            if !active_set.contains(&dep_name) {
                continue 'a;
            }
        }

        active_set.insert(name);
        active_plugins.push((name, plugin));
    }

    active_plugins
}

fn load_lib(path: &Path, new_path: PathBuf) -> miette::Result<Loaded> {
    let tmp = copy_dylib(path, new_path).wrap_err("Failed to copy dylib")?;

    // Safety: nope.
    let r = unsafe { libloading::Library::new(&tmp.path) };

    let lib = match r {
        Ok(lib) => lib,
        Err(source) => {
            return Err(PluginNotFound {
                source,
                path: path.to_owned(),
            }
            .into())
        }
    };

    type ArcanaVersionFn = fn() -> &'static str;
    type ArcanaLinkedFn = fn(&AtomicBool) -> bool;
    type ArcanaPluginsFn = fn() -> Vec<(Ident, &'static dyn ArcanaPlugin)>;

    let arcana_version =
        unsafe { lib.get::<ArcanaVersionFn>(b"arcana_version\0") }.map_err(|source| {
            PluginNotFound {
                source,
                path: path.to_owned(),
            }
        })?;

    let arcana_linked =
        unsafe { lib.get::<ArcanaLinkedFn>(b"arcana_linked\0") }.map_err(|source| {
            NotPluginsLibrary {
                source,
                path: path.to_owned(),
            }
        })?;

    let arcana_plugins =
        unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugins\0") }.map_err(|source| {
            NotPluginsLibrary {
                source,
                path: path.to_owned(),
            }
        })?;

    let arcana_version = arcana_version();
    if arcana_version != arcana::version() {
        return Err(PluginsLibraryEngineVersionMismatch {
            expected: arcana::version(),
            found: arcana_version,
        }
        .into());
    }

    if !check_arcana_instance(*arcana_linked) {
        return Err(PluginsLibraryEngineUnlinked.into());
    }

    let plugins = sort_plugins(&arcana_plugins())?;

    Ok(Loaded {
        plugins,
        _lib: lib,
        _tmp: tmp,
    })
}
