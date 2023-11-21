use arcana::edict::world::WorldLocal;
use egui::Ui;
use egui_tracing::{EventCollector, Logs};

use super::Tab;

pub(super) struct Console {
    collector: EventCollector,
}

impl Console {
    pub fn new(collector: EventCollector) -> Self {
        Console { collector }
    }

    pub fn show(world: &WorldLocal, ui: &mut Ui) {
        let console = world.expect_resource::<Console>();
        ui.add(Logs::new(console.collector.clone()));
    }

    pub fn tab() -> Tab {
        Tab::Console
    }
}
