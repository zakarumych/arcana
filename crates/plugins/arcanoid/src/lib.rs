use arcana::{
    edict::{self, Component, Scheduler, View, World},
    egui::{EguiRender, EguiResource},
    events::{ElementState, KeyboardInput, VirtualKeyCode},
    na,
    plugin::ArcanaPlugin,
    render::RenderGraph,
    winit::window::Window,
};
use camera::Camera2;
use input::{new_commander, CommandQueue, InputHandler, Translator};
use physics::{geometry::ColliderBuilder, PhysicsResource};
use scene::Global2;
use sdf::SdfRender;

arcana::export_arcana_plugin!(ArcanoidPlugin);

pub struct ArcanoidPlugin;

impl ArcanaPlugin for ArcanoidPlugin {
    fn name(&self) -> &'static str {
        "arcanoid"
    }

    arcana::feature_ed! {
        fn dependencies(&self) -> Vec<(&'static dyn ArcanaPlugin, arcana::project::Dependency)> {
            vec![
                scene::path_dependency(),
                physics::path_dependency(),
                sdf::path_dependency(),
                input::path_dependency(),
            ]
        }
    }

    fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
        let camera = world
            .spawn((
                Global2::identity().translated(na::Vector2::new(0.0, 12.0)),
                Camera2::new().with_fovy(30.0),
            ))
            .id();

        {
            let world = world.local();
            let window = world.expect_resource::<Window>().id();
            let mut graph = world.expect_resource_mut::<RenderGraph>();

            // Create main pass.
            // It returns target id that it renders to.
            let mut target = SdfRender::build(camera, &mut graph);

            if world.get_resource::<EguiResource>().is_some() {
                target = EguiRender::build_overlay(target, &mut graph, window);
            }

            // Use window's surface for the render target.
            graph.present(target, window);
        }

        let paddle_body = {
            let mut physics = world.expect_resource_mut::<PhysicsResource>();
            let paddle_body = physics.new_position_body();

            physics.add_collider(&paddle_body, ColliderBuilder::cuboid(0.5, 0.5));
            paddle_body
        };

        let paddle = world
            .spawn((
                PaddleState {
                    left: false,
                    right: false,
                },
                sdf::Shape::new_rect(1.0, 1.0),
                Global2::identity(),
                paddle_body,
            ))
            .id();

        let commander = new_commander(PaddleTranslator, paddle, world).unwrap();

        world
            .expect_resource_mut::<InputHandler>()
            .add_global_controller(Box::new(commander));

        scheduler.add_system(paddle_system);
    }
}

pub struct PaddleTranslator;

pub enum PaddleAction {
    PaddleLeft,
    PaddleRight,
    PaddleUnLeft,
    PaddleUnRight,
}

impl Translator for PaddleTranslator {
    type Action = PaddleAction;

    fn on_keyboard_input(&mut self, input: &KeyboardInput) -> Option<PaddleAction> {
        match (input.virtual_keycode, input.state) {
            (Some(VirtualKeyCode::A), ElementState::Pressed) => Some(PaddleAction::PaddleLeft),
            (Some(VirtualKeyCode::D), ElementState::Pressed) => Some(PaddleAction::PaddleRight),
            (Some(VirtualKeyCode::A), ElementState::Released) => Some(PaddleAction::PaddleUnLeft),
            (Some(VirtualKeyCode::D), ElementState::Released) => Some(PaddleAction::PaddleUnRight),
            _ => None,
        }
    }
}

#[derive(Component)]
struct PaddleState {
    left: bool,
    right: bool,
}

fn paddle_system(
    view: View<(
        &mut PaddleState,
        &mut Global2,
        &mut CommandQueue<PaddleAction>,
    )>,
) {
    for (state, global, queue) in view {
        for action in queue.drain() {
            match action {
                PaddleAction::PaddleLeft => state.left = true,
                PaddleAction::PaddleRight => state.right = true,
                PaddleAction::PaddleUnLeft => state.left = false,
                PaddleAction::PaddleUnRight => state.right = false,
            }
        }

        match (state.left, state.right) {
            (true, false) => global.iso.translation.x -= 1.0,
            (false, true) => global.iso.translation.x += 1.0,
            _ => {}
        }

        if global.iso.translation.x < -15.0 {
            global.iso.translation.x = -15.0;
        }
        if global.iso.translation.x > 15.0 {
            global.iso.translation.x = 15.0;
        }
    }
}
