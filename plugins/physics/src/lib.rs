#[cfg(feature = "dim2")]
pub mod dim2 {
    use rapier2d as rapier;
    use scene::dim2::Global;

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
    use scene::dim3::Global;

    macro_rules! with_dim2 {
        ($($tt:tt)*) => {};
    }

    macro_rules! with_dim3 {
        ($($tt:tt)*) => { $($tt)* };
    }

    std::include!("impl.rs");
}

#[repr(C)]
struct UserData {
    entity: Option<arcana::EntityId>,
    id: u64,
}

impl UserData {
    fn new(entity: impl arcana::Entity, id: u64) -> Self {
        UserData {
            entity: Some(entity.id()),
            id,
        }
    }

    fn bits(&self) -> u128 {
        ((self.id as u128) << 64) | self.entity.map_or(0, |e| e.bits()) as u128
    }

    fn from_bits(bits: u128) -> Self {
        UserData {
            entity: arcana::EntityId::from_bits(bits as u64),
            id: (bits >> 64) as u64,
        }
    }
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
