//! Pixie widgets in Arcana's world and render graph.
pub use pixie_ui;
pub use pixie_ui_mev;
mod render;
mod sync;
use arcana::ecs::world::World;

arcana::declare_plugin!();

#[arcana::init]
fn init_ui(world: &mut World) {
    world.with_resource(pixie_ui::ui::UiState::new);
}
