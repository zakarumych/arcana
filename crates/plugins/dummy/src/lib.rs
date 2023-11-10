use arcana::plugin::ArcanaPlugin;

arcana::export_arcana_plugin!(DummyPlugin);

pub struct DummyPlugin;

impl ArcanaPlugin for DummyPlugin {}
