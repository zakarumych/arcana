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
    sync::{Arc, atomic::AtomicBool},
};

use hashbrown::{HashMap, HashSet, hash_map::RawEntryMut};

use arcana::{
    Name,
    error::{Error, UnifyError, WithContext, error},
    plugin::{ArcanaPlugin, check_arcana_instance},
};

use crate::{
    error::{FileCopyError, FileOpenError, FileReadError},
    project::Dependency,
};

#[derive(thiserror::Error, Debug)]
#[error("Plugin not found")]
pub struct PluginNotFound {
    #[source]
    source: libloading::Error,
    path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
#[error("Dynamic lib is not a plugins library")]
pub struct NotPluginsLibrary {
    #[source]
    source: libloading::Error,
    path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
#[error("Plugins library engine version mismatch. Expected: {expected}, found: {found}")]
pub struct PluginsLibraryEngineVersionMismatch {
    expected: &'static str,
    found: &'static str,
}

#[derive(thiserror::Error, Debug)]
#[error("Plugins library engine is not linked")]
pub struct PluginsLibraryEngineUnlinked;

#[derive(thiserror::Error, Debug)]
#[error("Circular dependency between plugins: {0} <-> {1}")]
pub struct CircularDependency(pub Name, pub Name);

#[derive(thiserror::Error, Debug)]
#[error("Missing dependency: {dependency} for plugin {plugin}")]
pub struct MissingDependency {
    pub plugin: Name,
    pub dependency: Dependency,
}

#[derive(thiserror::Error, Debug)]
pub struct PluginsError {
    pub circular_dependencies: Vec<CircularDependency>,
    pub missing_dependencies: Vec<MissingDependency>,
}

impl fmt::Display for PluginsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Plugins error: ")?;

        if !self.circular_dependencies.is_empty() {
            write!(f, "Circular dependencies found: ")?;
            for dep in &self.circular_dependencies {
                write!(f, "{} <-> {} ", dep.0, dep.1)?;
            }
        }

        if !self.missing_dependencies.is_empty() {
            if !self.circular_dependencies.is_empty() {
                write!(f, "\n")?;
            }
            write!(f, "Missing dependencies: ")?;
            for dep in &self.missing_dependencies {
                write!(f, "{} for plugin {} ", dep.dependency, dep.plugin)?;
            }
        }

        Ok(())
    }
}

/// Container holds an instance of plugin library and must be supplied to the game instance to use plugins.
/// This ensures that no references to plugin data are leaked beyond the lifetime of the plugin library.
struct Loaded {
    /// List of plugins loaded from the library.
    /// In dependency-first order.
    plugins: Arc<[(Name, ArcanaPlugin)]>,

    /// Linked library.
    /// It is only used to keep the library loaded.
    /// Drop it after plugins are dropped
    /// but before the file is removed.
    _lib: libloading::Library,

    /// Remove the temporary file after library is unloaded.
    _tmp: TmpPath,
}

impl Drop for Loaded {
    fn drop(&mut self) {
        tracing::info!("Dropping loaded library '{}'", self._tmp.path.display());
    }
}

/// Container with loaded and initialized plugins.
/// Clone of it must be kept where exported items are stored,
/// until after all exported items are dropped.
#[derive(Clone, Default)]
pub struct Plugins {
    active_plugins: HashSet<Name>,

    // Unload library last.
    loaded: Option<Arc<Loaded>>,
}

impl fmt::Debug for Plugins {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.loaded.is_none() {
            return f.debug_struct("Plugins {{ #empty# }}").finish();
        }

        struct Plugins<I> {
            plugins: I,
        }

        impl<'a, I> fmt::Debug for Plugins<I>
        where
            I: Iterator<Item = (Name, &'a ArcanaPlugin)> + Clone,
        {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut list = f.debug_list();

                for (name, _) in self.plugins.clone() {
                    list.entry(&name);
                }

                list.finish()
            }
        }

