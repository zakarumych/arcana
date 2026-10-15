//! Block-breaker gameplay and playfield rendering for Arcana.
use arcana::{
    ecs::world::World,
    gametime::Tick,
    input::{ElementState, Input, KeyCode, PhysicalKey, ViewInput},
    plugin::{FilterInfo, JobInfo, SystemInfo},
    render::RenderGraphId,
    work_graph::{Job, JobId},
};

mod game;
mod render;
mod scene;
mod ui;

use game::Game;
pub use render::BlockbreakerFrame;

arcana::declare_plugin!();

pub const GRAPH_ID: RenderGraphId = arcana::id::static_id!(0x42424B01 as RenderGraphId);

#[arcana::init]
fn init_game(world: &mut World) {
    world.insert_resource(Game::new());
}

arcana::plugin::plugin_ctor_add!(ARCANA_PLUGIN_REGISTRY_main @ plugin => {
    scene::register(plugin);
});

#[arcana::system]
fn update(world: &mut World) {
    let dt = world
        .get_resource::<Tick>()
        .map_or(0.0, |tick| tick.span.as_secs_f32());
    if let Some(mut game) = world.remove_resource::<Game>() {
        game.advance(world, dt);
        world.insert_resource(game);
    }
    let _ = arcana::scene::update_transforms(world);
    ui::update(world);
}

#[arcana::filter]
fn controls(world: &mut World, input: &Input) -> bool {
    let Input::ViewInput { input } = input else {
        return false;
    };

    let Some(mut game) = world.get_resource_mut::<Game>() else {
        return false;
    };

    match input {
        ViewInput::CursorLeft { .. } => {
            game.keys = [false; 4];
            false
        }
        ViewInput::KeyboardInput { event, .. } => {
            let down = event.state == ElementState::Pressed;
            match event.physical_key {
                PhysicalKey::Code(KeyCode::ArrowLeft) => game.keys[0] = down,
                PhysicalKey::Code(KeyCode::KeyA) => game.keys[1] = down,
                PhysicalKey::Code(KeyCode::ArrowRight) => game.keys[2] = down,
                PhysicalKey::Code(KeyCode::KeyD) => game.keys[3] = down,
                PhysicalKey::Code(KeyCode::Space) => {
                    if down && !event.repeat {
                        game.launch();
                    }
                }
                PhysicalKey::Code(KeyCode::KeyR) => {
                    if down && !event.repeat {
                        game.restart = true;
                    }
                }
                _ => return false,
            }
            true
        }
        _ => false,
    }
}
