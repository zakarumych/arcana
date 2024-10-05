use egui::Ui;
use egui_tracing::{EventCollector, Logs};

pub(super) struct Console {
    collector: EventCollector,
}

impl Console {
    pub fn new(collector: EventCollector) -> Self {
        Console { collector }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.add(Logs::new(self.collector.clone()));
    }
}
