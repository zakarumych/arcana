use std::{
    collections::VecDeque,
    task::{Context, Poll, Waker},
};

use amity::flip_queue::FlipQueue;
use arcana::{
    edict::{self, action::LocalActionEncoder, Component, EntityId, ResMut, State, View},
    flow::FlowEntity,
    ActionEncoder, Entities, Modified, With, World,
};

use rapier::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
        RigidBodyHandle, RigidBodySet, RigidBodyType,
    },
    geometry::{
        BroadPhase, ColliderBuilder, ColliderHandle, ColliderSet, CollisionEvent, ContactPair,
        NarrowPhase, Shape,
    },
    math::{Isometry, Point, Vector},
    pipeline::{ActiveEvents, PhysicsPipeline, QueryFilter, QueryPipeline},
};

pub use rapier::{dynamics, geometry, pipeline};

use super::{Collision, CollisionState, UserData};

#[derive(Clone, Component)]
#[edict(name = "Collider")]
#[edict(on_drop = Collider::on_drop)]
pub struct Collider {
    builder: ColliderBuilder,
    handle: Option<ColliderHandle>,
    body: Option<EntityId>,
}

impl Collider {
    pub fn new(shape: geometry::SharedShape) -> Self {
        Collider {
            builder: ColliderBuilder::new(shape),
            handle: None,
            body: None,
        }
    }

    pub fn ball(radius: f32) -> Self {
        Collider::new(geometry::SharedShape::ball(radius))
    }

    pub fn halfspace(outward_normal: na::Unit<Vector<f32>>) -> Self {
        Collider::new(geometry::SharedShape::halfspace(outward_normal))
    }

    with_dim2! {
        pub fn cuboid(hx: f32, hy: f32) -> Self {
            Collider::new(geometry::SharedShape::cuboid(hx, hy))
        }

        pub fn round_cuboid(hx: f32, hy: f32, border_radius: f32) -> Self {
            Collider::new(geometry::SharedShape::round_cuboid(hx, hy, border_radius))
        }
    }

    with_dim3! {
        pub fn cuboid(hx: f32, hy: f32, hz: f32) -> Self {
            Collider::new(geometry::SharedShape::cuboid(hx, hy, hz))
        }

        pub fn round_cuboid(hx: f32, hy: f32, hz: f32, border_radius: f32) -> Self {
            Collider::new(geometry::SharedShape::round_cuboid(hx, hy, hz, border_radius))
        }
    }

    pub fn capsule_x(half_height: f32, radius: f32) -> Self {
        Collider::new(geometry::SharedShape::capsule_x(half_height, radius))
    }

    pub fn capsule_y(half_height: f32, radius: f32) -> Self {
        Collider::new(geometry::SharedShape::capsule_y(half_height, radius))
    }

    with_dim3! {
        pub fn capsule_z(half_height: f32, radius: f32) -> Self {
            Collider::new(geometry::SharedShape::capsule_z(half_height, radius))
        }
    }

    pub fn segment(a: Point<f32>, b: Point<f32>) -> Self {
        Collider::new(geometry::SharedShape::segment(a, b))
    }

    pub fn triangle(a: Point<f32>, b: Point<f32>, c: Point<f32>) -> Self {
        Collider::new(geometry::SharedShape::triangle(a, b, c))
    }

    pub fn sensor(self, is_sensor: bool) -> Self {
        Collider {
            builder: self.builder.sensor(is_sensor),
            handle: self.handle,
            body: self.body,
        }
    }

    pub fn position(self, pos: Isometry<f32>) -> Self {
        Collider {
            builder: self.builder.position(pos),
            handle: self.handle,
            body: self.body,
        }
    }

    pub fn active_events(self, active_events: ActiveEvents) -> Self {
        Collider {
            builder: self.builder.active_events(active_events),
            handle: self.handle,
            body: self.body,
        }
    }

    fn on_drop(&self, _: EntityId, mut encoder: LocalActionEncoder) {
        if let Some(collider) = self.handle {
            encoder.closure(move |world: &mut World| {
                let ref mut res = *world.expect_resource_mut::<PhysicsResource>();
                res.colliders
                    .remove(collider, &mut res.islands, &mut res.bodies, true);
            });
        }
    }
}

