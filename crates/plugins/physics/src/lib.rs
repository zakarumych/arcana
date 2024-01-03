use std::{
    collections::VecDeque,
    task::{Context, Poll, Waker},
};

use amity::flip_queue::FlipQueue;
use arcana::{
    edict::{self, action::LocalActionEncoder, Component, EntityId, ResMut, State, View},
    flow::FlowEntity,
    Entities, Entity, Modified, World,
};

use rapier2d::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
        RigidBody, RigidBodySet,
    },
    geometry::{
        BroadPhase, Collider, ColliderHandle, ColliderSet, CollisionEvent, CollisionEventFlags,
        ContactPair, NarrowPhase,
    },
    pipeline::PhysicsPipeline,
};
use scene::Global2;

pub use rapier2d::{dynamics, geometry, pipeline};

arcana::export_arcana_plugin! {
    PhysicsPlugin {
        dependencies: [scene ...],
        resources: [PhysicsResource::new()],
        components: [Body],
        systems: [physics_system],
    }
}

/// Component that represents a physics body.
/// Use `PhysicsResource` to create bodies.
/// Use `PhysicsResource::add_collider` to add colliders to bodies.
#[derive(Debug, Component)]
#[edict(name = "Body")]
#[edict(on_drop = remove_body)]
pub struct Body {
    handle: rapier2d::dynamics::RigidBodyHandle,
}

/// Remove body from physics world.
fn remove_body(body: &Body, _: EntityId, mut encoder: LocalActionEncoder) {
    let body = body.handle;
    encoder.closure(move |world: &mut World| {
        let ref mut res = *world.expect_resource_mut::<PhysicsResource>();
        res.bodies.remove(
            body,
            &mut res.islands,
            &mut res.colliders,
            &mut res.impulse_joints,
            &mut res.multibody_joints,
            true,
        );
    });
}