        f.debug_tuple("Plugins")
            .field(&Plugins {
                plugins: self.iter(),
            })
            .finish()
    }
}

impl Plugins {
    /// Create a new container from same library with the given plugins enabled.
    pub fn with_plugins(&self, enabled_plugins: &HashSet<Name>) -> Self {
        let active_plugins = self.loaded.as_ref().map_or(HashSet::new(), |loaded| {
            get_active_plugins(loaded, enabled_plugins)
        });
        Plugins {
            loaded: self.loaded.clone(),
            active_plugins,
        }
    }

    pub fn has(&self, name: Name) -> bool {
        let Some(loaded) = self.loaded.as_ref() else {
            return false;
        };
        loaded.plugins.iter().any(|(n, _)| *n == name)
    }

    pub fn is_active(&self, name: Name) -> bool {
        self.active_plugins.contains(&name)
    }

    pub fn get(&self, name: Name) -> Option<&ArcanaPlugin> {
        let Some(loaded) = self.loaded.as_ref() else {
            return None;
        };

        for &(plugin_name, ref plugin) in loaded.plugins.iter() {
            if plugin_name == name {
                return Some(plugin);
            }
        }

        None
    }

    /// Returns all active plugins loaded from the library.
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Name, &'a ArcanaPlugin)> + Clone + 'a {
        let loaded_plugins = match &self.loaded {
            None => &[],
            Some(loaded) => &*loaded.plugins,
        };

        loaded_plugins.iter().filter_map(|(name, plugin)| {
            if self.active_plugins.contains(name) {
                Some((*name, plugin))
            } else {
                None
            }
        })
    }
}

impl PartialEq for Plugins {
    fn eq(&self, other: &Self) -> bool {
        match (&self.loaded, &other.loaded) {
            (Some(lhs), Some(rhs)) if !Arc::ptr_eq(lhs, rhs) => return false,
            (Some(_), None) | (None, Some(_)) => return false,
            _ => {}
        };

        if self.active_plugins != other.active_plugins {
            return false;
        }

        true
    }
}

impl Eq for Plugins {}

/// Sort plugins placing dependencies first.
/// Errors if there are circular dependencies or missing dependencies.
fn sort_plugins<'a>(plugins: &mut [(Name, ArcanaPlugin)]) -> Result<(), PluginsError> {
    let mut order = Vec::new();

    {
        let plugins = &*plugins;
        let mut queue = VecDeque::new();

        for (name, _) in plugins {
            queue.push_back(*name);
        }

        let has = |name: Name| -> bool { plugins.iter().any(|(n, _)| *n == name) };

        let get =
            |name: Name| -> &ArcanaPlugin { &plugins.iter().find(|(n, _)| *n == name).unwrap().1 };

        let mut circular_dependencies = Vec::new();
        let mut missing_dependencies = Vec::new();

        let mut pending = HashSet::new();
        let mut sorted = HashSet::new();

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
                order.push(name);
            }
        }

        if !circular_dependencies.is_empty() || !missing_dependencies.is_empty() {
            return Err(PluginsError {
                circular_dependencies,
                missing_dependencies,
            });
        }
    }

    plugins.sort_by_key(|(name, _)| {
        order
            .iter()
            .position(|n| n == name)
            .expect("Plugin should be found in sorted list")
    });

    Ok(())
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
            if let Err(error) = std::fs::remove_file(&self.path) {
                tracing::warn!(
                    "Failed to remove temp file '{}': {}",
                    self.path.display(),
                    error
                );
            }
        }
    }
}