/// Initializes newly added or modified colliders.
fn init_colliders(
    mut res: ResMut<PhysicsResource>,
    modified_colliders: View<(Entities, Modified<&mut Collider>)>,
    mut bodies: View<&mut RigidBody>,
    mut encoder: ActionEncoder,
) {
    let res = &mut *res;

    // Update colliders.
    // Set user data and attach to parent body.
    for (e, collider) in modified_colliders {
        let body = match collider.body {
            None => match bodies.try_get_mut(e) {
                Err(_) => None,
                Ok(body) => Some(body),
            },
            Some(body) => match bodies.try_get_mut(body) {
                Err(_) => {
                    encoder.despawn(e);
                    continue;
                }
                Ok(body) => Some(body),
            },
        };

        match collider.handle.and_then(|handle| res.colliders.get(handle)) {
            None => {
                // No handle or outdated.

                let mut col = collider.builder.build();
                col.user_data = UserData::new(e).bits();

                if let Some(body) = body {
                    let handle = res.colliders.insert_with_parent(
                        col,
                        body.handle.unwrap(),
                        &mut res.bodies,
                    );
                    if let Some(rb) = res.bodies.get_mut(body.handle.unwrap()) {
                        rb.recompute_mass_properties_from_colliders(&res.colliders);
                        body.mass = rb.mass();
                    }
                    collider.handle = Some(handle);
                } else {
                    let handle = res.colliders.insert(col);
                    collider.handle = Some(handle);
                }
            }
            Some(col) => {
                match body {
                    None => {
                        if let Some(old_parent) = col.parent() {
                            res.colliders.set_parent(
                                collider.handle.unwrap(),
                                None,
                                &mut res.bodies,
                            );

                            if let Some(rb) = res.bodies.get_mut(old_parent) {
                                if let Some(body) = UserData::from_bits(rb.user_data).entity {
                                    if let Ok(body) = bodies.try_get_mut(body) {
                                        rb.recompute_mass_properties_from_colliders(&res.colliders);
                                        body.mass = rb.mass();
                                    }
                                }
                            }
                        }
                    }
                    Some(body) => {
                        if col.parent() != Some(body.handle.unwrap()) {
                            res.colliders.set_parent(
                                collider.handle.unwrap(),
                                Some(body.handle.unwrap()),
                                &mut res.bodies,
                            );
                            if let Some(rb) = res.bodies.get_mut(body.handle.unwrap()) {
                                rb.recompute_mass_properties_from_colliders(&res.colliders);
                                body.mass = rb.mass();
                            }
                        }
                    }
                };
            }
        }
    }
}

/// Component that represents a physics body.
/// Use `PhysicsResource` to create bodies.
/// Use `PhysicsResource::add_collider` to add colliders to bodies.
#[derive(Debug, Component)]
#[edict(name = "RigidBody")]
#[edict(on_drop = RigidBody::on_drop)]
pub struct RigidBody {
    body_type: RigidBodyType,

    /// Rigid body setup.
    builder: dynamics::RigidBodyBuilder,

    /// Handle to the body in the physics world.
    handle: Option<RigidBodyHandle>,

    mass: f32,

    /// Total force applied to the body.
    force: Vector<f32>,

    /// Total impulse applied to the body.
    impulse: Vector<f32>,
}

impl RigidBody {
    pub fn new(body_type: RigidBodyType) -> Self {
        let builder = dynamics::RigidBodyBuilder::new(body_type);

        RigidBody {
            body_type,
            builder,
            handle: None,
            mass: 0.0,
            force: Vector::zeros(),
            impulse: Vector::zeros(),
        }
    }

    pub fn fixed() -> Self {
        RigidBody::new(RigidBodyType::Fixed)
    }

    pub fn kinematic_position_based() -> Self {
        RigidBody::new(RigidBodyType::KinematicPositionBased)
    }

    pub fn kinematic_velocity_based() -> Self {
        RigidBody::new(RigidBodyType::KinematicVelocityBased)
    }

    pub fn dynamic() -> Self {
        RigidBody::new(RigidBodyType::Dynamic)
    }

    pub fn position(mut self, pos: Isometry<f32>) -> Self {
        self.builder.position = pos;
        self
    }

    pub fn body_type(&self) -> RigidBodyType {
        self.body_type
    }

    pub fn is_fixed(&self) -> bool {
        self.body_type == RigidBodyType::Fixed
    }

    pub fn is_kinematic(&self) -> bool {
        self.body_type == RigidBodyType::KinematicPositionBased
            || self.body_type == RigidBodyType::KinematicVelocityBased
    }

