use arcana::{
    edict::{self, spawn_block, ActionEncoder, Component, Entities, Res, View, World},
    events::{ElementState, KeyboardInput, VirtualKeyCode},
    flow::sleep,
    gametime::{timespan, TimeSpan},
    na,
    render::RenderGraph,
    viewport::Viewport,
    ClockStep,
};
use camera::Camera2;
use cursor::MainCursor;
use input::Translator;
use motion::dim2::{Motor, Move, MoveAfter, MoveTo};
use physics::dim2::{
    pipeline::ActiveEvents, Collider, Collision, CollisionEvents, CollisionState, PhysicsResource,
    RigidBody,
};
use scene::Global2;
use sdf::SdfRender;

#[derive(Component)]
pub struct BallComponent;

arcana::export_arcana_plugin! {
    ArcanoidPlugin {
        dependencies: [
            scene ...,
            physics ...,
            sdf ...,
            input ...,
            motion ...,
            cursor ...,
        ],
        systems: [
            target_cursor: move |cursor: Res<MainCursor>,
                viewport: Res<Viewport>,
                mut r#move: View<&mut Move>,
                cameras: View<(&Camera2, &Global2)>| {
                    let extent = viewport.extent();

                    // Ignore when viewport is zero-sized.
                    if extent.width() == 0 || extent.height() == 0 {
                        return;
                    }

                    let point = na::Point2::new(cursor.x / extent.width() as f32 * 2.0 - 1.0, 1.0 - cursor.y / extent.height() as f32 * 2.0);

                    let ratio = extent.width() as f32 / extent.height() as f32;

                    let (camera, camera_global) = cameras.try_get(camera).unwrap();

                    let position = camera
                        .viewport
                        .transform(1.0, ratio)
                        .transform_point(&point);

                    let position = camera_global.iso.transform_point(&position);
                    *r#move.try_get_mut(target).unwrap() = Move::To(MoveTo::new(position));
                },
            burst_system,
        ],

        in world => {
            let camera = world
                .spawn((Global2::identity(), Camera2::new().with_fovy(15.0)))
                .id();

            {
                let world = world.local();
                let mut graph = world.expect_resource_mut::<RenderGraph>();

                // Create main pass.
                // It returns target id that it renders to.
                let target = SdfRender::build(camera, &mut graph);

                // if world.get_resource::<EguiResource>().is_some() {
                //     target = EguiRender::build_overlay(target, &mut graph, window);
                // }

                // Use window's surface for the render target.
                graph.present(target);
            }

            let target = world.allocate().id();
            let mut last_ball = target;

            world.insert_bundle(
                target,
                (
                    sdf::Shape::circle(1.0).with_color([
                        rand::random(),
                        rand::random(),
                        rand::random(),
                        1.0,
                    ]),
                    Global2::identity(),
                    RigidBody::dynamic(),
                    Collider::ball(1.0),
                    Move::to(na::Point2::new(0.0, 0.0)),
                    Motor::new(30.0, 100.0),
                    BallComponent,
                )
            ).unwrap();

            // insert_global_entity_controller(PaddleTranslator, paddle, world).unwrap();

            let left_side = Collider::halfspace(na::UnitVector2::new_unchecked(na::Vector2::x())).position(na::Translation2::new(-15.0, 0.0).into());
            let right_side = Collider::halfspace(na::UnitVector2::new_unchecked(-na::Vector2::x())).position(na::Translation2::new(15.0, 0.0).into());
            let top_side = Collider::halfspace(na::UnitVector2::new_unchecked(-na::Vector2::y())).position(na::Translation2::new(0.0, 15.0).into());
            let bottom_side = Collider::halfspace(na::UnitVector2::new_unchecked(na::Vector2::y())).position(na::Translation2::new(0.0, -15.0).into());


            world.spawn_one(left_side);
            world.spawn_one(right_side);
            world.spawn_one(top_side);
            world.spawn_one(bottom_side);

            let mut new_node = move |world: &mut World| {
                let id = world.allocate().id();

                let global = Global2::from_position(na::Point2::new(
                    rand::random::<f32>() * 26.0 - 13.0,
                    rand::random::<f32>() * 26.0 - 13.0,
                ));

                world
                    .insert_bundle(id, (
                        sdf::Shape::circle(1.0).with_color([
                            rand::random(),
                            rand::random(),
                            rand::random(),
                            1.0,
                        ]),
                        RigidBody::dynamic().position(global.iso),
                        global,
                        Collider::ball(1.0).active_events(ActiveEvents::COLLISION_EVENTS),
                        Move::After(MoveAfter::new(last_ball).with_distance(2.0)),
                        Motor::new(10.0, 100.0),
                        CollisionEvents::new(),
                        BallComponent,
                    )).unwrap();

                last_ball = id;

                spawn_block!(in ref world for last_ball -> {
                    loop {
                        let event: Collision = CollisionEvents::async_deque_from(&mut last_ball).await;

                        if event.state == CollisionState::Started {
                            if let Some(other) = event.other {
                                if last_ball.world().try_has_component::<BallComponent>(other).unwrap_or(false) {
                                    // // Despawn on any collision.
                                    // for _ in 0..100 {
                                    //     let mut s = last_ball.get_copied::<sdf::Shape>().unwrap();
                                    //     s.transform *= na::Similarity2::from_scaling(1.01);
                                    //     last_ball.set(s).unwrap();

                                    //     sleep(timespan!(0.02 s), &mut last_ball.world()).await;
                                    // }
                                    // let _ = last_ball.despawn();
                                    // yield_now!();

                                    let _ = last_ball.insert(Burst { span: TimeSpan::ZERO, scale: 1.0 });
                                    return;
                                }
                            }
                        }
                    }
                });
            };

            spawn_block!(in ref world -> {
                sleep(timespan!(2 seconds), &mut world).await;
                for _ in 0.. {
                    new_node(&mut world);
                    sleep(timespan!(1 s), &mut world).await;
                }
            });
        }
    }
}

