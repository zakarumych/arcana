use super::*;
use athena::{Vector2, Vector3, Vector4};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
fn close2(a: Vector2<f64>, b: Vector2<f64>) {
    close(a.x, b.x);
    close(a.y, b.y);
}
fn close3(a: Vector3<f64>, b: Vector3<f64>) {
    close(a.x, b.x);
    close(a.y, b.y);
    close(a.z, b.z);
}

#[test]
fn hierarchy_derives_parent_first_and_preserves_standalone() {
    let mut world = World::new();
    let child = world
        .spawn_one(dim2::LocalTransform::new(Vector2::new(2.0, 0.0), 0.0))
        .id();
    let parent = world
        .spawn_one(dim2::Transform::new(
            Vector2::new(10.0, 20.0),
            std::f64::consts::FRAC_PI_2,
        ))
        .id();
    world
        .insert_relation(child, ChildOf, parent)
        .expect("live entities");
    update_transforms(&mut world).expect("valid hierarchy");
    close2(
        world
            .get::<&dim2::Transform>(child)
            .expect("derived")
            .position(),
        Vector2::new(10.0, 22.0),
    );
    close2(
        world
            .get::<&dim2::Transform>(parent)
            .expect("standalone")
            .position(),
        Vector2::new(10.0, 20.0),
    );
    dim2::set_world_transform(
        &mut world,
        child,
        dim2::Transform::new(Vector2::new(10.0, 25.0), 0.0),
    )
    .expect("editable");
    close2(
        world
            .get::<&dim2::LocalTransform>(child)
            .expect("authored")
            .position(),
        Vector2::new(5.0, 0.0),
    );
    update_transforms(&mut world).expect("valid hierarchy");
    close2(
        world
            .get::<&dim2::Transform>(child)
            .expect("derived")
            .position(),
        Vector2::new(10.0, 25.0),
    );
}

#[test]
fn invalid_hierarchy_does_not_partially_update() {
    let mut world = World::new();
    let parent = world.spawn_empty().id();
    let child = world.spawn_one(dim2::LocalTransform::default()).id();
    world
        .insert_relation(child, ChildOf, parent)
        .expect("live entities");
    assert_eq!(
        update_transforms(&mut world),
        Err(TransformError::MissingParentTransform(parent))
    );
    assert!(world.get::<&dim2::Transform>(child).is_err());
    world
        .insert(parent, dim2::LocalTransform::default())
        .expect("live parent");
    world
        .insert_relation(parent, ChildOf, child)
        .expect("live entities");
    assert!(matches!(
        update_transforms(&mut world),
        Err(TransformError::Cycle(_))
    ));
    assert!(world.get::<&dim2::Transform>(child).is_err());
}

#[test]
fn transform_reflection_and_inverse_preserve_geometry() {
    let t = dim2::LocalTransform::new(Vector2::new(4.0, -8.0), 1.4);
    let restored = dim2::LocalTransform::from_value(t.to_value()).expect("roundtrip");
    let point = Vector2::new(3.0, 5.0);
    close2(restored.transform_point(point), t.transform_point(point));
    close2(t.inverse().transform_point(t.transform_point(point)), point);
    for rotation in [
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.3, 0.7, -1.2),
        Vector3::new(3.14, 0.0, 0.0),
    ] {
        let t = dim3::LocalTransform::new(Vector3::new(4.0, -8.0, 2.0), rotation);
        close3(t.position(), Vector3::new(4.0, -8.0, 2.0));
        let restored = dim3::LocalTransform::from_value(t.to_value()).expect("roundtrip");
        let point = Vector3::new(3.0, 5.0, 1.0);
        close3(restored.transform_point(point), t.transform_point(point));
        close3(t.inverse().transform_point(t.transform_point(point)), point);
    }
}

