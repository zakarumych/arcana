//! Cached project and external source catalog. Filesystem work runs only on refresh or explicit edits.
use super::{AssetRegistry, SourceMeta};
use arcana::{
    Ident,
    assets::AssetId,
    error::{Error, fail},
};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub struct ImportedAsset {
    pub target: Ident,
    pub id: AssetId,
    pub version: u64,
}

pub struct AssetEntry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub tags: Vec<String>,
    pub imported: Vec<ImportedAsset>,
    pub warning: Option<String>,
    pub metadata_error: bool,
    normalized_name: String,
}

pub struct AssetDatabase {
    base: PathBuf,
    entries: Vec<AssetEntry>,
    warnings: Vec<String>,
    revision: u64,
}

impl AssetDatabase {
    pub fn new(base: &Path, registry: &AssetRegistry) -> Result<Self, Error> {
        fs::create_dir_all(base).map_err(Error::wrap)?;
        let mut database = Self {
            base: fs::canonicalize(base).map_err(Error::wrap)?,
            entries: Vec::new(),
            warnings: Vec::new(),
            revision: 0,
        };
        database.refresh(registry)?;
        Ok(database)
    }

    pub fn entries(&self) -> &[AssetEntry] {
        &self.entries
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn refresh(&mut self, registry: &AssetRegistry) -> Result<(), Error> {
        let mut entries = Vec::new();
        let mut warnings = Vec::new();
        let mut mappings = Vec::new();
        let mut folders = vec![PathBuf::new()];
        while let Some(folder) = folders.pop() {
            let absolute = self.validate_folder(&folder)?;
            for item in fs::read_dir(absolute).map_err(Error::wrap)? {
                let item = item.map_err(Error::wrap)?;
                let path = folder.join(item.file_name());
                if reserved(&path) {
                    continue;
                }
                let metadata = fs::symlink_metadata(item.path()).map_err(Error::wrap)?;
                // Directory links can create cycles; junctions and symlinks are never catalogued.
                if is_link(&metadata) {
                    continue;
                }
                let canonical = fs::canonicalize(item.path()).map_err(Error::wrap)?;
                if !canonical.starts_with(&self.base) {
                    continue;
                }
                let directory = metadata.is_dir();
                if !directory && !metadata.is_file() {
                    continue;
                }
                let name = item.file_name().to_string_lossy().into_owned();
                let mut entry = AssetEntry {
                    path: path.clone(),
                    normalized_name: name.to_lowercase(),
                    name,
                    directory,
                    tags: Vec::new(),
                    imported: Vec::new(),
                    warning: None,
                    metadata_error: false,
                };
                if directory {
                    folders.push(path);
                } else {
                    match self.open_meta(&canonical) {
                        Ok(meta) => {
                            entry.tags = meta.tags().to_vec();
                            for asset in meta.assets() {
                                let version = asset.registry_version();
                                entry.imported.push(ImportedAsset {
                                    target: asset.target,
                                    id: asset.id,
                                    version,
                                });
                                if registry.contains_blob(asset.blob) {
                                    mappings.push((asset.id, asset.blob, version));
                                } else {
                                    let warning = format!(
                                        "{}: imported artifact {} is missing",
                                        entry.path.display(),
                                        asset.id
                                    );
                                    entry.warning = Some(warning.clone());
                                    warnings.push(warning);
                                }
                            }
                            entry.imported.sort_by_key(|asset| asset.target.to_string());
                        }
                        Err(error) => {
                            entry.metadata_error = true;
                            let warning = format!("{}: {}", entry.path.display(), error);
                            entry.warning = Some(warning.clone());
                            warnings.push(warning);
                        }
                    }
                }
                entries.push(entry);
            }
        }
        let external = SourceMeta::external_directory(&self.base).map_err(Error::wrap)?;
        if external.exists() {
            for item in fs::read_dir(&external).map_err(Error::wrap)? {
                let item = item.map_err(Error::wrap)?;
                if item.path().extension() != Some(std::ffi::OsStr::new("arc")) {
                    continue;
                }
                match SourceMeta::open_project_meta(&self.base, item.path()) {
                    Ok(meta) => {
                        let source = meta.source().to_owned();
                        let name = source
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        let mut entry = AssetEntry {
                            path: source.clone(),
                            normalized_name: name.to_lowercase(),
                            name,
                            directory: false,
                            tags: meta.tags().to_vec(),
                            imported: Vec::new(),
                            warning: None,
                            metadata_error: false,
                        };
                        if !source.is_file() {
                            entry.warning =
                                Some(format!("External source is missing: {}", source.display()));
                        }
                        for asset in meta.assets() {
                            let version = asset.registry_version();
                            entry.imported.push(ImportedAsset {
                                target: asset.target,
                                id: asset.id,
                                version,
                            });
                            if registry.contains_blob(asset.blob) {
                                mappings.push((asset.id, asset.blob, version));
                            } else {
                                entry.warning = Some(format!(
                                    "{}: imported artifact {} is missing",
                                    source.display(),
                                    asset.id
                                ));
                            }
                        }
                        entry.imported.sort_by_key(|asset| asset.target.to_string());
                        if let Some(warning) = &entry.warning {
                            warnings.push(warning.clone());
                        }
                        entries.push(entry);
                    }
                    Err(error) => warnings.push(format!("{}: {}", item.path().display(), error)),
                }
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        registry.restore(mappings);
        self.entries = entries;
        self.warnings = warnings;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub fn search(
        &self,
        folder: &Path,
        query: &str,
        fuzzy: bool,
        recursive: bool,
        tags: &[String],
    ) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        let tags: Vec<_> = tags
            .iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        let mut matches = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.path.is_absolute() {
                if !folder.as_os_str().is_empty() {
                    continue;
                }
            } else if entry.path == folder || !entry.path.starts_with(folder) {
                continue;
            }
            if !entry.path.is_absolute() && !recursive && entry.path.parent() != Some(folder) {
                continue;
            }
            if !tags
                .iter()
                .all(|tag| entry.tags.iter().any(|value| value.to_lowercase() == *tag))
            {
                continue;
            }
            if let Some(score) = match_score(&entry.normalized_name, &query, fuzzy) {
                matches.push((
                    score,
                    !entry.directory,
                    &entry.normalized_name,
                    &entry.path,
                    index,
                ));
            }
        }
        matches.sort();
        matches
            .into_iter()
            .map(|(_, _, _, _, index)| index)
            .collect()
    }

    pub fn validate_folder(&self, folder: &Path) -> Result<PathBuf, Error> {
        let path = self.resolve_source(folder)?;
        if !path.is_dir() {
            fail!("Asset folder does not exist: {}", folder.display());
        }
        Ok(path)
    }

    pub fn resolve_source(&self, relative: &Path) -> Result<PathBuf, Error> {
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
            || reserved(relative)
        {
            fail!("Invalid or reserved asset path: {}", relative.display());
        }
        let path = fs::canonicalize(self.base.join(relative)).map_err(Error::wrap)?;
        if !path.starts_with(&self.base) {
            fail!("Asset path leaves project asset directory");
        }
        if reserved(path.strip_prefix(&self.base).unwrap()) {
            fail!("Asset path resolves to reserved storage");
        }
        Ok(path)
    }

    /// Resolves an existing import source to a relative internal or absolute external identity.
    pub fn import_source(&self, source: &Path) -> Result<PathBuf, Error> {
        let source = source
            .to_str()
            .ok_or_else(|| Error::msg("Importer requires a UTF-8 source path".into()))?;
        let path = super::store::resolve_project_source(&self.base, &self.base, source)?;
        Ok(path.strip_prefix(&self.base).unwrap_or(&path).to_owned())
    }

    pub fn set_tags(&mut self, relative: &Path, tags: Vec<String>) -> Result<(), Error> {
        let source = if relative.is_absolute() {
            if !self
                .entries
                .iter()
                .any(|entry| entry.path == relative && !entry.directory)
            {
                fail!("External source is not in the asset catalog");
            }
            relative.to_owned()
        } else {
            self.resolve_source(relative)?
        };
        if !relative.is_absolute() && !source.is_file() {
            fail!("Tags belong to asset source files");
        }
        let mut meta = self.open_meta(&source)?;
        meta.set_tags(tags).map_err(Error::wrap)?;
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == relative) {
            entry.tags = meta.tags().to_vec();
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub(super) fn deimport(
        &mut self,
        identity: &Path,
        registry: &AssetRegistry,
    ) -> Result<(), Error> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.path == identity && !entry.directory)
            .ok_or_else(|| Error::msg("Source is not in the asset catalog".into()))?;
        let source = if identity.is_absolute() {
            identity.to_owned()
        } else {
            self.resolve_source(identity)?
        };
        let mut meta = self.open_meta(&source)?;
        let ids = meta.clear_assets().map_err(Error::wrap)?;
        registry.unpublish(&ids);
        for id in &ids {
            let warning = format!(
                "{}: imported artifact {} is missing",
                identity.display(),
                id
            );
            self.warnings.retain(|value| value != &warning);
        }
        let entry = &mut self.entries[index];
        entry.imported.clear();
        if let Some(warning) = entry.warning.take() {
            self.warnings.retain(|value| value != &warning);
        }
        if identity.is_absolute() && !source.is_file() {
            let warning = format!("External source is missing: {}", source.display());
            self.warnings.push(warning.clone());
            entry.warning = Some(warning);
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub(super) fn open_meta(&self, source: &Path) -> Result<SourceMeta, Error> {
        SourceMeta::open_project(&self.base, source.to_owned()).map_err(Error::wrap)
    }
}

pub(super) fn reserved(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("arc"))
        || path.components().next().is_some_and(|component| {
            ["tmp", "registry", "sources", ".external"]
                .iter()
                .any(|name| component.as_os_str().eq_ignore_ascii_case(name))
        })
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn match_score(name: &str, query: &str, fuzzy: bool) -> Option<(u8, usize, usize)> {
    if query.is_empty() {
        return Some((0, 0, 0));
    }
    if let Some(offset) = name.find(query) {
        return Some((0, offset, name.chars().count()));
    }
    if !fuzzy {
        return None;
    }
    let mut remaining = query.chars();
    let mut next = remaining.next()?;
    let mut first = None;
    for (index, ch) in name.chars().enumerate() {
        if ch != next {
            continue;
        }
        let start = *first.get_or_insert(index);
        match remaining.next() {
            Some(ch) => next = ch,
            None => return Some((1, index - start + 1 - query.chars().count(), start)),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "arcana-catalog-{}-{}",
                std::process::id(),
                rand::random::<u64>()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn database(&self) -> (AssetDatabase, AssetRegistry) {
            let registry = AssetRegistry::new(self.0.join("registry")).unwrap();
            (AssetDatabase::new(&self.0, &registry).unwrap(), registry)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn catalog_search_navigation_tags_and_refresh() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.0.join("textures")).unwrap();
        fs::create_dir_all(fixture.0.join("tmp")).unwrap();
        fs::write(fixture.0.join("tmp/hidden.png"), b"hidden").unwrap();
        fs::write(fixture.0.join("Brick.png"), b"source").unwrap();
        fs::write(fixture.0.join("textures/BrightRock.png"), b"source").unwrap();
        fs::write(fixture.0.join("textures/Éclair.png"), b"source").unwrap();
        let (mut db, registry) = fixture.database();
        let names = |db: &AssetDatabase, matches: Vec<usize>| {
            matches
                .into_iter()
                .map(|i| db.entries()[i].name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&db, db.search(Path::new(""), "br", false, false, &[])),
            ["Brick.png"]
        );
        assert_eq!(
            names(&db, db.search(Path::new(""), "brick", true, true, &[])),
            ["Brick.png", "BrightRock.png"]
        );
        assert_eq!(
            names(
                &db,
                db.search(Path::new("textures"), "ÉCL", false, false, &[])
            ),
            ["Éclair.png"]
        );
        assert!(
            !db.entries()
                .iter()
                .any(|entry| entry.path.starts_with("tmp")
                    || entry.path.starts_with("registry")
                    || entry.path.extension().is_some_and(|ext| ext == "arc"))
        );
        db.set_tags(
            Path::new("Brick.png"),
            vec![" Stone ".into(), "WALL".into()],
        )
        .unwrap();
        assert_eq!(
            names(
                &db,
                db.search(
                    Path::new(""),
                    "",
                    false,
                    true,
                    &["STONE".into(), "wall".into()]
                )
            ),
            ["Brick.png"]
        );
        assert!(
            db.search(
                Path::new(""),
                "",
                false,
                true,
                &["wall".into(), "missing".into()]
            )
            .is_empty()
        );
        db.refresh(&registry).unwrap();
        assert_eq!(
            names(
                &db,
                db.search(Path::new(""), "", false, true, &["stone".into()])
            ),
            ["Brick.png"]
        );
        db.set_tags(Path::new("Brick.png"), Vec::new()).unwrap();
        assert!(
            SourceMeta::open(fixture.0.join("Brick.png"))
                .unwrap()
                .tags()
                .is_empty()
        );
        fs::write(fixture.0.join("textures/Broken.png"), b"source").unwrap();
        fs::write(fixture.0.join("textures/Broken.png.arc"), b"invalid json").unwrap();
        db.refresh(&registry).unwrap();
        assert_eq!(db.warnings().len(), 1);
        assert!(
            db.entries()
                .iter()
                .find(|entry| entry.name == "Broken.png")
                .unwrap()
                .metadata_error
        );
    }

    #[test]
    fn import_sources_must_exist_without_copying() {
        let fixture = Fixture::new();
        let external = Fixture::new();
        fs::create_dir_all(fixture.0.join("textures")).unwrap();
        fs::create_dir_all(fixture.0.join("tmp")).unwrap();
        fs::write(fixture.0.join("textures/source.png"), b"project").unwrap();
        fs::write(external.0.join("source.png"), b"external").unwrap();
        let (db, _) = fixture.database();
        assert_eq!(
            db.import_source(&external.0.join("source.png")).unwrap(),
            fs::canonicalize(external.0.join("source.png")).unwrap()
        );
        assert!(!fixture.0.join("source.png").exists());
        assert_eq!(
            fs::read(external.0.join("source.png")).unwrap(),
            b"external"
        );
        assert_eq!(
            db.import_source(&fixture.0.join("textures/source.png"))
                .unwrap(),
            Path::new("textures/source.png")
        );
        assert_eq!(
            db.import_source(&fs::canonicalize(fixture.0.join("textures/source.png")).unwrap())
                .unwrap(),
            Path::new("textures/source.png")
        );
        assert!(db.import_source(&fixture.0.join("textures")).is_err());
        assert!(db.import_source(&fixture.0.join("missing.png")).is_err());
        assert_eq!(
            fs::read(fixture.0.join("textures/source.png")).unwrap(),
            b"project"
        );
        for path in [
            "../outside",
            "tmp",
            "registry",
            "sources",
            ".external",
            "source.png.arc",
        ] {
            assert!(db.resolve_source(Path::new(path)).is_err());
        }
        fs::write(fixture.0.join("tmp/reserved.png"), b"internal").unwrap();
        assert!(
            db.import_source(&fixture.0.join("tmp/reserved.png"))
                .is_err()
        );
        fs::write(fixture.0.join("source.png.arc"), b"metadata").unwrap();
        assert!(db.import_source(&fixture.0.join("source.png.arc")).is_err());
        assert!(
            db.validate_folder(Path::new("textures/source.png"))
                .is_err()
        );
        assert!(db.resolve_source(&external.0).is_err());
    }

    #[test]
    fn nested_external_named_folder_keeps_internal_sibling_metadata() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.0.join("textures/.external")).unwrap();
        let source = fixture.0.join("textures/.external/source.png");
        fs::write(&source, b"source").unwrap();
        let (mut db, registry) = fixture.database();
        db.set_tags(
            Path::new("textures/.external/source.png"),
            vec!["internal".into()],
        )
        .unwrap();
        db.refresh(&registry).unwrap();
        assert_eq!(
            db.entries()
                .iter()
                .find(|entry| entry.name == "source.png")
                .unwrap()
                .tags,
            ["internal"]
        );
        assert_eq!(SourceMeta::open(source.clone()).unwrap().source(), source);
        assert!(!fixture.0.join(".external").exists());
    }

    #[test]
    fn external_metadata_identity_and_linked_storage_are_rejected() {
        let fixture = Fixture::new();
        let external = Fixture::new();
        fs::write(external.0.join("source.png"), b"external").unwrap();
        let (mut db, registry) = fixture.database();
        let source = fs::canonicalize(external.0.join("source.png")).unwrap();
        let mut meta = SourceMeta::open_project(&db.base, source.clone()).unwrap();
        meta.set_tags(vec!["tag".into()]).unwrap();
        let path = meta.meta_path().to_owned();
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        json["source"] = serde_json::json!(external.0.join("other.png"));
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        db.refresh(&registry).unwrap();
        assert!(db.entries().is_empty());
        assert_eq!(db.warnings().len(), 1);
        assert!(SourceMeta::open_project(&db.base, source.clone()).is_err());
        fs::remove_file(&path).unwrap();
        fs::remove_dir(db.base.join(".external")).unwrap();
        #[cfg(windows)]
        let link = std::os::windows::fs::symlink_dir(&external.0, db.base.join(".external"));
        #[cfg(unix)]
        let link = std::os::unix::fs::symlink(&external.0, db.base.join(".external"));
        if link.is_err() {
            return;
        }
        assert!(db.refresh(&registry).is_err());
        assert!(SourceMeta::open_project(&db.base, source).is_err());
        assert!(meta.set_tags(vec!["blocked".into()]).is_err());
        assert_eq!(fs::read_dir(&external.0).unwrap().count(), 1);
    }

    #[test]
    fn unicode_subsequence_and_contiguous_precedence() {
        assert!(match_score("étoile", "éie", true).is_some());
        assert!(match_score("étoile", "ieé", true).is_none());
        assert!(match_score("brick", "brick", true) < match_score("brightrock", "brick", true));
        assert!(match_score("brightrock", "brick", false).is_none());
    }

    #[test]
    fn symlinks_do_not_escape_or_cycle() {
        let fixture = Fixture::new();
        let external = Fixture::new();
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_dir(&external.0, fixture.0.join("outside"));
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(&external.0, fixture.0.join("outside"));
        // Windows developer mode or link privilege may be unavailable.
        if result.is_err() {
            return;
        }
        let (mut db, registry) = fixture.database();
        assert!(db.entries().is_empty());
        assert!(db.validate_folder(Path::new("outside")).is_err());
        fs::write(external.0.join("external.png"), b"external").unwrap();
        assert!(
            db.import_source(&fixture.0.join("outside/external.png"))
                .is_err()
        );
        assert!(!fixture.0.join("external.png").exists());
        fs::write(fixture.0.join("source.png"), b"source").unwrap();
        fs::write(external.0.join("metadata.json"), b"[]").unwrap();
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(
            external.0.join("metadata.json"),
            fixture.0.join("source.png.arc"),
        );
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(
            external.0.join("metadata.json"),
            fixture.0.join("source.png.arc"),
        );
        if result.is_err() {
            return;
        }
        db.refresh(&registry).unwrap();
        assert_eq!(db.warnings().len(), 1);
        assert!(
            db.set_tags(Path::new("source.png"), vec!["tag".into()])
                .is_err()
        );
        assert_eq!(fs::read(external.0.join("metadata.json")).unwrap(), b"[]");
    }
}
