
let store_path = Path::new(env!("CARGO_MANIFEST_DIR"))
.parent()
.unwrap()
.join("argosy.toml");

if !store_path.exists() {
let info = argosy_store::StoreInfo {
    artifacts: Some(Path::new("target/artifacts").to_owned()),
    external: Some(Path::new("target/external").to_owned()),
    temp: Some(Path::new("target/temp").to_owned()),
    importers: vec![Path::new("target/debug/arcana_importers").to_owned()],
};
info.write(&store_path).unwrap();
}

let store = argosy_store::Store::find(&store_path).unwrap();