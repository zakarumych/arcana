use edict::World;
use egui::Ui;

use crate::alloc::ArcanaAllocator;

use super::Tab;

pub(super) struct Memory;

impl Memory {
    pub fn show(world: &mut World, ui: &mut Ui) {
        let stats = ArcanaAllocator::global_stats();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Allocations:");
                ui.label("Deallocations:");
                ui.label("Allocated bytes:");
                ui.label("Deallocated bytes:");
                ui.label("Used bytes:");
            });
            ui.vertical(|ui| {
                ui.label(format!("{}", stats.allocations));
                ui.label(format!("{}", stats.deallocations));
                ui.label(format!("{}", stats.allocated_bytes));
                ui.label(format!("{}", stats.deallocated_bytes));
                ui.label(format!(
                    "{}",
                    stats.allocated_bytes - stats.deallocated_bytes
                ));
            });
        });
    }

    // pub fn tab() -> Tab {
    //     Tab::Memory
    // }
}
