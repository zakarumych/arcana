use arcana::{
    edict::{self, Component, ResMut, Scheduler, View, World},
    plugin::ArcanaPlugin,
};

use rapier2d::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, MultibodyJointSet, RigidBodyHandle,
        RigidBodySet,
    },
    geometry::{BroadPhase, ColliderHandle, ColliderSet, NarrowPhase},
    pipeline::PhysicsPipeline,
};

arcana::export_arcana_plugin!(PhysicsPlugin);

pub struct PhysicsPlugin;

#[derive(Component, Clone, Copy, Debug)]
#[edict(name = "Body")]
#[edict(on_set = on_body_set)]
pub struct Body {
    handle: rapier2d::dynamics::RigidBodyHandle,
}

struct PhysicsResource {
    pipeline: PhysicsPipeline,
    parameters: IntegrationParameters,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccds: CCDSolver,
}

fn physics_system(mut res: ResMut<PhysicsResource>, bodies: View<&Body>) {}

impl ArcanaPlugin for PhysicsPlugin {
    fn name(&self) -> &'static str {
        "Physics"
    }

    fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
        let res = PhysicsResource {
            pipeline: PhysicsPipeline::new(),
            parameters: IntegrationParameters::default(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccds: CCDSolver::new(),
        };
        world.insert_resource(res);
        scheduler.add_system(physics_system);
    }
}
