use std::path::Path;

use egui::Ui;

mod store;

use store::{Store, StoreInfo};

/// Assets viewer.
pub struct Assets {
    store: Store,
}

impl Assets {
    pub fn new(base: &Path) -> Self {
        Self {
            store: Store::new(base, StoreInfo::default()).expect("Failed to create asset store"),
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.label("Assets");
    }
}
