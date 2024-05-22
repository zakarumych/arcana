use arcana::{
    edict::{self, spawn_block, ActionEncoder, Component, Entities, Res, View, World},
    flow::sleep,
    gametime::{timespan, TimeSpan},
    na,
    render::RenderGraph,
    viewport::Viewport,
    ClockStep,
};
use camera::Camera2;
use cursor::MainCursor;
use motion::dim2::{Motion, Motor, MoveAfter, MoveTo};
use physics::dim2::{Collider, ContactForceEvents, FlowEntityExt, PhysicsResource, RigidBody};
use scene::dim2::Global;
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
                mut motion: View<&mut Motion>,
                cameras: View<(&Camera2, &Global)>| {
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
                    *motion.try_get_mut(target).unwrap() = MoveTo::new(position).into();
                },
            burst_system,
        ],

        in world => {
            let camera = world
                .spawn((Global::identity(), Camera2::new().with_fovy(15.0)))
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
                    Global::identity(),
                    RigidBody::dynamic(),
                    Collider::ball(1.0),
                    Motion::to(na::Point2::new(0.0, 0.0)),
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

                let global = Global::from_position(na::Point2::new(
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
                        Collider::ball(1.0).enable_contact_force_events().contact_force_event_threshold(2000.0),
                        Motion::After(MoveAfter::new(last_ball).with_distance(2.0)),
                        Motor::new(10.0, 100.0),
                        ContactForceEvents::new(),
                        BallComponent,
                    )).unwrap();

                last_ball = id;

                spawn_block!(in ref world for last_ball -> {
                    last_ball.next_contact_force_event().await;
                    let _ = last_ball.insert(Burst { span: TimeSpan::ZERO, scale: 1.0, color: [0.0, 0.0, 0.0] });
                });
            };

            spawn_block!(in ref world -> {
                sleep(timespan!(2 s), world).await;
                loop {
                    new_node(&mut world);
                    sleep(timespan!(1 s), world).await;
                }
            });
        }
    }
}

#[derive(Component)]
struct Burst {
    span: TimeSpan,
    scale: f32,
    color: [f32; 3],
}

fn burst_system(
    burst: View<(Entities, &mut Burst, &mut sdf::Shape, &Global)>,
    mut bodies: View<(&mut RigidBody, &Global)>,
    clock: Res<ClockStep>,
    mut encoder: ActionEncoder,
    physics: Res<PhysicsResource>,
) {
    for (e, burst, shape, global) in burst {
        if burst.span == TimeSpan::ZERO {
            let [r, g, b, _] = shape.color;
            burst.color = [r, g, b];
        }

        burst.span += clock.step;
        if burst.span >= TimeSpan::SECOND * 3 {
            encoder.despawn(e);

            physics.intersections_with_shape(
                &global.iso,
                &physics::dim2::Ball::new(30.0),
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
            let x = (1.0 / (3.001 - burst.span.as_secs_f32())).sin();
            if x.fract() < 0.1 {
                shape.color = [1.0, 1.0, 1.0, 1.0];
            } else {
                let [r, g, b] = burst.color;
                shape.color = [r, g, b, 1.0];
            }

            let new_scale = 2f32.powf(burst.span.as_secs_f32() / 3.0);
            shape.transform *= na::Similarity2::from_scaling(new_scale / burst.scale);
            burst.scale = new_scale;
        }
    }
}
