use arcana::{events::Event, Blink, World};

arcana::export_arcana_plugin! {
    DummyPlugin {
        systems: [fake1: || {}, fake2: || {}],
        filters: [fake1: |_: &Blink, _: &mut World, _: &Event| false, fake2: |_: &Blink, _: &mut World, _: &Event| true],
    }
}
