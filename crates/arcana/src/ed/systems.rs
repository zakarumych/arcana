use arcana_project::{IdentBuf, Project};
use edict::{System, World};
use egui::Ui;
use hashbrown::HashMap;

use super::Tab;

pub struct Systems {}

impl Systems {
    pub fn new() -> Self {
        Systems {}
    }

    pub fn tab() -> Tab {
        Tab::Systems
    }

    pub fn show(world: &mut World, ui: &mut Ui) {
        let world = world.local();
        let mut project = world.expect_resource_mut::<Project>();
    }
}
