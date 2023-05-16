use bob::{
    edict::world::WorldBuilder,
    gametime::FrequencyTicker,
    nix,
    render::{RenderTarget, RenderTargetAlwaysUpdate, RenderTargetUpdate},
    winit,
};

fn main() {
    bob::install_tracing_subscriber();

    // Build the world.
    // Register external resources.
    let mut world_builder = WorldBuilder::new();
    world_builder.register_external::<winit::window::Window>();
    world_builder.register_external::<FrequencyTicker>();
    world_builder.register_external::<nix::Surface>();
    world_builder.register_component::<RenderTarget>();
    world_builder.register_component::<RenderTargetAlwaysUpdate>();
    world_builder.register_component::<RenderTargetUpdate>();

    let mut world = world_builder.build();
}