pub struct PaddleTranslator;

pub enum PaddleAction {
    // PaddleLeft,
    // PaddleRight,
    // PaddleUnLeft,
    // PaddleUnRight,
    Switch,
}

impl Translator for PaddleTranslator {
    type Action = PaddleAction;

    fn on_keyboard_input(&mut self, input: &KeyboardInput) -> Option<PaddleAction> {
        match (input.virtual_keycode, input.state) {
            // (Some(VirtualKeyCode::A), ElementState::Pressed) => Some(PaddleAction::PaddleLeft),
            // (Some(VirtualKeyCode::D), ElementState::Pressed) => Some(PaddleAction::PaddleRight),
            // (Some(VirtualKeyCode::A), ElementState::Released) => Some(PaddleAction::PaddleUnLeft),
            // (Some(VirtualKeyCode::D), ElementState::Released) => Some(PaddleAction::PaddleUnRight),
            (Some(VirtualKeyCode::Space), ElementState::Released) => Some(PaddleAction::Switch),
            _ => None,
        }
    }
}

// #[derive(Clone, Copy, Component)]
// struct PaddleState {
//     // left: bool,
//     // right: bool,
//     chasing: MoveAfter,
// }

// fn paddle_system(
//     paddles: View<(
//         Entities,
//         &mut PaddleState,
//         Option<&MoveAfter>,
//         &mut ActionQueue<PaddleAction>,
//     )>,
//     mut encoder: ActionEncoder,
// ) {
//     for (e, state, ma, queue) in paddles {
//         if let Some(ma) = ma {
//             state.chasing = *ma;
//         }

//         for action in queue.drain() {
//             match action {
//                 // PaddleAction::PaddleLeft => state.left = true,
//                 // PaddleAction::PaddleRight => state.right = true,
//                 // PaddleAction::PaddleUnLeft => state.left = false,
//                 // PaddleAction::PaddleUnRight => state.right = false,
//                 PaddleAction::Switch => match ma {
//                     None => encoder.insert(e, state.chasing),
//                     Some(_) => encoder.drop::<MoveAfter>(e),
//                 },
//             }
//         }

//         // let target = match (state.left, state.right) {
//         //     (true, true) | (false, false) => {
//         //         if move_to.get_mut(e).is_some() {
//         //             encoder.drop_bundle::<(MoveTo, Motion2)>(e);
//         //         }
//         //         continue;
//         //     }
//         //     (true, false) => na::Point2::new(-15.0, 12.0),
//         //     (false, true) => na::Point2::new(15.0, 12.0),
//         // };

//         // let m = MoveTo {
//         //     target,
//         //     acceleration: 1.0,
//         //     max_velocity: 3.0,
//         //     threshold: 0.1,
//         // };

//         // match move_to.get_mut(e) {
//         //     Some(motion) => *motion = m,
//         //     None => encoder.insert(e, m),
//         // }

//         // if global.iso.translation.x < -15.0 {
//         //     global.iso.translation.x = -15.0;
//         // }
//         // if global.iso.translation.x > 15.0 {
//         //     global.iso.translation.x = 15.0;
//         // }
//     }
// }

#[derive(Component)]
struct Burst {
    span: TimeSpan,
    scale: f32,
}

fn burst_system(
    burst: View<(Entities, &mut Burst, &mut sdf::Shape, &Global2)>,
    mut bodies: View<(&mut RigidBody, &Global2)>,
    clock: Res<ClockStep>,
    mut encoder: ActionEncoder,
    physics: Res<PhysicsResource>,
) {
    for (e, burst, shape, global) in burst {
        burst.span += clock.step;
        if burst.span >= TimeSpan::SECOND {
            encoder.despawn(e);

            physics.intersections_with_shape(
                &global.iso,
                &physics::dim2::geometry::Ball::new(30.0),
                |_collider, body| {
                    if let Some(body) = body {
                        if let Ok((body, body_global)) = bodies.try_get_mut(body) {
                            let offset =
                                body_global.iso.translation.vector - global.iso.translation.vector;

                            let dir = offset.normalize();
                            let d = offset.norm();

                            body.apply_impulse(dir * 200.0 / d);
                        }
                    }
                },
            )
        } else {
            let new_scale = 2f32.powf(burst.span.as_secs_f32());
            shape.transform *= na::Similarity2::from_scaling(new_scale / burst.scale);
            burst.scale = new_scale;
        }
    }
}
