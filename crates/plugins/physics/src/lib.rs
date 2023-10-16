use arcana::{
    edict::{self, Component, ResMut, Scheduler, View, World},
    plugin::ArcanaPlugin,
};

arcana::feature_ed! {
    use arcana::project::Dependency;
}

use rapier2d::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, MultibodyJointSet, RigidBodySet,
    },
    geometry::{BroadPhase, ColliderSet, NarrowPhase},
    pipeline::PhysicsPipeline,
    prelude::IslandManager,
};
use scene::Global2;

arcana::export_arcana_plugin!(PhysicsPlugin);

pub struct PhysicsPlugin;

impl ArcanaPlugin for PhysicsPlugin {
    fn name(&self) -> &'static str {
        "physics"
    }

    fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
        let res = PhysicsResource {
            pipeline: PhysicsPipeline::new(),
            parameters: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
        };
        world.insert_resource(res);
        scheduler.add_system(physics_system);
    }

    arcana::feature_ed! {
        fn dependencies(&self) -> Vec<(&'static dyn ArcanaPlugin, Dependency)> {
            vec![scene::path_dependency()]
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
#[edict(name = "Body")]
pub struct Body {
    handle: rapier2d::dynamics::RigidBodyHandle,
}

pub struct PhysicsResource {
    pipeline: PhysicsPipeline,
    parameters: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
}

fn physics_system(mut res: ResMut<PhysicsResource>, bodies: View<(&Body, &mut Global2)>) {
    for (body, global) in bodies.iter() {
        let rb = res.bodies.get_mut(body.handle).unwrap();
        rb.set_position(global.iso, true);
    }

    let res = &mut *res;
    res.pipeline.step(
        &na::Vector2::new(0.0, -9.81),
        &res.parameters,
        &mut res.islands,
        &mut res.broad_phase,
        &mut res.narrow_phase,
        &mut res.bodies,
        &mut res.colliders,
        &mut res.impulse_joints,
        &mut res.multibody_joints,
        &mut res.ccd_solver,
        None,
        &(),
        &(),
    );

    for (body, global) in bodies.iter() {
        let rb = res.bodies.get(body.handle).unwrap();
        global.iso = *rb.position();
    }
}
