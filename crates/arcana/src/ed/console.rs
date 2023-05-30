use std::io;

use edict::World;
use egui::Ui;

use crate::ed::AppTab;

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

    pub fn tab() -> Box<dyn AppTab> {
        struct ConsoleTab;

        impl AppTab for ConsoleTab {
            fn title(&self) -> &'static str {
                "Console"
            }

            fn show(&mut self, world: &mut World, ui: &mut Ui) {
                Console::show(world, ui);
            }
        }

        Box::new(ConsoleTab)
    }
}