    pub fn is_kinematic_position_based(&self) -> bool {
        self.body_type == RigidBodyType::KinematicPositionBased
    }

    pub fn is_kinematic_velocity_based(&self) -> bool {
        self.body_type == RigidBodyType::KinematicVelocityBased
    }

    pub fn is_dynamic(&self) -> bool {
        self.body_type == RigidBodyType::Dynamic
    }

    fn on_drop(&self, entity: EntityId, mut encoder: LocalActionEncoder) {
        if let Some(body) = self.handle {
            encoder.closure(move |world: &mut World| {
                let world = world.local();

                let ref mut res = *world.expect_resource_mut::<PhysicsResource>();

                if let Some(rb) = res.bodies.get(body) {
                    for &c in rb.colliders() {
                        if let Some(col) = res.colliders.get(c) {
                            if let Some(e) = UserData::from_bits(col.user_data).entity {
                                if entity != e {
                                    world.despawn_defer(e);
                                } else {
                                    world.drop_defer::<Collider>(entity);
                                }
                            }
                        }
                    }
                    res.bodies.remove(
                        body,
                        &mut res.islands,
                        &mut res.colliders,
                        &mut res.impulse_joints,
                        &mut res.multibody_joints,
                        true,
                    );
                }
            });
        }
    }

    /// Returns mass of the body.
    pub fn mass(&self) -> f32 {
        self.mass
    }

    pub fn apply_force(&mut self, force: Vector<f32>) {
        self.force += force;
        self.force;
    }

    pub fn apply_impulse(&mut self, impulse: Vector<f32>) {
        self.impulse += impulse;
    }
}

/// This component helps iterating only over kinematic bodies to update position from [`Global`].
struct KinematicPositionBased;
impl Component for KinematicPositionBased {}

/// Initializes newly added rigid bodies.
/// Inserts or updates body and saves handle to it.
fn init_bodies(
    mut res: ResMut<PhysicsResource>,
    modified_bodies: View<(
        Entities,
        Modified<&mut RigidBody>,
        Option<With<KinematicPositionBased>>,
    )>,
    mut encoder: ActionEncoder,
) {
    // Update bodies.
    // Set user data and kinematic state.
    // This is cold path as it only touches bodies that were modified (including newly inserted).
    for (e, body, kinematic_position_based) in modified_bodies {
        match body.handle.and_then(|handle| res.bodies.get_mut(handle)) {
            None => {
                // No handle or outdated.
                let mut rb = body.builder.build();

                rb.user_data = UserData::new(e).bits();

                rb.add_force(body.force, false);
                rb.apply_impulse(body.impulse, false);
                body.force = Vector::zeros();
                body.impulse = Vector::zeros();

                let is_kinematic_position_based =
                    rb.body_type() == RigidBodyType::KinematicPositionBased;

                // Set/unser kinematic flag component.
                if kinematic_position_based.is_some() != is_kinematic_position_based {
                    if !is_kinematic_position_based {
                        encoder.drop::<KinematicPositionBased>(e);
                    }
                } else {
                    if is_kinematic_position_based {
                        encoder.insert(e, KinematicPositionBased);
                    }
                }

                body.mass = rb.mass();
                let handle = res.bodies.insert(rb);
                body.handle = Some(handle);
            }
            Some(rb) => {
                rb.add_force(body.force, true);
                rb.apply_impulse(body.impulse, true);
                body.force = Vector::zeros();
                body.impulse = Vector::zeros();
            }
        }
    }
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
    query_pipeline: QueryPipeline,
}

impl PhysicsResource {
    pub(crate) fn new() -> Self {
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
            query_pipeline: QueryPipeline::new(),
        }
    }

    pub fn intersections_with_shape(
        &self,
        pos: &Isometry<f32>,
        shape: &impl Shape,
        mut f: impl FnMut(EntityId, Option<EntityId>),
    ) {
        self.query_pipeline.intersections_with_shape(
            &self.bodies,
            &self.colliders,
            pos,
            shape,
            QueryFilter::default(),
            |collider| {
                if let Some(col) = self.colliders.get(collider) {
                    if let Some(collider) = UserData::from_bits(col.user_data).entity {
                        let body = col
                            .parent()
                            .and_then(|b| self.bodies.get(b))
                            .and_then(|b| UserData::from_bits(b.user_data).entity);

                        f(collider, body);
                    }
                }
                true
            },
        )
    }
}

