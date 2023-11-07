use arcana::{
    edict::{self, ActionEncoder, Component, Entities, Scheduler, View, World},
    egui::{EguiRender, EguiResource},
    events::{ElementState, KeyboardInput, VirtualKeyCode},
    na,
    plugin::ArcanaPlugin,
    render::RenderGraph,
    winit::window::Window,
};
use camera::Camera2;
use input::{insert_global_entity_controller, ActionQueue, InputHandler, Translator};
use motion::{Motion2, MoveAfter2, MoveTo2};
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
                motion::path_dependency(),
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

        let target = world
            .spawn((
                sdf::Shape::new_rect(1.0, 1.0).with_color([0.1, 0.8, 0.2, 1.0]),
                Global2::identity().translated(na::Vector2::new(-10.0, 20.0)),
            ))
            .id();

        let paddle = world
            .spawn((
                // PaddleState {
                //     left: false,
                //     right: false,
                // },
                sdf::Shape::new_rect(1.0, 1.0),
                Global2::identity(),
                Motion2 {
                    velocity: na::Vector2::new(10.0, 15.0),
                    acceleration: na::Vector2::zeros(),
                    deceleration: 0.0,
                },
                // MoveTo2 {
                //     target: na::Point2::new(-10.0, 3.0),
                //     acceleration: 20.0,
                //     velocity: 10.0,
                //     threshold: 4.0,
                // },
                MoveAfter2 {
                    id: target,
                    global_offset: na::Vector2::zeros(),
                    local_offset: na::Vector2::zeros(),
                    velocity: 10.0,
                    acceleration: 12.0,
                    threshold: 4.0,
                },
            ))
            .id();

        // insert_global_entity_controller(PaddleTranslator, paddle, world).unwrap();

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

#[derive(Clone, Copy, Component)]
struct PaddleState {
    left: bool,
    right: bool,
}

fn paddle_system(
    paddles: View<(Entities, &mut PaddleState, &mut ActionQueue<PaddleAction>)>,
    mut move_to: View<&mut MoveTo2>,
    mut encoder: ActionEncoder,
) {
    for (e, state, queue) in paddles {
        for action in queue.drain() {
            match action {
                PaddleAction::PaddleLeft => state.left = true,
                PaddleAction::PaddleRight => state.right = true,
                PaddleAction::PaddleUnLeft => state.left = false,
                PaddleAction::PaddleUnRight => state.right = false,
            }
        }

        let target = match (state.left, state.right) {
            (true, true) | (false, false) => {
                if move_to.get_mut(e).is_some() {
                    encoder.drop_bundle::<(MoveTo2, Motion2)>(e);
                }
                continue;
            }
            (true, false) => na::Point2::new(-15.0, 12.0),
            (false, true) => na::Point2::new(15.0, 12.0),
        };

        // let m = MoveTo2 {
        //     target,
        //     acceleration: 1.0,
        //     max_velocity: 3.0,
        //     threshold: 0.1,
        // };

        // match move_to.get_mut(e) {
        //     Some(motion) => *motion = m,
        //     None => encoder.insert(e, m),
        // }

        // if global.iso.translation.x < -15.0 {
        //     global.iso.translation.x = -15.0;
        // }
        // if global.iso.translation.x > 15.0 {
        //     global.iso.translation.x = 15.0;
        // }
    }
}
