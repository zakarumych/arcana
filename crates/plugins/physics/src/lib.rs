#[cfg(feature = "dim2")]
pub mod dim2 {
    use rapier2d as rapier;
    use scene::Global2 as Global;

    macro_rules! with_dim2 {
        ($($tt:tt)*) => { $($tt)* };
    }

    macro_rules! with_dim3 {
        ($($tt:tt)*) => {};
    }

    std::include!("impl.rs");
}

#[cfg(feature = "dim3")]
pub mod dim3 {
    use rapier3d as rapier;
    use scene::Global3 as Global;

    macro_rules! with_dim2 {
        ($($tt:tt)*) => {};
    }

    macro_rules! with_dim3 {
        ($($tt:tt)*) => { $($tt)* };
    }

    std::include!("impl.rs");
}

#[cfg(all(feature = "dim2", not(feature = "dim3")))]
arcana::export_arcana_plugin! {
    PhysicsPlugin {
        dependencies: [scene ...],
        resources: [dim2::PhysicsResource::new()],
        components: [dim2::RigidBody],
        systems: [physics_system_2d: dim2::make_physics_system()],
    }
}

#[cfg(all(feature = "dim3", not(feature = "dim2")))]
arcana::export_arcana_plugin! {
    PhysicsPlugin {
        dependencies: [scene ...],
        resources: [dim3::PhysicsResource::new()],
        components: [dim3::RigidBody],
        systems: [physics_system_3d: dim3::make_physics_system()],
    }
}

#[cfg(all(feature = "dim2", feature = "dim3"))]
arcana::export_arcana_plugin! {
    PhysicsPlugin {
        dependencies: [scene ...],
        resources: [dim2::PhysicsResource::new(), dim3::PhysicsResource::new()],
        components: [dim2::RigidBody, dim3::RigidBody],
        systems: [physics_system_2d: dim2::make_physics_system(), physics_system_3d: dim3::make_physics_system()],
    }
}

#[cfg(feature = "dim2")]
pub use dim2::{
    Collider as Collider2, PhysicsResource as PhysicsResource2, RigidBody as RigidBody2,
};

#[cfg(feature = "dim3")]
pub use dim3::{
    Collider as Collider3, PhysicsResource as PhysicsResource3, RigidBody as RigidBody3,
};
