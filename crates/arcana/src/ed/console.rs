use edict::World;
use egui::Ui;

use super::Tab;

pub(super) struct Console {
    collector: egui_tracing::EventCollector,
}

impl Console {
    pub fn new(collector: egui_tracing::EventCollector) -> Self {
        Console { collector }
    }

    pub fn show(world: &mut World, ui: &mut Ui) {
        let console = world.expect_resource::<Console>();
        ui.add(egui_tracing::Logs::new(console.collector.clone()));
    }

    pub fn tab() -> Tab {
        Tab::Console
    }
}
