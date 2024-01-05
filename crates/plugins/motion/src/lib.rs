#[cfg(feature = "dim2")]
pub mod dim2 {
    use na::Point2 as Point;
    use na::Vector2 as Vector;
    use physics::dim2::{dynamics::RigidBodyType, RigidBody};
    use scene::Global2 as Global;

    std::include!("impl.rs");
}

#[cfg(feature = "dim3")]
pub mod dim3 {
    use na::Point3 as Point;
    use na::Vector3 as Vector;
    use physics::dim3::{dynamics::RigidBodyType, RigidBody};
    use scene::Global3 as Global;

    std::include!("impl.rs");
}

#[cfg(all(feature = "dim2", not(feature = "dim3")))]
arcana::export_arcana_plugin! {
    MotionPlugin {
        dependencies: [scene ..., physics ...],
        components: [dim2::Motor, dim2::Motion],
        systems: [ motion_system_2d: dim2::make_motion_system() ],
    }
}

#[cfg(all(feature = "dim3", not(feature = "dim2")))]
arcana::export_arcana_plugin! {
    MotionPlugin {
        dependencies: [scene ..., physics ...],
        components: [dim3::Motor, dim3::Motion],
        systems: [ motion_system_3d: dim3::make_motion_system() ],
    }
}

#[cfg(all(feature = "dim2", feature = "dim3"))]
arcana::export_arcana_plugin! {
    MotionPlugin {
        dependencies: [scene ..., physics ...],
        components: [dim2::Motor, dim2::Motion, dim3::Motor, dim3::Motion],
        systems: [ motion_system_2d: dim2::make_motion_system() ],
    }
}

#[cfg(feature = "dim2")]
pub use dim2::{Motion as Motion2, Motor as Motor2, MoveAfter as MoveAfter2, MoveTo as MoveTo2};

#[cfg(feature = "dim3")]
pub use dim3::{Motion as Motion3, Motor as Motor3, MoveAfter as MoveAfter3, MoveTo as MoveTo3};
