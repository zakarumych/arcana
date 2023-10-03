use arcana::{
    edict::{Scheduler, World},
    plugin::ArcanaPlugin,
};

arcana::export_arcana_plugin!(DummyPlugin);

pub struct DummyPlugin;

impl ArcanaPlugin for DummyPlugin {
    fn name(&self) -> &'static str {
        "dummy"
    }

    fn init(&self, _world: &mut World, _scheduler: &mut Scheduler) {}
}