#[test]
fn camera_projection_matches_logical_rectangle_and_clip_depth() {
    let matrix = dim2::Camera::default()
        .matrices(dim2::Transform::default(), 4.0 / 3.0)
        .expect("valid camera")
        .view_projection();
    let origin = matrix * Vector4::new(0.0, 0.0, 0.0, 1.0);
    close(origin.x, -1.0);
    close(origin.y, 1.0);
    let end = matrix * Vector4::new(960.0, 720.0, 0.0, 1.0);
    close(end.x, 1.0);
    close(end.y, -1.0);
    let camera = dim3::Camera::default();
    for perspective in [true, false] {
        let c = dim3::Camera {
            perspective,
            ..camera
        };
        let matrix = c
            .matrices(dim3::Transform::default(), 1.5)
            .expect("valid camera")
            .view_projection();
        for (z, depth) in [(-c.near, 0.0), (-c.far, 1.0)] {
            let p = matrix * Vector4::new(0.0, 0.0, z, 1.0);
            close(p.z / p.w, depth);
        }
    }
    assert!(camera.matrices(dim3::Transform::default(), 0.0).is_none());
}

#[test]
fn registered_defaults_roundtrip() {
    let mut hub = arcana_plugin::PluginsHub::new();
    register_components(&mut hub);
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    for desc in hub.components.values() {
        desc.insert(
            &mut world,
            entity,
            desc.initial_value().expect("built-in default"),
        )
        .expect("default insert");
        let value = desc.read(&mut world, entity).expect("read");
        desc.apply(&mut world, entity, value).expect("roundtrip");
    }
}

#[test]
fn right_handed_axis_rotations_and_three_dimensional_hierarchy() {
    let zero = Vector3::new(0.0, 0.0, 0.0);
    let x = Vector3::new(1.0, 0.0, 0.0);
    let y = Vector3::new(0.0, 1.0, 0.0);
    let z = Vector3::new(0.0, 0.0, 1.0);
    let quarter = std::f64::consts::FRAC_PI_2;
    for (axis, point, expected) in [(x, y, z), (y, z, x), (z, x, y)] {
        let transform = dim3::Transform::new(zero, axis * quarter);
        close3(transform.transform_point(point), expected);
        close3(transform.rotation_vector(), axis * quarter);
    }
    let mut world = World::new();
    let parent = world
        .spawn_one(dim3::Transform::new(
            Vector3::new(4.0, 5.0, 6.0),
            z * quarter,
        ))
        .id();
    let child = world
        .spawn_one(dim3::LocalTransform::new(x * 2.0, zero))
        .id();
    world
        .insert_relation(child, ChildOf, parent)
        .expect("live entities");
    update_transforms(&mut world).expect("valid hierarchy");
    close3(
        world
            .get::<&dim3::Transform>(child)
            .expect("derived")
            .position(),
        Vector3::new(4.0, 7.0, 6.0),
    );
    dim3::set_world_transform(
        &mut world,
        child,
        dim3::Transform::new(Vector3::new(4.0, 9.0, 6.0), zero),
    )
    .expect("valid edit");
    close3(
        world
            .get::<&dim3::LocalTransform>(child)
            .expect("authored")
            .position(),
        x * 4.0,
    );
}

#[test]
fn camera_pose_and_aspect_fit_share_world_coordinates() {
    let pose = dim2::Transform::new(Vector2::new(20.0, -10.0), 0.7);
    let camera = dim2::Camera::default();
    let clip = camera
        .matrices(pose, 2.0)
        .expect("camera")
        .view_projection();
    let center = pose.transform_point(camera.center);
    let p = clip * Vector4::new(center.x, center.y, 0.0, 1.0);
    close(p.x, 0.0);
    close(p.y, 0.0);
    let corner = pose.transform_point(Vector2::new(0.0, 0.0));
    let p = clip * Vector4::new(corner.x, corner.y, 0.0, 1.0);
    close(p.x, -2.0 / 3.0);
    close(p.y, 1.0);
    let pose = dim3::Transform::new(Vector3::new(5.0, 6.0, 7.0), Vector3::new(0.2, 0.7, -0.3));
    let clip = dim3::Camera::default()
        .matrices(pose, 1.5)
        .expect("camera")
        .view_projection();
    let forward = pose.transform_point(Vector3::new(0.0, 0.0, -10.0));
    let p = clip * Vector4::new(forward.x, forward.y, forward.z, 1.0);
    close(p.x / p.w, 0.0);
    close(p.y / p.w, 0.0);
}
