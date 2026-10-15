use arcana::{
    assets::{
        AssetId,
        import::{ImportContext, ImportError, Importer, Missing},
    },
    model::{Model, Value},
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tilemap::{MapData, MapImporter, TilesetData, TilesetImporter};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "arcana-tileset-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn import(
        &self,
        json: &[u8],
        config: Value,
        context: &mut Context,
    ) -> Result<TilesetData, ImportError> {
        let source = self.0.join("tileset.json");
        let output = self.0.join("tileset.asset");
        std::fs::write(&source, json).unwrap();
        TilesetImporter.import(&source, &output, config, context)?;
        Ok(serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap())
    }

    fn import_map(
        &self,
        json: &[u8],
        config: Value,
        context: &mut Context,
    ) -> Result<MapData, ImportError> {
        let source = self.0.join("map.json");
        let output = self.0.join("map.asset");
        std::fs::write(&source, json).unwrap();
        MapImporter.import(&source, &output, config, context)?;
        Ok(serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Context {
    id: Option<AssetId>,
    requested: Vec<(String, arcana::Ident)>,
}
impl ImportContext for Context {
    fn get_dependency(&mut self, source: &str, target: arcana::Ident) -> Result<AssetId, Missing> {
        self.requested.push((source.into(), target));
        self.id.ok_or(Missing)
    }
    fn get_source(&mut self, _: &str) -> Result<PathBuf, Missing> {
        panic!("No raw image access required")
    }
}
const SAMPLE: &[u8] = include_bytes!("../../../assets/tilemap/tileset.json");
const MAP_SAMPLE: &[u8] = include_bytes!("../../../assets/tilemap/map.json");
fn high_id() -> AssetId {
    AssetId::new(std::num::NonZeroU64::new(u64::MAX - 7).unwrap())
}

#[test]
fn sample_requests_texture_then_preserves_data_and_full_width_id() {
    let fixture = Fixture::new();
    let mut context = Context::default();
    let Err(ImportError::Requires {
        sources,
        dependencies,
    }) = fixture.import(SAMPLE, Value::Unit, &mut context)
    else {
        panic!("Expected texture request")
    };
    assert!(sources.is_empty());
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].source, "atlas.png");
    assert_eq!(dependencies[0].target, arcana::ident!(Texture));
    assert!(dependencies[0].format.is_none());
    assert!(!fixture.0.join("tileset.asset").exists());
    context.id = Some(high_id());
    let artifact = fixture
        .import(SAMPLE, Value::Unit, &mut context)
        .unwrap_or_else(|_| panic!("Import failed"));
    assert_eq!(artifact.image, high_id());
    assert_eq!(artifact.tile_size, [16.0, 16.0]);
    assert_eq!(artifact.tiles[0].name, "ground");
    assert_eq!(artifact.tiles[0].region, [0, 0, 16, 16]);
    assert_eq!(artifact.tiles[1].name, "platform");
    assert_eq!(artifact.tiles[1].region, [16, 0, 16, 16]);
    assert_eq!(
        context.requested,
        vec![("atlas.png".into(), arcana::ident!(Texture)); 2]
    );
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.0.join("tileset.asset")).unwrap()).unwrap();
    assert_eq!(json["image"].as_u64(), Some(u64::MAX - 7));
}

#[test]
fn malformed_sources_and_configuration_fail_before_dependency_lookup() {
    let fixture = Fixture::new();
    let sample: serde_json::Value = serde_json::from_slice(SAMPLE).unwrap();
    let cases = [
        ("image", serde_json::json!(3)),
        ("image", serde_json::json!("")),
        ("tile_size", serde_json::json!([0, 16])),
        ("tile_size", serde_json::json!([-1, 16])),
        ("tile_size", serde_json::json!([1e100, 16])),
        ("tiles", serde_json::json!([])),
        (
            "tiles",
            serde_json::json!([{"name":"", "region":[0,0,1,1]}]),
        ),
        (
            "tiles",
            serde_json::json!([{"name":"bad\nname", "region":[0,0,1,1]}]),
        ),
        (
            "tiles",
            serde_json::json!([{"name":"x", "region":[0,0,0,1]}]),
        ),
        (
            "tiles",
            serde_json::json!([{"name":"x", "region":[4294967295u32,0,1,1]}]),
        ),
        (
            "tiles",
            serde_json::json!([{"name":"x", "region":[0,0,1,1]}, {"name":"x", "region":[1,0,1,1]}]),
        ),
        (
            "tiles",
            serde_json::json!([{"name":"x", "region":[0,0,1,1], "typo":true}]),
        ),
        ("typo", serde_json::json!(true)),
    ];
    let mut context = Context::default();
    for (field, value) in cases {
        let mut source = sample.clone();
        source[field] = value;
        assert!(
            matches!(
                fixture.import(
                    &serde_json::to_vec(&source).unwrap(),
                    Value::Unit,
                    &mut context
                ),
                Err(ImportError::Other { .. })
            ),
            "Accepted {source}"
        );
    }
    assert!(matches!(
        fixture.import(b"{", Value::Unit, &mut context),
        Err(ImportError::Other { .. })
    ));
    assert!(matches!(
        fixture.import(SAMPLE, Value::Bool(true), &mut context),
        Err(ImportError::Other { .. })
    ));
    assert!(context.requested.is_empty());
}

