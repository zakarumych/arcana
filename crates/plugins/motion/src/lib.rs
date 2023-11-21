use std::f32::EPSILON;

use arcana::{
    edict::{
        query::Xor2, ActionEncoder, Component, Entities, EntityId, Res, ResMut, View, Without,
    },
    gametime::ClockStep,
    project::{Dependency, Ident},
};
use physics::{Body, PhysicsResource};
use scene::Global2;

arcana::export_arcana_plugin! {
    MotionPlugin {
        dependencies: [scene ..., physics ...],
        components: [Motor2, Motion2, Motor2State, MoveTo2, MoveAfter2],
        systems: [
            motion_after_init_system,
            move_to_init_system,
            motion_after_system,
            move_to_system,
            motion_system,
        ],
    }
}

pub struct Motor2 {
    /// Cruise velocity for the motor.
    pub velocity: f32,

    /// Maximum acceleration for the motor.
    pub acceleration: f32,

    /// Deceleration threshold.
    pub threshold: f32,
}

impl Component for Motor2 {
    fn name() -> &'static str {
        "Motor2"
    }
}

impl Motor2 {
    pub fn new(velocity: f32, acceleration: f32) -> Self {
        // Find stopping threshold and add 10% to it just in case.
        let threshold = (velocity * velocity / acceleration / 2.0) * 1.1;
        Motor2 {
            velocity,
            acceleration,
            threshold,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    fn initial_state(position: na::Vector2<f32>) -> Motor2State {
        Motor2State {
            prev_position: position,
            // integral: na::Vector2::zeros(),
        }
    }

    /// Calculate required motion for the entity to eventually reach the target.
    fn update(
        &self,
        position: na::Vector2<f32>,
        target: na::Point2<f32>,
        distance: f32,
        state: &mut Motor2State,
        delta_time: f32,
    ) -> na::Vector2<f32> {
        let mut target = target.coords - position;
        let mut target_mag = target.magnitude();

        let target_velocity;

        if target_mag < distance {
            target = na::Vector2::zeros();
            target_mag = 0.0;
        } else {
            target -= target / target_mag * distance;
            target_mag -= distance;
        }

        if target_mag < self.threshold {
            target_velocity = self.velocity * target / self.threshold;
        } else {
            target_velocity = self.velocity * target / target_mag;
        }

        let velocity = (position - state.prev_position) / delta_time;
        state.prev_position = position;
        let error = target_velocity - velocity;

        let correction = error / delta_time;

        correction.cap_magnitude(self.acceleration)
    }
}

/// Makes this entity movable.
///
/// This is alternative to adding physics body to the entity.
/// Movable entities are not affected by other entities
/// and they don't affect other entities.
#[derive(Clone, Copy, Debug)]
pub struct Motion2 {
    pub velocity: na::Vector2<f32>,

    // Treated as aplied force * mass
    pub acceleration: na::Vector2<f32>,

    // Treated as friction * mass.
    pub deceleration: f32,
}

impl Component for Motion2 {
    fn name() -> &'static str {
        "Motion2"
    }
}

impl Motion2 {
    pub fn new() -> Self {
        Self {
            velocity: na::Vector2::zeros(),
            acceleration: na::Vector2::zeros(),
            deceleration: 0.0,
        }
    }

    pub fn step(&mut self, delta_time: f32) -> na::Vector2<f32> {
        let mut delta_acc = self.acceleration * delta_time;

        let mag = self.velocity.norm();
        if self.deceleration > 0.0 {
            let dir = self.velocity / mag;

            // Make sure to avoid tremor on deceleration.
            delta_acc -= (self.deceleration * delta_time).min(mag) * dir;
        }

        self.velocity += delta_acc;
        self.velocity * delta_time
    }
}

/// Applies motion to entities.
fn motion_system(view: View<(&mut Motion2, &mut Global2), Without<Body>>, clocks: Res<ClockStep>) {
    let delta_time = clocks.step.as_secs_f32();

    for (m, g) in view {
        m.velocity.y -= 9.8 * delta_time;

        g.iso.translation.vector += m.step(delta_time);
    }
}

struct Motor2State {
    prev_position: na::Vector2<f32>,
}

impl Component for Motor2State {
    fn name() -> &'static str {
        "Motor2State"
    }
}

/// Motion modifier that moves entity to a position.
#[derive(Clone, Copy, Debug)]
pub struct MoveTo2 {
    /// Target of the motion.
    pub target: na::Point2<f32>,

    /// Distance offset.
    pub distance: f32,
}

impl MoveTo2 {
    pub fn new(target: na::Point2<f32>) -> Self {
        MoveTo2 {
            target,
            distance: EPSILON,
        }
    }

    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }
}

impl Component for MoveTo2 {
    fn name() -> &'static str {
        "MoveTo2"
    }
}

