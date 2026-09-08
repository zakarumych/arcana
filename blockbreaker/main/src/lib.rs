//! A complete block-breaker game in one plugin and one frame-rendering job.
use arcana::{
    ecs::{
        resources::{Res, ResMut},
        world::World,
    },
    gametime::ClockStep,
    input::{ElementState, Input, KeyCode, PhysicalKey, ViewInput},
    plugin::{FilterInfo, JobInfo, SystemInfo},
    render::{RenderGraphId, Renderer},
    work_graph::{Job, JobId},
};

mod game;
mod render;

use game::Game;
pub use render::BlockbreakerFrame;

arcana::plugin::declare_plugin!();

pub const GRAPH_ID: RenderGraphId = arcana::id::static_id!(0x42424B01 as RenderGraphId);

pub const JOB_ID: JobId = arcana::id::static_id!(0x42424B02 as JobId);

pub const SYSTEM_ID: arcana::ecs::SystemId =
    arcana::id::static_id!(0x42424B03 as arcana::ecs::SystemId);

pub const FILTER_ID: arcana::input::FilterId =
    arcana::id::static_id!(0x42424B04 as arcana::input::FilterId);

arcana::plugin::plugin_ctor_add!(ARCANA_PLUGIN_REGISTRY @ plugin => {
    plugin.add_init(|world| {
        world.insert_resource(Game::new());
        world.spawn_one(Renderer { graph: GRAPH_ID });
    });
    plugin.add_system(SystemInfo { id: SYSTEM_ID, name: arcana::name!("Blockbreaker update"), location: None }, |hub| hub.add_system(SYSTEM_ID, update));
    plugin.add_filter(FilterInfo { id: FILTER_ID, name: arcana::name!("Blockbreaker controls"), location: None }, |hub| hub.add_filter(FILTER_ID, controls));
    plugin.add_job(JobInfo { id: JOB_ID, name: BlockbreakerFrame::name(), desc: BlockbreakerFrame::desc(), location: None }, |hub| hub.add_job(JOB_ID, BlockbreakerFrame::new()));
});

fn update(mut game: ResMut<Game>, step: Res<ClockStep>) {
    game.advance(step.step.as_secs_f32());
}

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
                        *game = Game::new();
                    }
                }
                _ => return false,
            }
            true
        }
        _ => false,
    }
}