#[derive(Default)]
pub struct PhysicsState {
    new_events: FlipQueue<CollisionEvent>,
}

fn update_kinematic(
    mut res: ResMut<PhysicsResource>,
    kinematic_bodies: View<(&RigidBody, Modified<&Global>), With<KinematicPositionBased>>,
) {
    for (body, global) in kinematic_bodies {
        let rb = res.bodies.get_mut(body.handle.unwrap()).unwrap();
        rb.set_position(global.iso, true);
    }
}

fn run_simulation(
    mut res: ResMut<PhysicsResource>,
    mut events: View<&mut CollisionEvents>,
    mut state: State<PhysicsState>,
) {
    let res = &mut *res;

    let mut gravity: Vector<f32> = Vector::zeros();
    gravity.y = -9.81;
    res.pipeline.step(
        &gravity,
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

    for collision in state.new_events.drain() {
        let (ch1, ch2, state) = match collision {
            CollisionEvent::Started(ch1, ch2, _cf) => (ch1, ch2, CollisionState::Started),
            CollisionEvent::Stopped(ch1, ch2, _cf) => (ch1, ch2, CollisionState::Stopped),
        };

        let c1 = res.colliders.get(ch1);
        let c2 = res.colliders.get(ch2);

        dbg!(c1.is_some(), c2.is_some());

        let b1 = c1.and_then(|c| c.parent()).and_then(|b| res.bodies.get(b));
        let b2 = c2.and_then(|c| c.parent()).and_then(|b| res.bodies.get(b));

        let c1 = c1.and_then(|c| UserData::from_bits(c.user_data).entity);
        let c2 = c2.and_then(|c| UserData::from_bits(c.user_data).entity);

        let b1 = b1.and_then(|b| UserData::from_bits(b.user_data).entity);
        let b2 = b2.and_then(|b| UserData::from_bits(b.user_data).entity);

        let mut emit = |c1, b1, c2, b2| {
            if let Ok(events) = events.try_get_mut(c1) {
                events.enque(Collision {
                    state,
                    body: b1,
                    other: c2,
                    other_body: b2,
                });
            }
        };

        if let Some(c1) = c1 {
            emit(c1, b1, c2, b2);
        }
        if let Some(c2) = c2 {
            emit(c2, b2, c1, b1);
        }
    }

    res.query_pipeline.update(&res.bodies, &res.colliders);
}

fn update_active(mut res: ResMut<PhysicsResource>, mut dynamic_bodies: View<&mut Global>) {
    let res = &mut *res;

    // Update position of active dynamic bodies.
    for &body in res.islands.active_dynamic_bodies() {
        let rb = res.bodies.get_mut(body).unwrap();
        if let Some(entity) = UserData::from_bits(rb.user_data).entity {
            if let Ok(global) = dynamic_bodies.try_get_mut(entity) {
                global.iso = *rb.position();
            }
        }
    }

    // Update position of active kinematic bodies.
    for &body in res.islands.active_kinematic_bodies() {
        let rb = res.bodies.get_mut(body).unwrap();
        if let Some(entity) = UserData::from_bits(rb.user_data).entity {
            if let Ok(global) = dynamic_bodies.try_get_mut(entity) {
                global.iso = *rb.position();
            }
        }
    }
}

// fn update_active(
//     mut res: ResMut<PhysicsResource>,
//     dynamic_bodies: View<(&RigidBody, &mut Global)>,
// ) {
//     let res = &mut *res;

//     // Update position of active dynamic bodies.
//     for (body, global) in dynamic_bodies {
//         let rb = res.bodies.get_mut(body.handle.unwrap()).unwrap();
//         global.iso = *rb.position();
//     }
// }

struct EventHandler<'a> {
    new_events: &'a FlipQueue<CollisionEvent>,
}

impl<'a> rapier::pipeline::EventHandler for EventHandler<'a> {
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
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _contact_pair: &ContactPair,
        _total_force_magnitude: f32,
    ) {
    }
}

pub(crate) fn make_physics_system() -> impl arcana::System {
    use arcana::IntoSystem;

    (
        init_bodies.into_system(),
        init_colliders.into_system(),
        update_kinematic.into_system(),
        run_simulation.into_system(),
        update_active.into_system(),
    )
        .into_system()
}
