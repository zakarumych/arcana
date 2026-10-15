use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use arcana::{
    Ident, Name,
    assets::{
        AssetId,
        import::{ImportContext, ImportError, ImporterDesc, ImporterId, Missing},
    },
    error::{Error, WithContext, fail},
    hash::{HashMap, HashSet},
    id::TimeUidGen,
    model::Value,
    plugin::PluginsHub,
};
use base64::{
    Engine,
    alphabet::URL_SAFE,
    engine::general_purpose::{GeneralPurpose, NO_PAD},
};
use rand::random;

use url::Url;

use crate::{
    assets::{
        AssetDatabase,
        fetcher::Fetcher,
        meta::{AssetMeta, SourceMeta},
        registry::AssetRegistry,
    },
    plugins::Plugins,
    project::Project,
};

const MAX_ITEM_ATTEMPTS: usize = 10;

/// Asset repository for Arcana Editor.
pub struct AssetStore {
    /// Base path to the asset store.
    base: PathBuf,

    /// Path to temporaries
    temporaries: PathBuf,

    /// Information about available asset importers.
    /// Built-in and plugin importers can import assets from various sources.
    /// Importer instances come from the simulation hub when running queued imports.
    importers: HashMap<ImporterId, ImporterDesc>,

    /// Asset ID generator.
    /// Currently initialized with a random node ID.
    id_gen: TimeUidGen,

    /// Asset registry to store and load.
    registry: AssetRegistry,

    database: AssetDatabase,

    /// Converts source which may be relative or an URL into PathBuf.
    fetcher: Fetcher,
}

impl AssetStore {
    pub fn new(project: &Project) -> Result<Self, Error> {
        let base = project.root_path().join("assets");
        let temporaries = base.join("tmp");
        let registry = base.join("registry");
        let sources = base.join("sources");

        std::fs::create_dir_all(&temporaries)
            .with_context("Failed to create import temporary directory")?;
        let base = std::fs::canonicalize(base)
            .with_context("Failed to resolve project asset directory")?;
        let registry = AssetRegistry::new(registry)?;
        let database = AssetDatabase::new(&base, &registry)?;

        Ok(AssetStore {
            base,
            temporaries,
            id_gen: TimeUidGen::random(),
            importers: arcana::assets::builtin_importers()
                .into_iter()
                .map(|info| (info.id, info.desc))
                .collect(),
            registry,
            database,
            fetcher: Fetcher::new(sources)
                .with_context("Failed to open temporary sources path for asset store")?,
        })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn database(&self) -> &AssetDatabase {
        &self.database
    }

    pub fn database_mut(&mut self) -> &mut AssetDatabase {
        &mut self.database
    }

    pub fn refresh(&mut self) -> Result<(), Error> {
        self.database.refresh(&self.registry)
    }

    pub fn deimport(&mut self, source: &Path) -> Result<(), Error> {
        self.database.deimport(source, &self.registry)
    }

    pub fn importers(&self) -> impl Iterator<Item = (ImporterId, &ImporterDesc)> {
        self.importers.iter().map(|(id, desc)| (*id, desc))
    }

    pub fn registry(&self) -> &AssetRegistry {
        &self.registry
    }

    pub fn find_importers(
        &self,
        target: Ident,
        format: Option<Name>,
        extension: Option<&str>,
    ) -> Vec<(ImporterId, &ImporterDesc)> {
        let mut found = Vec::new();
        for (id, desc) in self.importers.iter() {
            if desc.target != target {
                continue;
            }

            if format.map_or(false, |f| !desc.formats.contains(&f)) {
                continue;
            }

            if extension.map_or(false, |ext| !desc.extensions.contains(&ext)) {
                continue;
            }

            found.push((*id, desc));
        }

        found
    }

    pub fn store(
        &mut self,
        hub: &PluginsHub,
        source: &str,
        importer: ImporterId,
        config: Value,
    ) -> Result<AssetId, Error> {
        let source = resolve_project_source(&self.base, &self.base, source)?;
        self.import_one(
            hub,
            source,
            importer,
            config,
            &mut Vec::new(),
            &mut HashSet::default(),
        )
    }

    fn import_one(
        &mut self,
        hub: &PluginsHub,
        source: PathBuf,
        importer_id: ImporterId,
        config: Value,
        active: &mut Vec<(PathBuf, Ident)>,
        refreshed: &mut HashSet<(PathBuf, Ident)>,
    ) -> Result<AssetId, Error> {
        let Some(desc) = self.importers.get(&importer_id).cloned() else {
            fail!("Importer {importer_id} is not found");
        };
        let Some(importer) = hub.importers.get(&importer_id) else {
            fail!("Importer {importer_id} is not available");
        };
        if active.len() >= 32 || active.contains(&(source.clone(), desc.target)) {
            fail!(
                "Cyclic or excessively deep asset dependency at '{}'",
                source.display()
            );
        }
        let source_url = Url::from_file_path(&source)
            .map_err(|()| Error::msg("Invalid source path".to_owned()))?;
        let mut metadata = SourceMeta::open_project(&self.base, source.clone())
            .with_context("Failed to read asset metadata")?;
        let old = metadata.get_asset(desc.target);
        let id = old
            .map(|meta| meta.id)
            .unwrap_or_else(|| AssetId::generate(&mut self.id_gen));
        if let Some(old) = old
            && old.config == config
            && old.importer == Some(importer_id)
            && old.dependencies.is_empty()
            && !old.sources.is_empty()
            && !old.needs_reimport(&source_url)
            && self.registry.contains_blob(old.blob)
        {
            self.registry
                .publish(old.id, old.blob, old.registry_version());
            refreshed.insert((source.clone(), desc.target));
            return Ok(old.id);
        }
        let version = old
            .map_or(Some(1), |meta| meta.registry_version().checked_add(1))
            .ok_or_else(|| Error::msg("Asset version exhausted".to_owned()))?;
        active.push((source.clone(), desc.target));
        let result = (|| {
            for attempt in 0..MAX_ITEM_ATTEMPTS {
                let tmp = make_temporary(&self.temporaries)
                    .with_context("Failed to reserve import temporary")?;
                let cleanup = Temporary(tmp.clone());
                let modified = source
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .with_context("Failed to inspect import source")?;
                let mut stamps = HashMap::default();
                stamps.insert(source_url.as_str().into(), modified);
                let mut dependencies = Vec::new();
                let mut ctx = StoreContext {
                    base: &self.base,
                    parent: source.parent().unwrap(),
                    source_url: &source_url,
                    sources: &mut self.fetcher,
                    stamps: &mut stamps,
                    dependencies: &mut dependencies,
                    registry: &self.registry,
                    refreshed,
                };
                let imported = importer.import(&source, &tmp, config.clone(), &mut ctx);
                match imported {
                    Ok(()) => {
                        let blob = self
                            .registry
                            .stage_file(&tmp)
                            .with_context("Failed to store imported artifact")?;
                        metadata
                            .add_asset(AssetMeta {
                                id,
                                target: desc.target,
                                blob,
                                format: None,
                                dependencies,
                                sources: stamps,
                                config: config.clone(),
                                version,
                                importer: Some(importer_id),
                            })
                            .with_context("Failed to persist imported asset metadata")?;
                        self.registry.publish(id, blob, version);
                        refreshed.insert((source.clone(), desc.target));
                        drop(cleanup);
                        return Ok(id);
                    }
                    Err(ImportError::Other { reason }) => {
                        fail!("Failed to import '{}': {reason}", source.display())
                    }
                    Err(ImportError::Requires {
                        sources,
                        dependencies,
                    }) => {
                        drop(cleanup);
                        if attempt + 1 == MAX_ITEM_ATTEMPTS {
                            fail!("Maximum import attempts reached for '{}'", source.display());
                        }
                        for extra in sources {
                            let url = source_url
                                .join(&extra)
                                .with_context("Invalid additional source URL")?;
                            if url.scheme() == "file" {
                                let path = url
                                    .to_file_path()
                                    .map_err(|()| Error::msg("Invalid file URL".to_owned()))?;
                                resolve_project_source(
                                    &self.base,
                                    source.parent().unwrap(),
                                    &path.to_string_lossy(),
                                )?;
                            } else if url.scheme() != "data" {
                                fail!("Unsupported additional source URL '{url}'");
                            }
                            self.fetcher
                                .fetch(&url)
                                .with_context("Failed to fetch additional source")?;
                        }
                        for dependency in dependencies {
                            let dependency_source = resolve_project_source(
                                &self.base,
                                source.parent().unwrap(),
                                &dependency.source,
                            )?;
                            let old =
                                SourceMeta::open_project(&self.base, dependency_source.clone())
                                    .with_context("Failed to read dependency metadata")?;
                            let persisted = old.get_asset(dependency.target);
                            let (dep_importer, dep_config) = if let Some(meta) = persisted
                                && let Some(id) = meta.importer
                                && self
                                    .importers
                                    .get(&id)
                                    .is_some_and(|desc| desc.target == dependency.target)
                            {
                                (id, meta.config.clone())
                            } else {
                                let extension = dependency_source
                                    .extension()
                                    .and_then(|extension| extension.to_str());
                                let (id, desc) = exactly_one_importer(
                                    self.find_importers(
                                        dependency.target,
                                        dependency.format,
                                        extension,
                                    ),
                                    dependency.target,
                                    dependency.format,
                                    extension,
                                )?;
                                (id, desc.config.1.clone())
                            };
                            self.import_one(
                                hub,
                                dependency_source,
                                dep_importer,
                                dep_config,
                                active,
                                refreshed,
                            )?;
                        }
                    }
                }
            }
            unreachable!()
        })();
        active.pop();
        result
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) {
        self.importers.clear();
        self.importers.extend(
            arcana::assets::builtin_importers()
                .into_iter()
                .map(|info| (info.id, info.desc)),
        );
        for (_, plugin) in plugins.iter() {
            for importer in plugin.importers() {
                self.importers.insert(importer.id, importer.desc.clone());
            }
        }
    }
}

struct StoreContext<'a> {
    base: &'a Path,
    parent: &'a Path,
    source_url: &'a Url,
    sources: &'a mut Fetcher,
    stamps: &'a mut HashMap<arcana::smol_str::SmolStr, SystemTime>,
    dependencies: &'a mut Vec<AssetId>,
    registry: &'a AssetRegistry,
    refreshed: &'a HashSet<(PathBuf, Ident)>,
}