#[test]
fn map_requests_tileset_then_preserves_cells_and_full_width_id() {
    let fixture = Fixture::new();
    let mut context = Context::default();
    let Err(ImportError::Requires {
        sources,
        dependencies,
    }) = fixture.import_map(MAP_SAMPLE, Value::Unit, &mut context)
    else {
        panic!("Expected tileset request")
    };
    assert!(sources.is_empty());
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].source, "tileset.json");
    assert_eq!(dependencies[0].target, arcana::ident!(Tileset));
    assert!(dependencies[0].format.is_none());
    assert!(!fixture.0.join("map.asset").exists());
    context.id = Some(high_id());
    let artifact = fixture
        .import_map(MAP_SAMPLE, Value::Unit, &mut context)
        .unwrap_or_else(|_| panic!("Import failed"));
    let source: serde_json::Value = serde_json::from_slice(MAP_SAMPLE).unwrap();
    assert_eq!(artifact.tileset, high_id());
    assert_eq!(
        serde_json::to_value(&artifact.cells).unwrap(),
        source["cells"]
    );
    assert_eq!(
        context.requested,
        vec![("tileset.json".into(), arcana::ident!(Tileset)); 2]
    );
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.0.join("map.asset")).unwrap()).unwrap();
    assert_eq!(json["tileset"].as_u64(), Some(u64::MAX - 7));
}

#[test]
fn malformed_maps_and_configuration_fail_before_dependency_lookup() {
    let fixture = Fixture::new();
    let sample: serde_json::Value = serde_json::from_slice(MAP_SAMPLE).unwrap();
    let cases = [
        ("tileset", serde_json::json!(2)),
        ("tileset", serde_json::json!(" ")),
        ("tileset", serde_json::json!("bad\npath")),
        ("cells", serde_json::json!([])),
        ("cells", serde_json::json!([[]])),
        ("cells", serde_json::json!([[1], []])),
        ("cells", serde_json::json!([[1], [1, 2]])),
        ("cells", serde_json::json!([[-1]])),
        ("cells", serde_json::json!([[1.5]])),
        ("cells", serde_json::json!([[4294967296u64]])),
        ("typo", serde_json::json!(true)),
    ];
    let mut context = Context::default();
    for (field, value) in cases {
        let mut source = sample.clone();
        source[field] = value;
        assert!(
            matches!(
                fixture.import_map(
                    &serde_json::to_vec(&source).unwrap(),
                    Value::Unit,
                    &mut context
                ),
                Err(ImportError::Other { .. })
            ),
            "Accepted {source}"
        );
    }
    for json in [b"{".as_slice(), b"{}", b"{\"tileset\":\"x\"}"] {
        assert!(matches!(
            fixture.import_map(json, Value::Unit, &mut context),
            Err(ImportError::Other { .. })
        ));
    }
    assert!(matches!(
        fixture.import_map(MAP_SAMPLE, Value::Bool(true), &mut context),
        Err(ImportError::Other { .. })
    ));
    assert!(context.requested.is_empty());
    context.id = Some(high_id());
    let source = br#"{"tileset":"tileset.json","cells":[[0,4294967295]]}"#;
    assert_eq!(
        fixture
            .import_map(source, Value::Unit, &mut context)
            .unwrap_or_else(|_| panic!("Import failed"))
            .cells,
        [vec![0, u32::MAX]]
    );
}

#[test]
fn plugin_registers_descriptor_and_usable_instance() {
    let plugin = tilemap::arcana_plugin::get();
    assert_eq!(plugin.importers().len(), 2);
    let mut hub = arcana::plugin::PluginsHub::new();
    plugin.init(&mut arcana::ecs::world::World::new(), &mut hub);
    for (target, format, sample) in [
        (
            arcana::ident!(Tileset),
            arcana::name!("JSON tileset"),
            SAMPLE,
        ),
        (arcana::ident!(Map), arcana::name!("JSON map"), MAP_SAMPLE),
    ] {
        let info = plugin
            .importers()
            .iter()
            .find(|info| info.desc.target == target)
            .unwrap();
        assert_eq!(info.desc.extensions, ["json"]);
        assert_eq!(info.desc.formats, [format]);
        assert_eq!(info.desc.config, (Model::Unit, Value::Unit));
        let fixture = Fixture::new();
        let source = fixture.0.join("tileset.json");
        std::fs::write(&source, sample).unwrap();
        let mut context = Context {
            id: Some(high_id()),
            ..Context::default()
        };
        assert!(
            hub.importers[&info.id]
                .import(
                    Path::new(&source),
                    &fixture.0.join("output"),
                    Value::Unit,
                    &mut context
                )
                .is_ok()
        );
    }
}
