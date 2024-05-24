use arcana::{input::Input, Blink, World};

arcana::export_arcana_plugin! {
    DummyPlugin {
        systems: [fake1: || {}, fake2: || {}],
        filters: [fake1: |_: &Blink, _: &mut World, _: &Input| false, fake2: |_: &Blink, _: &mut World, _: &Input| true],
    }
}
