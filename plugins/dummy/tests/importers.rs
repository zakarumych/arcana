use std::path::{Path, PathBuf};

use arcana::{
    assets::{
        AssetId,
        import::{ImportContext, ImportError, Importer, ImporterDesc, Missing},
    },
    ecs::world::World,
    model::{Model, Value},
    plugin::PluginsHub,
};

arcana::declare_plugin!();

mod nested {
    use super::*;

    #[derive(Default)]
    pub struct TestImporter;

    #[arcana::importer]
    impl Importer for TestImporter {
        fn desc() -> ImporterDesc {
            ImporterDesc {
                name: arcana::format_name!("Test importer"),
                formats: vec![arcana::format_name!("Test source")],
                extensions: vec!["test"],
                target: arcana::ident!(test_asset),
                config: (Model::Bool, Value::Bool(true)),
            }
        }

        fn import(
            &self,
            source: &Path,
            output: &Path,
            config: Value,
            _: &mut dyn ImportContext,
        ) -> Result<(), ImportError> {
            assert_eq!(source, Path::new("source.test"));
            assert_eq!(output, Path::new("output.asset"));
            assert_eq!(config, Value::Bool(true));
            Ok(())
        }
    }
}

struct NoDependencies;

impl ImportContext for NoDependencies {
    fn get_dependency(&mut self, _: &str, _: arcana::Ident) -> Result<AssetId, Missing> {
        Err(Missing)
    }

    fn get_source(&mut self, _: &str) -> Result<PathBuf, Missing> {
        Err(Missing)
    }
}

#[test]
fn importer_registers_descriptor_and_usable_instance() {
    let plugin = arcana_plugin::get();
    assert_eq!(plugin.importers().len(), 1);
    let info = &plugin.importers()[0];
    assert_eq!(info.name, arcana::format_name!("Test importer"));
    assert_eq!(info.desc.name, info.name);
    assert_eq!(info.desc.formats, vec![arcana::format_name!("Test source")]);
    assert_eq!(info.desc.extensions, vec!["test"]);
    assert_eq!(info.desc.target, arcana::ident!(test_asset));
    assert_eq!(info.desc.config, (Model::Bool, Value::Bool(true)));
    assert!(info.location.as_ref().is_some_and(|location| {
        location.file.ends_with("importers.rs") && location.line > 0 && location.column > 0
    }));

    let mut hub = PluginsHub::new();
    plugin.init(&mut World::new(), &mut hub);
    assert_eq!(hub.importers.len(), 1);
    let importer = hub.importers.get(&info.id).expect("registered importer");
    assert!(
        importer
            .import(
                Path::new("source.test"),
                Path::new("output.asset"),
                info.desc.config.1.clone(),
                &mut NoDependencies,
            )
            .is_ok()
    );
}
