use arcana::{
    edict::{self, spawn_block, yield_now, ActionEncoder, Component, Entities, Res, View, World},
    events::{ElementState, KeyboardInput, VirtualKeyCode},
    flow::sleep,
    gametime::timespan,
    na,
    render::RenderGraph,
    viewport::Viewport,
};
use camera::Camera2;
use cursor::MainCursor;
use input::{ActionQueue, Translator};
use motion::{Motor2, MoveAfter2, MoveTo2};
use physics::{
    geometry::ColliderBuilder, pipeline::ActiveEvents, Collision, CollisionEvents, CollisionState,
    PhysicsResource,
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
                mut move_to: View<&mut MoveTo2>,
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
                    move_to.try_get_mut(target).unwrap().target = position;
                },
            paddle_system,
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

            let body = {
                let mut physics = world.expect_resource_mut::<PhysicsResource>();
                let body = physics.new_dynamic_body();

                physics.add_collider(&body, ColliderBuilder::ball(1.0));
                body
            };
            let mut target = world
                .spawn((
                    sdf::Shape::circle(1.0).with_color([
                        rand::random(),
                        rand::random(),
                        rand::random(),
                        1.0,
                    ]),
                    Global2::identity(),
                    body,
                    MoveTo2::new(na::Point2::new(0.0, 0.0)),
                    Motor2::new(30.0, 100.0),
                    BallComponent,
                ))
                .id();

            // insert_global_entity_controller(PaddleTranslator, paddle, world).unwrap();

            let mut physics = world.expect_resource_mut::<PhysicsResource>();
            // let wall_body = {
            //     let body = physics.new_fixed_body();

            //     physics.add_collider(&body, ColliderBuilder::cuboid(0.2, 5.0));
            //     // physics.get_body_mut(&body).set_angvel(1.0, false);
            //     body
            // };

            let left_side = {
                let body = physics.new_fixed_body();
                physics.add_collider(&body, ColliderBuilder::halfspace(na::UnitVector2::new_unchecked(na::Vector2::x())));
                body
            };

            let right_side = {
                let body = physics.new_fixed_body();
                physics.add_collider(&body, ColliderBuilder::halfspace(na::UnitVector2::new_unchecked(-na::Vector2::x())));
                body
            };

            let top_side = {
                let body = physics.new_fixed_body();
                physics.add_collider(&body, ColliderBuilder::halfspace(na::UnitVector2::new_unchecked(-na::Vector2::y())));
                body
            };

            let bottom_side = {
                let body = physics.new_fixed_body();
                physics.add_collider(&body, ColliderBuilder::halfspace(na::UnitVector2::new_unchecked(na::Vector2::y())));
                body
            };

            drop(physics);


            // world.spawn((
            //     sdf::Shape::rect(0.4, 10.0).with_color([0.3, 0.2, 0.1, 1.0]),
            //     Global2::identity().translated(na::Vector2::new(-10.0, 10.0)),
            //     wall_body,
            // ));
            world.spawn((
                Global2::identity().translated(na::Vector2::new(-15.0, 0.0)),
                left_side,
            ));
            world.spawn((
                Global2::identity().translated(na::Vector2::new(15.0, 0.0)),
                right_side,
            ));
            world.spawn((
                Global2::identity().translated(na::Vector2::new(0.0, 15.0)),
                top_side,
            ));
            world.spawn((
                Global2::identity().translated(na::Vector2::new(0.0, -15.0)),
                bottom_side,
            ));


            let mut new_node = move |world: &mut World| {
                let body = {
                    let mut physics = world.expect_resource_mut::<PhysicsResource>();
                    let body = physics.new_dynamic_body();

                    physics.add_collider(&body, ColliderBuilder::ball(1.0).active_events(ActiveEvents::COLLISION_EVENTS));
                    body
                };
                target = world
                    .spawn((
                        sdf::Shape::circle(1.0).with_color([
                            rand::random(),
                            rand::random(),
                            rand::random(),
                            1.0,
                        ]),
                        Global2::from_position(na::Point2::new(
                            rand::random::<f32>() * 26.0 - 13.0,
                            rand::random::<f32>() * 26.0 - 13.0,
                        )),
                        body,
                        MoveAfter2::new(target).with_distance(2.0),
                        Motor2::new(10.0, 100.0),
                        CollisionEvents::new(),
                        BallComponent,
                    ))
                    .id();

                spawn_block!(in ref world for target -> {
                    loop {
                        let event: Collision = CollisionEvents::async_deque_from(&mut target).await;

                        if event.state == CollisionState::Started {
                            if let Some(other) = event.other_entity {
                                if target.world().try_has_component::<BallComponent>(other).unwrap_or(false) {
                                    // Despawn on any collision.
                                    for _ in 0..100 {
                                        let mut s = target.get_copied::<sdf::Shape>().unwrap();
                                        s.transform *= na::Similarity2::from_scaling(1.01);
                                        target.set(s).unwrap();

                                        sleep(timespan!(0.02 s), &mut target.world()).await;
                                    }
                                    let _ = target.despawn();
                                    yield_now!();
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

#[derive(Clone, Copy, Component)]
struct PaddleState {
    // left: bool,
    // right: bool,
    chasing: MoveAfter2,
}

fn paddle_system(
    paddles: View<(
        Entities,
        &mut PaddleState,
        Option<&MoveAfter2>,
        &mut ActionQueue<PaddleAction>,
    )>,
    mut encoder: ActionEncoder,
) {
    for (e, state, ma, queue) in paddles {
        if let Some(ma) = ma {
            state.chasing = *ma;
        }

        for action in queue.drain() {
            match action {
                // PaddleAction::PaddleLeft => state.left = true,
                // PaddleAction::PaddleRight => state.right = true,
                // PaddleAction::PaddleUnLeft => state.left = false,
                // PaddleAction::PaddleUnRight => state.right = false,
                PaddleAction::Switch => match ma {
                    None => encoder.insert(e, state.chasing),
                    Some(_) => encoder.drop::<MoveAfter2>(e),
                },
            }
        }

        // let target = match (state.left, state.right) {
        //     (true, true) | (false, false) => {
        //         if move_to.get_mut(e).is_some() {
        //             encoder.drop_bundle::<(MoveTo2, Motion2)>(e);
        //         }
        //         continue;
        //     }
        //     (true, false) => na::Point2::new(-15.0, 12.0),
        //     (false, true) => na::Point2::new(15.0, 12.0),
        // };

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