/// Find new appropriate name for the dylib at the given path.
/// Copies the dylib to the new path and returns the new path.
fn copy_dylib(path: &Path, new_path: PathBuf) -> Result<TmpPath, Error> {
    let mut copied = false;
    if !new_path.exists() {
        std::fs::copy(&path, &new_path)
            .map_err(|source| FileCopyError {
                from: path.to_owned(),
                to: new_path.to_owned(),
                source,
            })
            .unify_error()?;

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
fn find_tmp_path(path: &Path) -> Result<PathBuf, Error> {
    let Some(file_stem) = path.file_stem() else {
        return Err(error!("Bad dylib path: {}", path.display()));
    };

    let ext = path.extension();

    let file = File::open(path)
        .map_err(|source| FileOpenError {
            path: path.to_owned(),
            source,
        })
        .with_context("Failed to open dylib file")?;

    let hash = arcana::hash::stable_hash_read(file)
        .map_err(|source| FileReadError {
            path: path.to_owned(),
            source,
        })
        .with_context("Failed to hash dylib file")?;

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
    /// This makes it improbable to load bad library by accName, yet easy to do so intentionally.
    /// At the end this function is technically unsound, but it is the best we can do.
    ///
    /// It also checks that plugin dependencies are satisfied and no circular dependencies exist.
    pub fn load(&mut self, path: &Path, enabled_plugins: &HashSet<Name>) -> Result<Plugins, Error> {
        let new_path = find_tmp_path(path).with_context("Failed to find temp path for dylib")?;

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

        Ok(Plugins {
            loaded: Some(loaded),
            active_plugins: active_plugins.into(),
        })
    }
}

/// Activate plugins based on enabled plugins.
///
/// Plugin is activated if it is enabled and all its dependencies are active.
fn get_active_plugins(loaded: &Loaded, enabled_plugins: &HashSet<Name>) -> HashSet<Name> {
    let mut active_set = HashSet::new();

    'a: for &(name, ref plugin) in loaded.plugins.iter() {
        if !enabled_plugins.contains(&name) {
            continue;
        }

        for (dep_name, _) in plugin.dependencies() {
            if !active_set.contains(&dep_name) {
                continue 'a;
            }
        }

        active_set.insert(name);
    }

    active_set
}

fn load_lib(path: &Path, new_path: PathBuf) -> Result<Loaded, Error> {
    tracing::info!("Loading library from '{}'", path.display());

    let tmp = copy_dylib(path, new_path).with_context("Failed to copy dylib")?;

    // Safety: nope.
    let r = unsafe { libloading::Library::new(&tmp.path) };

    let lib = match r {
        Ok(lib) => lib,
        Err(source) => {
            return Err(PluginNotFound {
                source,
                path: path.to_owned(),
            })
            .unify_error();
        }
    };

    // Type signatures must be synchronized with code generated by `arcana_project`.
    type ArcanaVersionFn = fn() -> &'static str;
    type ArcanaLinkedFn = fn(&AtomicBool) -> bool;
    type ArcanaPluginsFn = fn() -> Vec<(Name, ArcanaPlugin)>;

    let arcana_version = unsafe { lib.get::<ArcanaVersionFn>(b"arcana_version\0") }
        .map_err(|source| PluginNotFound {
            source,
            path: path.to_owned(),
        })
        .unify_error()?;

    let arcana_linked = unsafe { lib.get::<ArcanaLinkedFn>(b"arcana_linked\0") }
        .map_err(|source| NotPluginsLibrary {
            source,
            path: path.to_owned(),
        })
        .unify_error()?;

    let arcana_plugins = unsafe { lib.get::<ArcanaPluginsFn>(b"arcana_plugins\0") }
        .map_err(|source| NotPluginsLibrary {
            source,
            path: path.to_owned(),
        })
        .unify_error()?;

    let arcana_version = arcana_version();
    if arcana_version != arcana::version() {
        return Err(PluginsLibraryEngineVersionMismatch {
            expected: arcana::version(),
            found: arcana_version,
        })
        .unify_error();
    }

    if !check_arcana_instance(*arcana_linked) {
        return Err(PluginsLibraryEngineUnlinked).unify_error();
    }

    let mut plugins = arcana_plugins();
    sort_plugins(&mut plugins).unify_error()?;

    Ok(Loaded {
        plugins: plugins.into(),
        _lib: lib,
        _tmp: tmp,
    })
}
