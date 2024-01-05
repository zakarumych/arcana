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

#[repr(C)]
struct UserData {
    entity: Option<EntityId>,
    unused: u64,
}

impl UserData {
    fn new(entity: impl Entity) -> Self {
        UserData {
            entity: Some(entity.id()),
            unused: 0,
        }
    }

    fn bits(&self) -> u128 {
        self.entity.map_or(0, |e| e.bits()) as u128
    }

    fn from_bits(bits: u128) -> Self {
        UserData {
            entity: EntityId::from_bits(bits as u64),
            unused: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionState {
    Started,
    Stopped,
}

/// Payload of the collistion event.
/// Contains collider index, other body entity id and other collider index.
#[derive(Debug)]
pub struct Collision {
    /// State of the collision event.
    pub state: CollisionState,

    /// Body to which this collider belongs if any.
    pub body: Option<EntityId>,

    /// Other collider entity id.
    /// None if collider entity was despawned.
    pub other: Option<EntityId>,

    /// Other body entity id if any.
    pub other_body: Option<EntityId>,
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

use arcana::{Entity, EntityId};
#[cfg(feature = "dim2")]
pub use dim2::{
    dynamics as dynamics2, geometry as geometry2, pipeline as pipeline2, Collider as Collider2,
    CollisionEvents as CollisionEvents2, PhysicsResource as PhysicsResource2,
    RigidBody as RigidBody2,
};

#[cfg(feature = "dim3")]
pub use dim3::{
    dynamics as dynamics3, geometry as geometry3, pipeline as pipeline3, Collider as Collider3,
    CollisionEvents as CollisionEvents3, PhysicsResource as PhysicsResource3,
    RigidBody as RigidBody3,
};