/// Payload of the collistion event.
/// Contains collider index, other body entity id and other collider index.
#[derive(Debug)]
pub struct Collision {
    pub state: CollisionState,
    pub collider: ColliderHandle,
    pub other_entity: Option<EntityId>,
    pub other_collider: ColliderHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionState {
    Started,
    Stopped,
}

#[derive(Debug, Component)]
#[edict(name = "CollisionEvents")]
pub struct CollisionEvents {
    queue: VecDeque<Collision>,
    waker: Option<Waker>,
}

impl Drop for CollisionEvents {
    fn drop(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl CollisionEvents {
    pub fn new() -> Self {
        CollisionEvents {
            queue: VecDeque::new(),
            waker: None,
        }
    }

    pub fn enque(&mut self, collision: Collision) {
        self.queue.push_back(collision);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    pub fn deque(&mut self) -> Option<Collision> {
        self.queue.pop_front()
    }

    pub fn poll_deque(&mut self, cx: &mut Context) -> Poll<Collision> {
        if let Some(collision) = self.queue.pop_front() {
            Poll::Ready(collision)
        } else {
            self.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    pub async fn async_deque_from(entity: &mut FlowEntity<'_>) -> Collision {
        entity
            .expect_poll_view::<&mut CollisionEvents, _, _>(|events, cx| events.poll_deque(cx))
            .await
    }
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

impl PhysicsResource {
    fn new() -> Self {
        PhysicsResource {
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
        }
    }

    pub fn new_fixed_body(&mut self) -> Body {
        Body {
            handle: self.bodies.insert(
                rapier2d::dynamics::RigidBodyBuilder::fixed()
                    .translation(na::Vector2::zeros())
                    .build(),
            ),
        }
    }

    pub fn new_position_body(&mut self) -> Body {
        Body {
            handle: self.bodies.insert(
                rapier2d::dynamics::RigidBodyBuilder::kinematic_position_based()
                    .translation(na::Vector2::zeros())
                    .build(),
            ),
        }
    }

    pub fn new_velocity_body(&mut self) -> Body {
        Body {
            handle: self.bodies.insert(
                rapier2d::dynamics::RigidBodyBuilder::kinematic_velocity_based()
                    .translation(na::Vector2::zeros())
                    .build(),
            ),
        }
    }

    pub fn new_dynamic_body(&mut self) -> Body {
        Body {
            handle: self.bodies.insert(
                rapier2d::dynamics::RigidBodyBuilder::dynamic()
                    .translation(na::Vector2::zeros())
                    .build(),
            ),
        }
    }

    pub fn add_collider(&mut self, body: &Body, collider: impl Into<Collider>) -> ColliderHandle {
        self.colliders
            .insert_with_parent(collider, body.handle, &mut self.bodies)
    }

    pub fn get_body(&self, body: &Body) -> &RigidBody {
        self.bodies.get(body.handle).unwrap()
    }

    pub fn get_body_mut(&mut self, body: &Body) -> &mut RigidBody {
        self.bodies.get_mut(body.handle).unwrap()
    }
}

#[derive(Default)]
pub struct PhysicsState {
    new_events: FlipQueue<CollisionEvent>,
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

    fn set(&self, rb: &mut RigidBody) {
        rb.user_data = self.entity.map_or(0, |e| e.bits()) as u128;
    }

    fn get(rb: &RigidBody) -> Self {
        UserData {
            entity: EntityId::from_bits(rb.user_data as u64),
            unused: 0,
        }
    }
}

fn physics_system(
    mut res: ResMut<PhysicsResource>,
    new_bodies: View<(Entities, Modified<&Body>)>,
    mut bodies: View<(&Body, &mut Global2)>,
    mut events: View<&mut CollisionEvents>,
    mut state: State<PhysicsState>,
) {
    for (e, body) in new_bodies {
        let rb = res.bodies.get_mut(body.handle).unwrap();
        UserData::new(e).set(rb);
    }

    for (body, global) in bodies.iter_mut() {
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
        &EventHandler {
            new_events: &state.new_events,
        },
    );

    for (body, global) in bodies.iter_mut() {
        let rb = res.bodies.get_mut(body.handle).unwrap();
        global.iso = *rb.position();
        rb.reset_forces(false);
    }

    for collision in state.new_events.drain() {
        let (ch1, ch2, state) = match collision {
            CollisionEvent::Started(ch1, ch2, _cf) => (ch1, ch2, CollisionState::Started),
            CollisionEvent::Stopped(ch1, ch2, _cf) => (ch1, ch2, CollisionState::Stopped),
        };

        let c1 = res.colliders.get(ch1);
        let c2 = res.colliders.get(ch2);

        let e1 = c1
            .and_then(|c| c.parent())
            .and_then(|b| res.bodies.get(b))
            .and_then(|b| UserData::get(b).entity);

        let e2 = c2
            .and_then(|c| c.parent())
            .and_then(|b| res.bodies.get(b))
            .and_then(|b| UserData::get(b).entity);

        let mut emit = |e1, ch1, e2, ch2| {
            if let Ok(events) = events.try_get_mut(e1) {
                events.enque(Collision {
                    state,
                    collider: ch1,
                    other_entity: e2,
                    other_collider: ch2,
                });
            }
        };

        if let Some(e1) = e1 {
            emit(e1, ch1, e2, ch2);
        }
        if let Some(e2) = e2 {
            emit(e2, ch2, e1, ch1);
        }
    }
}

struct EventHandler<'a> {
    new_events: &'a FlipQueue<CollisionEvent>,
}

impl<'a> rapier2d::pipeline::EventHandler for EventHandler<'a> {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: CollisionEvent,
        _contact_pair: Option<&ContactPair>,
    ) {
        self.new_events.push(event);
    }

    fn handle_contact_force_event(
        &self,
        _dt: f32,
        bodies: &RigidBodySet,
        colliders: &ColliderSet,
        contact_pair: &ContactPair,
        _total_force_magnitude: f32,
    ) {
    }
}