impl ImportContext for StoreContext<'_> {
    fn get_dependency(&mut self, source: &str, target: Ident) -> Result<AssetId, Missing> {
        let source = resolve_project_source(self.base, self.parent, source).map_err(|_| Missing)?;
        let url = Url::from_file_path(&source).map_err(|()| Missing)?;
        let metadata = SourceMeta::open_project(self.base, source.clone()).map_err(|_| Missing)?;
        let asset = metadata.get_asset(target).ok_or(Missing)?;
        if asset.sources.is_empty()
            || asset.needs_reimport(&url)
            || !self.registry.contains_blob(asset.blob)
            || (!asset.dependencies.is_empty() && !self.refreshed.contains(&(source, target)))
        {
            return Err(Missing);
        }
        if !self.dependencies.contains(&asset.id) {
            self.dependencies.push(asset.id);
        }
        for (source, stamp) in &asset.sources {
            self.stamps.insert(source.clone(), *stamp);
        }
        Ok(asset.id)
    }

    fn get_source(&mut self, source: &str) -> Result<PathBuf, Missing> {
        let source_url = self.source_url.join(source).map_err(|_| Missing)?;
        if source_url.scheme() == "file" {
            let path = source_url.to_file_path().map_err(|()| Missing)?;
            resolve_project_source(self.base, self.parent, &path.to_string_lossy())
                .map_err(|_| Missing)?;
        }
        let (path, modified) = self.sources.get(&source_url).ok_or(Missing)?;
        self.stamps.insert(source_url.as_str().into(), modified);
        Ok(path.to_path_buf())
    }
}

pub(super) fn resolve_project_source(
    base: &Path,
    parent: &Path,
    source: &str,
) -> Result<PathBuf, Error> {
    let candidate = parent.join(source);
    let mut current = PathBuf::new();
    for component in candidate.components() {
        current.push(component);
        if std::fs::symlink_metadata(&current).is_ok_and(|meta| super::meta::is_link(&meta)) {
            fail!("Asset sources cannot traverse symbolic links");
        }
    }
    let path = std::fs::canonicalize(&candidate).with_context("Failed to resolve asset source")?;
    if path.strip_prefix(base).is_ok_and(super::database::reserved)
        || path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("arc"))
    {
        fail!("Reserved asset storage path");
    }
    if !path.is_file() {
        fail!("Asset source must be a file");
    }
    Ok(path)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub struct DependencyError;

