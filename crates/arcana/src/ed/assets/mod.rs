use std::{path::Path, sync::Arc};

use egui::Ui;

mod store;

use store::{Store, StoreInfo};

use crate::assets::Loader;

/// Assets viewer.
pub struct Assets {
    store: Arc<Store>,
    cache: crate::assets::Assets,
}

impl Assets {
    pub fn new(base: &Path) -> Self {
        let store = Store::new(base, StoreInfo::default()).expect("Failed to create asset store");
        let store = Arc::new(store);
        let cache = crate::assets::Assets::new([store.clone() as Arc<dyn Loader>]);
        Assets { store, cache }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.label("Assets");
    }

    pub fn cache(&self) -> &crate::assets::Assets {
        &self.cache
    }
}