/// Applies motion to entities.
fn move_to_init_system(
    initial: View<
        (
            Entities,
            &Global2,
            &MoveTo2,
            &Motor2,
            Xor2<&mut Motion2, &Body>,
        ),
        Without<Motor2State>,
    >,
    clocks: Res<ClockStep>,
    mut physics: ResMut<PhysicsResource>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, g, mt, m, m_b) in initial {
        let mut state = Motor2::initial_state(g.iso.translation.vector);

        let acc = m.update(
            g.iso.translation.vector,
            mt.target,
            mt.distance,
            &mut state,
            delta_time,
        );

        match m_b {
            (Some(m), None) => {
                m.acceleration = acc;
            }
            (None, Some(b)) => {
                let body = physics.get_body_mut(b);
                body.add_force(body.mass() * acc, true);
            }
            _ => {
                unreachable!()
            }
        }

        encoder.insert(e, state);
    }
}

/// Applies motion to entities.
fn move_to_system(
    with_state: View<(
        &Global2,
        &MoveTo2,
        &Motor2,
        &mut Motor2State,
        Xor2<&mut Motion2, &mut Body>,
    )>,
    clocks: Res<ClockStep>,
    mut physics: ResMut<PhysicsResource>,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (g, mt, m, ms, m_b) in with_state {
        let acc = m.update(
            g.iso.translation.vector,
            mt.target,
            mt.distance,
            ms,
            delta_time,
        );
        match m_b {
            (Some(m), None) => {
                m.acceleration = acc;
            }
            (None, Some(b)) => {
                let body = physics.get_body_mut(b);
                body.add_force(body.mass() * acc, true);
            }
            _ => {
                unreachable!()
            }
        }
    }
}

/// Motion modifier that moves entity to a position of another entity with
/// specified offset.
#[derive(Clone, Copy)]
pub struct MoveAfter2 {
    /// Target entity.
    /// It target loses `Global2` component or target is no longer valid
    /// motion is stopped.
    pub id: EntityId,

    /// Offset from entity's origin.
    /// It not affected by entity's orientation.
    pub global_offset: na::Vector2<f32>,

    /// Offset from entity's origin.
    /// It affected by entity's orientation.
    // both offsets are used to calculate final offset
    pub local_offset: na::Vector2<f32>,

    /// Distance offset.
    pub distance: f32,
}

impl MoveAfter2 {
    pub fn new(id: EntityId) -> Self {
        MoveAfter2 {
            id,
            global_offset: na::Vector2::zeros(),
            local_offset: na::Vector2::zeros(),
            distance: EPSILON,
        }
    }

    pub fn with_global_offset(mut self, offset: na::Vector2<f32>) -> Self {
        self.global_offset = offset;
        self
    }

    pub fn with_local_offset(mut self, offset: na::Vector2<f32>) -> Self {
        self.local_offset = offset;
        self
    }

    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }
}

impl Component for MoveAfter2 {
    fn name() -> &'static str {
        "MoveAfter2"
    }
}

/// System to perform MoveAfter2 motion.
fn motion_after_init_system(
    initial: View<
        (
            Entities,
            &Global2,
            &MoveAfter2,
            &Motor2,
            Xor2<&mut Motion2, &Body>,
        ),
        Without<Motor2State>,
    >,
    globals: View<&Global2>,
    clocks: Res<ClockStep>,
    mut physics: ResMut<PhysicsResource>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, g, ma, m, m_b) in initial {
        match globals.try_get(ma.id) {
            Ok(tg) => {
                let target = tg.iso.transform_vector(&ma.local_offset) + ma.global_offset;

                let mut ms = Motor2::initial_state(g.iso.translation.vector);
                let acc = m.update(
                    g.iso.translation.vector,
                    target.into(),
                    ma.distance,
                    &mut ms,
                    delta_time,
                );
                match m_b {
                    (Some(m), None) => {
                        m.acceleration = acc;
                    }
                    (None, Some(b)) => {
                        let body = physics.get_body_mut(b);
                        body.add_force(body.mass() * acc, true);
                    }
                    _ => {
                        unreachable!()
                    }
                }
                encoder.insert(e, ms);
            }
            Err(_) => {
                // Remove motion. Target is no longer exists or invalid.
                encoder.drop::<MoveAfter2>(ma.id);
            }
        }
    }
}

/// System to perform MoveAfter2 motion.
fn motion_after_system(
    with_state: View<(
        &Global2,
        &MoveAfter2,
        &Motor2,
        &mut Motor2State,
        Xor2<&mut Motion2, &Body>,
    )>,
    globals: View<&Global2>,
    clocks: Res<ClockStep>,
    mut physics: ResMut<PhysicsResource>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (g, ma, m, ms, m_b) in with_state {
        match globals.try_get(ma.id) {
            Ok(tg) => {
                let target = tg.iso.rotation.transform_vector(&ma.local_offset)
                    + tg.iso.translation.vector
                    + ma.global_offset;

                let acc = m.update(
                    g.iso.translation.vector,
                    target.into(),
                    ma.distance,
                    ms,
                    delta_time,
                );
                match m_b {
                    (Some(m), None) => {
                        m.acceleration = acc;
                    }
                    (None, Some(b)) => {
                        let body = physics.get_body_mut(b);
                        body.add_force(body.mass() * acc, true);
                    }
                    _ => {
                        unreachable!()
                    }
                }
            }
            Err(_) => {
                // Remove motion. Target is no longer exists or invalid.
                encoder.drop::<MoveAfter2>(ma.id);
            }
        }
    }
}