pub fn make_temporary(base: &Path) -> std::io::Result<PathBuf> {
    loop {
        let key: u128 = random();
        let key_bytes = key.to_le_bytes();
        let mut filename = [0; 26];
        let len = GeneralPurpose::new(&URL_SAFE, NO_PAD)
            .encode_slice(&key_bytes, &mut filename[..22])
            .unwrap();
        debug_assert_eq!(len, 22);
        filename[22..].copy_from_slice(b".tmp");
        let path = base.join(std::str::from_utf8(&filename).unwrap());
        match std::fs::File::create_new(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn exactly_one_importer<'a>(
    importers: Vec<(ImporterId, &'a ImporterDesc)>,
    target: Ident,
    format: Option<Name>,
    extension: Option<&str>,
) -> Result<(ImporterId, &'a ImporterDesc), Error> {
    if importers.len() > 1 {
        match (format, extension) {
            (None, None) => fail!(
                "Multiple importers found for target '{target}', try to narrow down with input format"
            ),
            (Some(format), None) => {
                fail!("Multiple importers found for target '{target}' and format '{format}'");
            }
            (None, Some(extension)) => {
                fail!("Multiple importers found for target '{target}' and extension '{extension}'");
            }
            (Some(format), Some(extension)) => {
                fail!(
                    "Multiple importers found for target '{target}', format {format} and extension '{extension}'"
                );
            }
        }
    }

    if importers.is_empty() {
        match (format, extension) {
            (None, None) => fail!(
                "No importers found for target '{target}', try to narrow down with input format"
            ),
            (Some(format), None) => {
                fail!("No importers found for target '{target}' and format '{format}'");
            }
            (None, Some(extension)) => {
                fail!("No importers found for target '{target}' and extension '{extension}'");
            }
            (Some(format), Some(extension)) => {
                fail!(
                    "No importers found for target '{target}', format {format} and extension '{extension}'"
                );
            }
        }
    }

    Ok(importers[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana::assets::{
        Loader,
        import::{AssetDependency, Importer},
    };
    use arcana::model::Model;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct TestImporter(Arc<AtomicUsize>);
    impl Importer for TestImporter {
        fn desc() -> ImporterDesc {
            ImporterDesc {
                name: arcana::name!(Test),
                formats: vec![],
                extensions: vec!["txt"],
                target: arcana::ident!(test),
                config: (Model::Bool, Value::Bool(false)),
            }
        }
        fn import(
            &self,
            source: &Path,
            output: &Path,
            config: Value,
            context: &mut dyn ImportContext,
        ) -> Result<(), ImportError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            if config == Value::Bool(true) {
                return Err(ImportError::Other {
                    reason: "forced failure".into(),
                });
            }
            let mut bytes = std::fs::read(source).unwrap();
            let text = String::from_utf8_lossy(&bytes).into_owned();
            if let Some(dependency) = text.strip_prefix("dep:") {
                let target = Self::desc().target;
                match context.get_dependency(dependency, target) {
                    Ok(id) => bytes.extend_from_slice(id.to_string().as_bytes()),
                    Err(_) => {
                        return Err(ImportError::Requires {
                            sources: vec![],
                            dependencies: vec![AssetDependency {
                                source: dependency.into(),
                                target,
                                format: None,
                            }],
                        });
                    }
                }
            }
            if text == "extra" {
                match context.get_source("extra.txt") {
                    Ok(path) => bytes.extend(std::fs::read(path).unwrap()),
                    Err(_) => {
                        return Err(ImportError::Requires {
                            sources: vec!["extra.txt".into()],
                            dependencies: vec![],
                        });
                    }
                }
            }
            if text == "retry" {
                return Err(ImportError::Requires {
                    sources: vec![],
                    dependencies: vec![],
                });
            }
            bytes.extend(format!("{config:?}").as_bytes());
            std::fs::write(output, bytes).unwrap();
            Ok(())
        }
    }

    struct Fixture {
        root: PathBuf,
        project: Project,
        store: AssetStore,
        hub: PluginsHub,
        importer: ImporterId,
        calls: Arc<AtomicUsize>,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("arcana-import-test-{}", random::<u128>()));
            let project = Project::new(
                arcana::ident!(test),
                &root,
                arcana::project::Dependency::Crates("*".into()),
                true,
            )
            .unwrap();
            let store = AssetStore::new(&project).unwrap();
            let mut fixture = Self {
                root,
                project,
                store,
                hub: PluginsHub::new(),
                importer: ImporterId::new(std::num::NonZeroU64::new(1).unwrap()),
                calls: Arc::new(AtomicUsize::new(0)),
            };
            fixture.register();
            fixture
        }
        fn register(&mut self) {
            self.store
                .importers
                .insert(self.importer, TestImporter::desc());
            self.hub
                .add_importer(self.importer, TestImporter(self.calls.clone()));
        }
        fn write(&self, name: &str, data: &str) {
            let path = self.store.base.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, data).unwrap();
            let file = std::fs::File::options().write(true).open(path).unwrap();
            file.set_times(std::fs::FileTimes::new().set_modified(
                SystemTime::now()
                    + std::time::Duration::from_secs(self.calls.load(Ordering::Relaxed) as u64 + 1),
            ))
            .unwrap();
        }
        fn import(&mut self, source: &str, config: Value) -> Result<AssetId, Error> {
            self.store.store(&self.hub, source, self.importer, config)
        }
        fn meta(&self, source: &str) -> AssetMeta {
            SourceMeta::open(self.store.base.join(source))
                .unwrap()
                .get_asset(TestImporter::desc().target)
                .unwrap()
                .clone()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn deimport_preserves_sources_tags_dependencies_and_shared_blobs() {
        let mut f = Fixture::new();
        f.write("one.txt", "same");
        f.write("other.txt", "same");
        f.write("parent.txt", "dep:child.txt");
        f.write("child.txt", "child");
        let one = f.import("one.txt", Value::Unit).unwrap();
        let other = f.import("other.txt", Value::Unit).unwrap();
        let parent = f.import("parent.txt", Value::Unit).unwrap();
        let child = f.meta("child.txt").id;
        let blob = f.meta("one.txt").blob;
        assert_eq!(blob, f.meta("other.txt").blob);
        // A source may have more than one imported target.
        let mut meta = SourceMeta::open(f.store.base.join("one.txt")).unwrap();
        let mut second = f.meta("one.txt");
        second.target = arcana::ident!(second);
        second.id = AssetId::new(std::num::NonZeroU64::new(123).unwrap());
        let second_id = second.id;
        meta.add_asset(second).unwrap();
        f.store.refresh().unwrap();
        f.store
            .database
            .set_tags(Path::new("one.txt"), vec!["Keep".into()])
            .unwrap();
        // Removing targets requires no importer instance or descriptor.
        f.store.importers.clear();
        f.hub = PluginsHub::new();
        let revision = f.store.registry.revision();
        f.store.deimport(Path::new("one.txt")).unwrap();
        f.store.deimport(Path::new("parent.txt")).unwrap();
        assert!(f.store.registry.revision() > revision);
        assert!(f.store.registry.contains_blob(blob));
        assert_eq!(
            std::fs::read_to_string(f.store.base.join("one.txt")).unwrap(),
            "same"
        );
        assert_eq!(
            std::fs::read_to_string(f.store.base.join("parent.txt")).unwrap(),
            "dep:child.txt"
        );
        for store in [&f.store, &AssetStore::new(&f.project).unwrap()] {
            for id in [one, second_id, parent] {
                assert!(futures::executor::block_on(store.registry.load(id)).is_err());
            }
            for id in [other, child] {
                assert!(futures::executor::block_on(store.registry.load(id)).is_ok());
            }
            let entry = store
                .database
                .entries()
                .iter()
                .find(|entry| entry.path == Path::new("one.txt"))
                .unwrap();
            assert!(entry.imported.is_empty());
            assert_eq!(entry.tags, ["keep"]);
        }
        assert_eq!(
            SourceMeta::open(f.store.base.join("one.txt"))
                .unwrap()
                .assets()
                .count(),
            0
        );
    }

    #[test]
    fn deimport_external_retains_identity_tags_and_supports_missing_source() {
        for missing in [false, true] {
            let mut f = Fixture::new();
            let source = f.root.join("external.txt");
            std::fs::write(&source, "external").unwrap();
            let source = std::fs::canonicalize(source).unwrap();
            let id = f.import(source.to_str().unwrap(), Value::Unit).unwrap();
            f.store.refresh().unwrap();
            f.store
                .database
                .set_tags(&source, vec!["Keep".into()])
                .unwrap();
            if missing {
                std::fs::remove_file(&source).unwrap();
            }
            f.store.deimport(&source).unwrap();
            f.store = AssetStore::new(&f.project).unwrap();
            assert!(futures::executor::block_on(f.store.registry.load(id)).is_err());
            let entry = f
                .store
                .database
                .entries()
                .iter()
                .find(|entry| entry.path == source)
                .unwrap();
            assert!(entry.imported.is_empty());
            assert_eq!(entry.tags, ["keep"]);
            assert_eq!(source.exists(), !missing);
            if !missing {
                assert_eq!(std::fs::read_to_string(&source).unwrap(), "external");
            }
            let meta = SourceMeta::open_project(&f.store.base, source.clone()).unwrap();
            assert_eq!(meta.source(), source);
            assert_eq!(meta.assets().count(), 0);
            assert!(meta.meta_path().exists());
        }
    }

    #[test]
    fn deimport_missing_artifact_and_corrupt_metadata() {
        let mut f = Fixture::new();
        f.write("one.txt", "one");
        let id = f.import("one.txt", Value::Unit).unwrap();
        let mut meta = SourceMeta::open(f.store.base.join("one.txt")).unwrap();
        let mut second = f.meta("one.txt");
        second.target = arcana::ident!(second);
        second.id = AssetId::new(std::num::NonZeroU64::new(123).unwrap());
        meta.add_asset(second).unwrap();
        f.store.refresh().unwrap();
        let path = f.meta_path("one.txt");
        let accepted = std::fs::read(&path).unwrap();
        std::fs::write(&path, "corrupt").unwrap();
        let revision = f.store.registry.revision();
        assert!(f.store.deimport(Path::new("one.txt")).is_err());
        assert_eq!(f.store.registry.revision(), revision);
        assert!(futures::executor::block_on(f.store.registry.load(id)).is_ok());
        std::fs::write(&path, accepted).unwrap();
        // Drop the artifact bytes, retaining source metadata and target actions.
        for item in std::fs::read_dir(f.store.base.join("registry")).unwrap() {
            std::fs::remove_file(item.unwrap().path()).unwrap();
        }
        f.store.refresh().unwrap();
        let entry = f
            .store
            .database
            .entries()
            .iter()
            .find(|entry| entry.path == Path::new("one.txt"))
            .unwrap();
        assert_eq!(entry.imported.len(), 2);
        assert!(entry.imported.iter().any(|asset| asset.id == id));
        assert!(entry.warning.is_some());
        f.store.deimport(Path::new("one.txt")).unwrap();
        assert!(f.store.database.warnings().is_empty());
        f.store = AssetStore::new(&f.project).unwrap();
        assert!(
            f.store
                .database
                .entries()
                .iter()
                .find(|entry| entry.path == Path::new("one.txt"))
                .unwrap()
                .imported
                .is_empty()
        );
    }

    #[cfg(windows)]
    #[test]
    fn deimport_failed_metadata_replacement_keeps_targets_and_registry() {
        use std::os::windows::fs::OpenOptionsExt;
        let mut f = Fixture::new();
        f.write("one.txt", "one");
        let id = f.import("one.txt", Value::Unit).unwrap();
        f.store.refresh().unwrap();
        let path = f.meta_path("one.txt");
        let accepted = std::fs::read(&path).unwrap();
        let revision = f.store.registry.revision();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let mut meta = SourceMeta::open(f.store.base.join("one.txt")).unwrap();
        assert!(meta.clear_assets().is_err());
        assert_eq!(meta.assets().next().unwrap().id, id);
        assert!(f.store.deimport(Path::new("one.txt")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), accepted);
        assert_eq!(f.store.registry.revision(), revision);
        assert!(futures::executor::block_on(f.store.registry.load(id)).is_ok());
        assert_eq!(
            f.store
                .database
                .entries()
                .iter()
                .find(|entry| entry.path == Path::new("one.txt"))
                .unwrap()
                .imported[0]
                .id,
            id
        );
        drop(lock);
    }

    #[test]
    fn stable_id_skip_reimport_and_reopen() {
        let mut f = Fixture::new();
        f.write("one.txt", "one");
        let id = f.import("one.txt", Value::Unit).unwrap();
        let imported_revision = f.store.registry.revision();
        assert_eq!(f.import("one.txt", Value::Unit).unwrap(), id);
        assert_eq!(f.store.registry.revision(), imported_revision);
        assert_eq!(f.calls.load(Ordering::Relaxed), 1);
        f.write("one.txt", "changed");
        assert_eq!(f.import("one.txt", Value::Unit).unwrap(), id);
        assert_eq!(f.meta("one.txt").version, 2);
        assert!(f.store.registry.revision() > imported_revision);
        let source_revision = f.store.registry.revision();
        assert_eq!(f.import("one.txt", Value::Int(7)).unwrap(), id);
        assert_eq!(f.meta("one.txt").version, 3);
        assert!(f.store.registry.revision() > source_revision);
        f.store = AssetStore::new(&f.project).unwrap();
        f.register();
        let data = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        assert_eq!(data.version, 3);
        assert!(String::from_utf8_lossy(&data.bytes).contains("changed"));
        assert_eq!(f.import("one.txt", Value::Int(7)).unwrap(), id);
        assert_eq!(f.calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn builtin_image_import_persists_and_reimports_without_plugins() {
        use arcana::assets::{Asset, Assets};
        use arcana::graphics::{ImageImporter, texture::Texture};
        let mut f = Fixture::new();
        let importer = ImageImporter::id();
        arcana::assets::register_builtin_importers(&mut f.hub);
        let target = arcana::ident!(Texture);
        assert_eq!(
            f.store.find_importers(target, None, Some("png"))[0].0,
            importer
        );
        f.store.update_plugins(&Plugins::default());
        assert_eq!(
            f.store.find_importers(target, None, Some("png"))[0].0,
            importer
        );
        let source = f.store.base.join("color.png");
        image::save_buffer(
            &source,
            &[12, 34, 56, 0, 90, 80, 70, 128],
            2,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        let id = f
            .store
            .store(&f.hub, "color.png", importer, Value::Unit)
            .unwrap();
        let first = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        assert_eq!(first.version, 1);
        assert!(
            futures::executor::block_on(Texture::load(first.bytes.clone(), &Assets::new([])))
                .is_ok()
        );
        let metadata = SourceMeta::open(source.clone()).unwrap();
        let meta = metadata.get_asset(target).unwrap();
        assert_eq!(meta.importer, Some(importer));
        assert_eq!(meta.config, Value::Unit);
        f.store = AssetStore::new(&f.project).unwrap();
        assert_eq!(
            f.store
                .store(&f.hub, "color.png", importer, Value::Unit)
                .unwrap(),
            id
        );
        assert_eq!(
            futures::executor::block_on(f.store.registry.load(id))
                .unwrap()
                .version,
            1
        );
        image::save_buffer(
            &source,
            &[255, 0, 0, 255, 0, 255, 0, 64],
            2,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::now() + std::time::Duration::from_secs(2)),
            )
            .unwrap();
        assert_eq!(
            f.store
                .store(&f.hub, "color.png", importer, Value::Unit)
                .unwrap(),
            id
        );
        let second = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        assert_eq!(second.version, 2);
        assert_ne!(second.bytes, first.bytes);
        let mut config = arcana::graphics::ImageImportConfig::default();
        config.format = arcana::graphics::ImageTargetFormat::Rgb8;
        config.swizzle.red = arcana::graphics::Channel::B;
        let config = arcana::model::serialize_to_value(&config).unwrap();
        assert_eq!(
            f.store
                .store(&f.hub, "color.png", importer, config.clone())
                .unwrap(),
            id
        );
        let third = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        assert_eq!(third.version, 3);
        assert_ne!(third.bytes, second.bytes);
        assert_eq!(
            SourceMeta::open(source.clone())
                .unwrap()
                .get_asset(target)
                .unwrap()
                .config,
            config
        );
        f.store = AssetStore::new(&f.project).unwrap();
        assert_eq!(
            f.store
                .store(&f.hub, "color.png", importer, config)
                .unwrap(),
            id
        );
        assert!(
            f.store
                .store(&f.hub, "color.png", importer, Value::Bool(false))
                .is_err()
        );
        assert_eq!(
            futures::executor::block_on(f.store.registry.load(id))
                .unwrap()
                .version,
            3
        );
        assert_eq!(
            futures::executor::block_on(f.store.registry.load(id))
                .unwrap()
                .bytes,
            third.bytes
        );
    }

    #[test]
    fn extra_source_retry_and_stamps() {
        let mut f = Fixture::new();
        f.write("one.txt", "extra");
        f.write("extra.txt", "first");
        let id = f.import("one.txt", Value::Unit).unwrap();
        assert_eq!(f.calls.load(Ordering::Relaxed), 2);
        assert_eq!(f.meta("one.txt").sources.len(), 2);
        f.write("extra.txt", "second");
        f.import("one.txt", Value::Unit).unwrap();
        assert_eq!(f.meta("one.txt").version, 2);
        assert!(
            String::from_utf8_lossy(
                &futures::executor::block_on(f.store.registry.load(id))
                    .unwrap()
                    .bytes
            )
            .contains("second")
        );
        std::fs::remove_file(f.store.base.join("extra.txt")).unwrap();
        assert!(f.import("one.txt", Value::Unit).is_err());
        assert_eq!(f.meta("one.txt").version, 2);
    }

    #[test]
    fn dependency_relative_to_source_and_changes_propagate() {
        let mut f = Fixture::new();
        f.write("nested/parent.txt", "dep:child.txt");
        f.write("nested/child.txt", "child");
        let id = f.import("nested/parent.txt", Value::Unit).unwrap();
        let child = f.meta("nested/child.txt");
        let parent = f.meta("nested/parent.txt");
        assert_eq!(parent.dependencies, vec![child.id]);
        assert_eq!(parent.sources.len(), 2);
        f.write("nested/child.txt", "changed");
        assert_eq!(f.import("nested/parent.txt", Value::Unit).unwrap(), id);
        assert_eq!(f.meta("nested/parent.txt").version, 2);
        assert_eq!(f.meta("nested/child.txt").version, 2);
        assert_eq!(f.meta("nested/child.txt").id, child.id);
    }

    #[test]
    fn cycles_and_requires_attempts_are_bounded() {
        let mut f = Fixture::new();
        f.write("a.txt", "dep:b.txt");
        f.write("b.txt", "dep:a.txt");
        assert!(f.import("a.txt", Value::Unit).is_err());
        assert_eq!(f.calls.load(Ordering::Relaxed), 2);
        f.write("retry.txt", "retry");
        let before = f.calls.load(Ordering::Relaxed);
        assert!(f.import("retry.txt", Value::Unit).is_err());
        assert_eq!(f.calls.load(Ordering::Relaxed) - before, MAX_ITEM_ATTEMPTS);
        assert_eq!(std::fs::read_dir(&f.store.temporaries).unwrap().count(), 0);
    }

    #[test]
    fn failed_import_keeps_metadata_and_loader_revision() {
        let mut f = Fixture::new();
        f.write("one.txt", "one");
        let id = f.import("one.txt", Value::Unit).unwrap();
        let before = std::fs::read(f.meta_path("one.txt")).unwrap();
        assert!(f.import("one.txt", Value::Bool(true)).is_err());
        assert_eq!(std::fs::read(f.meta_path("one.txt")).unwrap(), before);
        assert_eq!(
            futures::executor::block_on(f.store.registry.load(id))
                .unwrap()
                .version,
            1
        );
        assert_eq!(std::fs::read_dir(&f.store.temporaries).unwrap().count(), 0);
    }

    impl Fixture {
        fn meta_path(&self, name: &str) -> PathBuf {
            SourceMeta::open(self.store.base.join(name))
                .unwrap()
                .meta_path()
                .to_owned()
        }
    }

    #[test]
    fn reserved_sources_rejected_and_external_sources_allowed() {
        let mut f = Fixture::new();
        f.write("tmp/one.txt", "one");
        assert!(f.import("tmp/one.txt", Value::Unit).is_err());
        std::fs::write(f.root.join("outside.txt"), "one").unwrap();
        assert!(f.import("../outside.txt", Value::Unit).is_ok());
        assert!(!f.root.join("outside.txt.arc").exists());
    }

    #[test]
    fn external_sources_keep_distinct_metadata_ids_tags_and_missing_artifacts() {
        let mut f = Fixture::new();
        for folder in ["external-a", "external-b"] {
            std::fs::create_dir(f.root.join(folder)).unwrap();
            std::fs::write(f.root.join(folder).join("same.txt"), folder).unwrap();
        }
        let a = std::fs::canonicalize(f.root.join("external-a/same.txt")).unwrap();
        let b = std::fs::canonicalize(f.root.join("external-b/same.txt")).unwrap();
        let id_a = f.import(a.to_str().unwrap(), Value::Int(7)).unwrap();
        let id_b = f.import(b.to_str().unwrap(), Value::Unit).unwrap();
        assert_ne!(id_a, id_b);
        let meta_a = SourceMeta::open_project(&f.store.base, a.clone()).unwrap();
        let meta_b = SourceMeta::open_project(&f.store.base, b.clone()).unwrap();
        assert_ne!(meta_a.meta_path(), meta_b.meta_path());
        assert_eq!(
            meta_a.meta_path().parent().unwrap(),
            f.store.base.join(".external")
        );
        assert_eq!(meta_a.source(), a);
        assert_eq!(meta_a.meta_path().file_stem().unwrap().len(), 64);
        f.store.refresh().unwrap();
        f.store
            .database
            .set_tags(&a, vec!["Outside".into()])
            .unwrap();
        f.store = AssetStore::new(&f.project).unwrap();
        f.register();
        let results = f
            .store
            .database
            .search(Path::new(""), "same", false, false, &[]);
        assert_eq!(results.len(), 2);
        assert_eq!(
            f.store
                .database
                .search(Path::new(""), "same", false, true, &["outside".into()])
                .len(),
            1
        );
        assert_eq!(
            SourceMeta::open_project(&f.store.base, a.clone())
                .unwrap()
                .get_asset(TestImporter::desc().target)
                .unwrap()
                .config,
            Value::Int(7)
        );
        assert_eq!(f.import(a.to_str().unwrap(), Value::Int(7)).unwrap(), id_a);
        let calls = f.calls.load(Ordering::Relaxed);
        std::fs::remove_file(&a).unwrap();
        f.store = AssetStore::new(&f.project).unwrap();
        f.register();
        let entry = f
            .store
            .database
            .entries()
            .iter()
            .find(|entry| entry.path == a)
            .unwrap();
        assert!(entry.warning.as_ref().unwrap().contains("missing"));
        assert_eq!(entry.imported[0].id, id_a);
        assert_eq!(entry.tags, ["outside"]);
        assert!(futures::executor::block_on(f.store.registry.load(id_a)).is_ok());
        f.store
            .database
            .set_tags(&a, vec!["Missing".into()])
            .unwrap();
        assert!(f.import(a.to_str().unwrap(), Value::Int(7)).is_err());
        assert_eq!(f.calls.load(Ordering::Relaxed), calls);
        assert_eq!(
            std::fs::read_dir(f.root.join("external-a"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_dir(f.root.join("external-b"))
                .unwrap()
                .count(),
            1
        );
        assert!(!f.store.base.join("same.txt").exists());
    }

    #[test]
    fn external_dependencies_and_additional_sources_use_external_parent() {
        let mut f = Fixture::new();
        let folder = f.root.join("external");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("parent.txt"), "dep:child.txt").unwrap();
        std::fs::write(folder.join("child.txt"), "extra").unwrap();
        std::fs::write(folder.join("extra.txt"), "additional").unwrap();
        let parent = std::fs::canonicalize(folder.join("parent.txt")).unwrap();
        let child = std::fs::canonicalize(folder.join("child.txt")).unwrap();
        let id = f.import(parent.to_str().unwrap(), Value::Unit).unwrap();
        let parent_meta = SourceMeta::open_project(&f.store.base, parent.clone()).unwrap();
        let parent_asset = parent_meta.get_asset(TestImporter::desc().target).unwrap();
        let child_meta = SourceMeta::open_project(&f.store.base, child).unwrap();
        let child_asset = child_meta.get_asset(TestImporter::desc().target).unwrap();
        assert_eq!(parent_asset.dependencies, [child_asset.id]);
        assert_eq!(child_asset.sources.len(), 2);
        assert_eq!(parent_asset.sources.len(), 3);
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 3);
        assert_eq!(
            std::fs::read_dir(f.store.base.join(".external"))
                .unwrap()
                .count(),
            2
        );
        assert_eq!(f.import(parent.to_str().unwrap(), Value::Unit).unwrap(), id);
        assert!(!f.store.base.join("parent.txt").exists());
    }

    #[test]
    fn duplicate_reimports_reuse_blob_and_increment_revision() {
        let mut f = Fixture::new();
        f.write("one.txt", "same");
        let id = f.import("one.txt", Value::Unit).unwrap();
        let blob = f.meta("one.txt").blob;
        f.write("one.txt", "same");
        assert_eq!(f.import("one.txt", Value::Unit).unwrap(), id);
        assert_eq!(f.meta("one.txt").blob, blob);
        assert_eq!(f.meta("one.txt").version, 2);
        assert_eq!(
            std::fs::read_dir(f.store.base.join("registry"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn dependency_config_change_invalidates_parent_and_preserves_child_config() {
        let mut f = Fixture::new();
        f.write("parent.txt", "dep:child.txt");
        f.write("child.txt", "child");
        f.import("parent.txt", Value::Unit).unwrap();
        f.import("child.txt", Value::Int(7)).unwrap();
        f.import("parent.txt", Value::Unit).unwrap();
        assert_eq!(f.meta("parent.txt").version, 2);
        assert_eq!(f.meta("child.txt").config, Value::Int(7));
        assert_eq!(f.meta("child.txt").version, 2);
    }

    #[test]
    fn transitive_dependency_config_changes_refresh_intermediate_assets() {
        let mut f = Fixture::new();
        f.write("grandparent.txt", "dep:parent.txt");
        f.write("parent.txt", "dep:child.txt");
        f.write("child.txt", "child");
        f.import("grandparent.txt", Value::Unit).unwrap();
        f.import("child.txt", Value::Int(7)).unwrap();
        f.import("grandparent.txt", Value::Unit).unwrap();
        assert_eq!(f.meta("grandparent.txt").version, 2);
        assert_eq!(f.meta("parent.txt").version, 2);
        assert_eq!(f.meta("child.txt").version, 2);
        assert_eq!(f.meta("child.txt").config, Value::Int(7));
    }

    #[test]
    fn legacy_metadata_reads_and_tags_upgrade_without_losing_assets() {
        let mut f = Fixture::new();
        f.write("one.txt", "one");
        let id = f.import("one.txt", Value::Unit).unwrap();
        let path = f.meta_path("one.txt");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let mut assets = json["assets"].take();
        assets[0].as_object_mut().unwrap().remove("version");
        assets[0].as_object_mut().unwrap().remove("importer");
        std::fs::write(&path, serde_json::to_vec(&assets).unwrap()).unwrap();
        let mut meta = SourceMeta::open(f.store.base.join("one.txt")).unwrap();
        assert_eq!(meta.assets().next().unwrap().id, id);
        assert_eq!(meta.assets().next().unwrap().version, 0);
        meta.set_tags(vec![
            " Hero ".into(),
            "hero".into(),
            " SKY".into(),
            "".into(),
        ])
        .unwrap();
        let meta = SourceMeta::open(f.store.base.join("one.txt")).unwrap();
        assert_eq!(meta.tags(), ["hero", "sky"]);
        assert_eq!(meta.assets().next().unwrap().id, id);
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(value.is_object());
        assert!(value["assets"].is_array());
    }

    #[cfg(windows)]
    #[test]
    fn failed_sidecar_replacement_preserves_disk_and_registry() {
        use std::os::windows::fs::OpenOptionsExt;
        let mut f = Fixture::new();
        f.write("one.txt", "one");
        let id = f.import("one.txt", Value::Unit).unwrap();
        let path = f.meta_path("one.txt");
        let before = std::fs::read(&path).unwrap();
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(f.import("one.txt", Value::Int(7)).is_err());
        drop(locked);
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert_eq!(
            futures::executor::block_on(f.store.registry.load(id))
                .unwrap()
                .version,
            1
        );
    }
    #[test]
    fn map_import_resolves_tileset_and_texture_dependencies_and_preserves_publication_on_failure() {
        use arcana::assets::{Asset, Assets};
        use arcana::graphics::{ImageImporter, texture::Texture};

        let mut f = Fixture::new();
        let plugin = tilemap::arcana_plugin::get();
        plugin.init(&mut arcana::ecs::world::World::new(), &mut f.hub);
        arcana::assets::register_builtin_importers(&mut f.hub);
        for info in plugin.importers() {
            f.store.importers.insert(info.id, info.desc.clone());
        }
        let importer = plugin
            .importers()
            .iter()
            .find(|info| info.desc.target == arcana::ident!(Map))
            .unwrap()
            .id;
        let tileset_importer = plugin
            .importers()
            .iter()
            .find(|info| info.desc.target == arcana::ident!(Tileset))
            .unwrap()
            .id;
        let folder = f.store.base.join("nested/tilemap");
        std::fs::create_dir_all(&folder).unwrap();
        for (name, bytes) in [
            (
                "map.json",
                include_bytes!("../../../assets/tilemap/map.json").as_slice(),
            ),
            (
                "tileset.json",
                include_bytes!("../../../assets/tilemap/tileset.json").as_slice(),
            ),
            (
                "atlas.png",
                include_bytes!("../../../assets/tilemap/atlas.png").as_slice(),
            ),
        ] {
            std::fs::write(folder.join(name), bytes).unwrap();
        }
        let source = "nested/tilemap/map.json";
        let id = f
            .store
            .store(&f.hub, source, importer, Value::Unit)
            .unwrap();
        let map_meta = SourceMeta::open(folder.join("map.json")).unwrap();
        let tileset_meta = SourceMeta::open(folder.join("tileset.json")).unwrap();
        let texture_meta = SourceMeta::open(folder.join("atlas.png")).unwrap();
        let map = map_meta.get_asset(arcana::ident!(Map)).unwrap();
        let tileset = tileset_meta.get_asset(arcana::ident!(Tileset)).unwrap();
        let texture = texture_meta.get_asset(arcana::ident!(Texture)).unwrap();
        let tileset_id = tileset.id;
        let texture_id = texture.id;
        assert_eq!(map.id, id);
        assert_eq!(map.importer, Some(importer));
        assert_eq!(tileset.importer, Some(tileset_importer));
        assert_eq!(texture.importer, Some(ImageImporter::id()));
        assert_eq!(map.dependencies, [tileset_id]);
        assert_eq!(tileset.dependencies, [texture_id]);
        assert!(texture.dependencies.is_empty());
        let first = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        let map_data: tilemap::MapData = serde_json::from_slice(&first.bytes).unwrap();
        assert_eq!(map_data.tileset, tileset_id);
        let sample: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../assets/tilemap/map.json")).unwrap();
        assert_eq!(
            serde_json::to_value(map_data.cells).unwrap(),
            sample["cells"]
        );
        let tileset_first = futures::executor::block_on(f.store.registry.load(tileset_id)).unwrap();
        let tileset_data: tilemap::TilesetData =
            serde_json::from_slice(&tileset_first.bytes).unwrap();
        assert_eq!(tileset_data.image, texture_id);
        let texture_first = futures::executor::block_on(f.store.registry.load(texture_id)).unwrap();
        assert!(
            futures::executor::block_on(Texture::load(
                texture_first.bytes.clone(),
                &Assets::new([])
            ))
            .is_ok()
        );

        let image_source = folder.join("atlas.png");
        image::save_buffer(
            &image_source,
            &vec![255; 32 * 16 * 4],
            32,
            16,
            image::ColorType::Rgba8,
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&image_source)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::now() + std::time::Duration::from_secs(2)),
            )
            .unwrap();
        assert_eq!(
            f.store
                .store(&f.hub, source, importer, Value::Unit)
                .unwrap(),
            id
        );
        let last = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        let tileset_last = futures::executor::block_on(f.store.registry.load(tileset_id)).unwrap();
        let texture_last = futures::executor::block_on(f.store.registry.load(texture_id)).unwrap();
        assert!(last.version > first.version);
        assert!(tileset_last.version > tileset_first.version);
        assert!(texture_last.version > texture_first.version);
        assert_ne!(texture_last.bytes, texture_first.bytes);
        assert_eq!(
            serde_json::from_slice::<tilemap::MapData>(&last.bytes)
                .unwrap()
                .tileset,
            tileset_id
        );
        assert_eq!(
            serde_json::from_slice::<tilemap::TilesetData>(&tileset_last.bytes)
                .unwrap()
                .image,
            texture_id
        );
        assert!(
            futures::executor::block_on(Texture::load(
                texture_last.bytes.clone(),
                &Assets::new([])
            ))
            .is_ok()
        );
        let mut metadata = Vec::new();
        for (name, target, expected_id, dependencies) in [
            ("map.json", arcana::ident!(Map), id, vec![tileset_id]),
            (
                "tileset.json",
                arcana::ident!(Tileset),
                tileset_id,
                vec![texture_id],
            ),
            ("atlas.png", arcana::ident!(Texture), texture_id, vec![]),
        ] {
            let meta = SourceMeta::open(folder.join(name)).unwrap();
            let asset = meta.get_asset(target).unwrap();
            assert_eq!(asset.id, expected_id);
            assert_eq!(asset.dependencies, dependencies);
            let path = meta.meta_path().to_owned();
            metadata.push((path.clone(), std::fs::read(path).unwrap()));
        }
        std::fs::write(&image_source, b"invalid PNG").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&image_source)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::now() + std::time::Duration::from_secs(4)),
            )
            .unwrap();
        let revision = f.store.registry.revision();
        assert!(
            f.store
                .store(&f.hub, source, importer, Value::Unit)
                .is_err()
        );
        assert_eq!(f.store.registry.revision(), revision);
        for (path, before) in metadata {
            assert_eq!(std::fs::read(path).unwrap(), before);
        }
        for (asset_id, expected) in [
            (id, last),
            (tileset_id, tileset_last),
            (texture_id, texture_last),
        ] {
            let published = futures::executor::block_on(f.store.registry.load(asset_id)).unwrap();
            assert_eq!(published.version, expected.version);
            assert_eq!(published.bytes, expected.bytes);
        }
    }

    #[test]
    fn tileset_import_resolves_texture_dependency_and_preserves_publication_on_failure() {
        use arcana::assets::{Asset, Assets};
        use arcana::graphics::{ImageImporter, texture::Texture};

        let mut f = Fixture::new();
        let plugin = tilemap::arcana_plugin::get();
        plugin.init(&mut arcana::ecs::world::World::new(), &mut f.hub);
        arcana::assets::register_builtin_importers(&mut f.hub);
        let info = plugin
            .importers()
            .iter()
            .find(|info| info.desc.target == arcana::ident!(Tileset))
            .unwrap();
        f.store.importers.insert(info.id, info.desc.clone());
        let folder = f.store.base.join("nested/tilemap");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("tileset.json"),
            include_bytes!("../../../assets/tilemap/tileset.json"),
        )
        .unwrap();
        let image_source = folder.join("atlas.png");
        std::fs::write(
            &image_source,
            include_bytes!("../../../assets/tilemap/atlas.png"),
        )
        .unwrap();
        let id = f
            .store
            .store(&f.hub, "nested/tilemap/tileset.json", info.id, Value::Unit)
            .unwrap();
        let metadata = SourceMeta::open(folder.join("tileset.json")).unwrap();
        let parent = metadata.get_asset(arcana::ident!(Tileset)).unwrap();
        let image_metadata = SourceMeta::open(image_source.clone()).unwrap();
        let texture = image_metadata.get_asset(arcana::ident!(Texture)).unwrap();
        assert_eq!(texture.importer, Some(ImageImporter::id()));
        assert_eq!(parent.dependencies, [texture.id]);
        let texture_id = texture.id;
        let first = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        let artifact: tilemap::TilesetData = serde_json::from_slice(&first.bytes).unwrap();
        assert_eq!(artifact.image, texture_id);
        assert_eq!(artifact.tiles.len(), 2);
        let image_first = futures::executor::block_on(f.store.registry.load(texture_id)).unwrap();
        assert!(
            futures::executor::block_on(Texture::load(image_first.bytes.clone(), &Assets::new([])))
                .is_ok()
        );

        // A dependency source change is imported through the parent's next import.
        image::save_buffer(
            &image_source,
            &vec![255; 32 * 16 * 4],
            32,
            16,
            image::ColorType::Rgba8,
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&image_source)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::now() + std::time::Duration::from_secs(2)),
            )
            .unwrap();
        assert_eq!(
            f.store
                .store(&f.hub, "nested/tilemap/tileset.json", info.id, Value::Unit)
                .unwrap(),
            id
        );
        let image_second = futures::executor::block_on(f.store.registry.load(texture_id)).unwrap();
        assert_eq!(image_second.version, image_first.version + 1);
        assert_ne!(image_second.bytes, image_first.bytes);
        assert!(
            futures::executor::block_on(Texture::load(
                image_second.bytes.clone(),
                &Assets::new([])
            ))
            .is_ok()
        );

        let config = arcana::graphics::ImageImportConfig {
            format: arcana::graphics::ImageTargetFormat::Rgb8,
            ..Default::default()
        };
        let config = arcana::model::serialize_to_value(&config).unwrap();
        assert_eq!(
            f.store
                .store(
                    &f.hub,
                    "nested/tilemap/atlas.png",
                    ImageImporter::id(),
                    config.clone()
                )
                .unwrap(),
            texture_id
        );
        assert_eq!(
            f.store
                .store(&f.hub, "nested/tilemap/tileset.json", info.id, Value::Unit)
                .unwrap(),
            id
        );
        let configured = futures::executor::block_on(f.store.registry.load(texture_id)).unwrap();
        assert_eq!(configured.version, image_second.version + 1);
        assert!(
            futures::executor::block_on(Texture::load(configured.bytes, &Assets::new([]))).is_ok()
        );
        let image_metadata = SourceMeta::open(image_source.clone()).unwrap();
        assert_eq!(
            image_metadata
                .get_asset(arcana::ident!(Texture))
                .unwrap()
                .config,
            config
        );
        let metadata = SourceMeta::open(folder.join("tileset.json")).unwrap();
        assert_eq!(
            metadata
                .get_asset(arcana::ident!(Tileset))
                .unwrap()
                .dependencies,
            [texture_id]
        );
        let last = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        assert!(last.version > first.version);
        let artifact: tilemap::TilesetData = serde_json::from_slice(&last.bytes).unwrap();
        assert_eq!(artifact.image, texture_id);

        // A failed dependency import leaves both published assets available.
        let parent_meta_path = metadata.meta_path().to_owned();
        let parent_before = std::fs::read(&parent_meta_path).unwrap();
        let texture_meta_path = image_metadata.meta_path().to_owned();
        let texture_before = std::fs::read(&texture_meta_path).unwrap();
        std::fs::write(&image_source, b"invalid PNG").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&image_source)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::now() + std::time::Duration::from_secs(4)),
            )
            .unwrap();
        let revision = f.store.registry.revision();
        assert!(
            f.store
                .store(&f.hub, "nested/tilemap/tileset.json", info.id, Value::Unit)
                .is_err()
        );
        assert_eq!(f.store.registry.revision(), revision);
        assert_eq!(std::fs::read(parent_meta_path).unwrap(), parent_before);
        assert_eq!(std::fs::read(texture_meta_path).unwrap(), texture_before);
        let published = futures::executor::block_on(f.store.registry.load(id)).unwrap();
        assert_eq!(published.version, last.version);
        assert_eq!(published.bytes, last.bytes);
        assert!(futures::executor::block_on(f.store.registry.load(texture_id)).is_ok());
    }
}
