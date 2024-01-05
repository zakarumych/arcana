use std::f32::EPSILON;

use arcana::{
    edict::{ActionEncoder, Component, Entities, EntityId, Res, View, Without},
    gametime::ClockStep,
};

pub struct Motor {
    /// Cruise velocity for the motor.
    pub velocity: f32,

    /// Maximum acceleration for the motor.
    pub acceleration: f32,

    /// Deceleration threshold.
    pub threshold: f32,
}

impl Component for Motor {
    fn name() -> &'static str {
        "Motor"
    }
}

impl Motor {
    pub fn new(velocity: f32, acceleration: f32) -> Self {
        // Find stopping threshold and add 10% to it just in case.
        let threshold = (velocity * velocity / acceleration / 2.0) * 2.0;
        Motor {
            velocity,
            acceleration,
            threshold,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    fn initial_state(position: Vector<f32>) -> MotorState {
        MotorState {
            prev_pos: position,
            prev_vel: Vector::zeros(),
            integral: Vector::zeros(),

            apply_velocity: Vector::zeros(),
            apply_acceleration: Vector::zeros(),
            apply_deceleration: 0.1,

            force: Vector::zeros(),
            impulse: Vector::zeros(),
        }
    }

    /// Calculate required motion for the entity to eventually reach the target.
    fn update(
        &self,
        position: Vector<f32>,
        target: Point<f32>,
        distance: f32,
        state: &mut MotorState,
        delta_time: f32,
    ) {
        let mut error = target.coords - position;
        let error_mag = error.magnitude();

        if error_mag < distance {
            error = Vector::zeros();
            // error_mag = 0.0;
        } else {
            // target -= target / error_mag * distance;
            error *= (error_mag - distance) / error_mag;
            // error_mag -= distance;
        }

        const KP: f32 = 1.0;
        const TI: f32 = 3.0;
        const TD: f32 = 3.0;

        // Use velocity based PID.
        let target_velocity = if error_mag > EPSILON && error_mag > self.threshold {
            self.velocity * error / error_mag
        } else if self.threshold > EPSILON {
            self.velocity * error / self.threshold
        } else {
            Vector::zeros()
        };

        let velocity = (position - state.prev_pos) / delta_time;
        let correction;

        //self.threshold > EPSILON && error_mag < self.threshold {
        // Use position based PID.

        let velocity_error = target_velocity - velocity;

        state.integral += velocity_error * delta_time;

        let neg_integral = state.integral.dot(&velocity_error);
        if neg_integral < 0.0 {
            state.integral -= velocity_error.normalize() * neg_integral * delta_time;
        }

        state.integral = state.integral.cap_magnitude(self.acceleration);

        correction = (error + state.integral * TI + velocity_error * TD) * KP;

        state.prev_pos = position;
        state.prev_vel = velocity;

        state.apply_acceleration = correction.cap_magnitude(self.acceleration);
    }
}

struct MotorState {
    prev_pos: Vector<f32>,
    prev_vel: Vector<f32>,
    integral: Vector<f32>,

    apply_velocity: Vector<f32>,

    // Treated as aplied force * mass
    apply_acceleration: Vector<f32>,

    // Treated as friction * mass.
    apply_deceleration: f32,

    // Force already applied to the entity.
    force: Vector<f32>,

    // Impulse already applied to the entity.
    impulse: Vector<f32>,
}

impl Component for MotorState {
    fn name() -> &'static str {
        "MotorState"
    }
}

impl MotorState {
    pub fn update_velocity(&mut self, delta_time: f32) {
        let mut delta_acc = self.apply_acceleration * delta_time;

        let mag = self.apply_velocity.norm();
        if self.apply_deceleration > 0.0 {
            let dir = self.apply_velocity / mag;

            // Make sure to avoid tremor on deceleration.
            delta_acc -= (self.apply_deceleration * delta_time).min(mag) * dir;
        }

        self.apply_velocity += delta_acc;
    }

    pub fn find_translation(&self, delta_time: f32) -> Vector<f32> {
        self.apply_velocity * delta_time
    }
}

/// Motion modifier that moves entity to a position.
#[derive(Clone, Copy, Debug)]
pub struct MoveTo {
    /// Target of the motion.
    pub target: Point<f32>,

    /// Distance offset.
    pub distance: f32,
}

impl MoveTo {
    pub fn new(target: Point<f32>) -> Self {
        MoveTo {
            target,
            distance: EPSILON,
        }
    }

    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }
}

/// Motion modifier that moves entity to a position of another entity with
/// specified offset.
#[derive(Clone, Copy)]
pub struct MoveAfter {
    /// Target entity.
    /// It target loses `Global` component or target is no longer valid
    /// motion is stopped.
    pub id: EntityId,

    /// Offset from entity's origin.
    /// It not affected by entity's orientation.
    pub global_offset: Vector<f32>,

    /// Offset from entity's origin.
    /// It affected by entity's orientation.
    // both offsets are used to calculate final offset
    pub local_offset: Vector<f32>,

    /// Distance offset.
    pub distance: f32,
}

impl MoveAfter {
    pub fn new(id: EntityId) -> Self {
        MoveAfter {
            id,
            global_offset: Vector::zeros(),
            local_offset: Vector::zeros(),
            distance: EPSILON,
        }
    }

    pub fn with_global_offset(mut self, offset: Vector<f32>) -> Self {
        self.global_offset = offset;
        self
    }

    pub fn with_local_offset(mut self, offset: Vector<f32>) -> Self {
        self.local_offset = offset;
        self
    }

    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }
}

pub enum Motion {
    To(MoveTo),
    After(MoveAfter),
}

impl Motion {
    #[inline]
    pub fn to(target: Point<f32>) -> Self {
        Motion::To(MoveTo::new(target))
    }

    #[inline]
    pub fn after(id: EntityId) -> Self {
        Motion::After(MoveAfter::new(id))
    }
}

impl From<MoveTo> for Motion {
    #[inline]
    fn from(m: MoveTo) -> Self {
        Motion::To(m)
    }
}

impl From<MoveAfter> for Motion {
    #[inline]
    fn from(m: MoveAfter) -> Self {
        Motion::After(m)
    }
}

impl Component for Motion {
    fn name() -> &'static str {
        "Motion"
    }
}

/// Applies motion to entities.
fn infer_motion(
    with_state: View<(Entities, &Global, &Motion, &Motor, Option<&mut MotorState>)>,
    globals: View<&Global>,
    clocks: Res<ClockStep>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, global, the_move, motor, motor_state_opt) in with_state {
        let mut new_motor_state = None;

        let motor_state = match motor_state_opt {
            Some(motor_state) => motor_state,
            None => {
                new_motor_state.get_or_insert(Motor::initial_state(global.iso.translation.vector))
            }
        };

        match the_move {
            Motion::To(move_to) => motor.update(
                global.iso.translation.vector,
                move_to.target,
                move_to.distance,
                motor_state,
                delta_time,
            ),
            Motion::After(move_after) => {
                match globals.try_get(move_after.id) {
                    Ok(tg) => {
                        let target = tg.iso.rotation.transform_vector(&move_after.local_offset)
                            + tg.iso.translation.vector
                            + move_after.global_offset;

                        motor.update(
                            global.iso.translation.vector,
                            target.into(),
                            move_after.distance,
                            motor_state,
                            delta_time,
                        );
                    }
                    Err(_) => {
                        motor_state.apply_acceleration = Vector::zeros();
                        motor_state.apply_velocity = Vector::zeros();

                        // Remove motion. Target is no longer exists or invalid.
                        encoder.drop::<Motion>(move_after.id);
                        continue;
                    }
                }
            }
        };

        if let Some(motor_state) = new_motor_state {
            encoder.insert(e, motor_state);
        }
    }
}

fn cancel_move(
    state: View<(Entities, &mut MotorState), Without<Motion>>,
    mut encoder: ActionEncoder,
) {
    for (e, state) in state {
        state.apply_acceleration = Vector::zeros();
        state.apply_velocity = Vector::zeros();
        encoder.drop::<MotorState>(e);
    }
}

/// Applies motion to entities.
fn do_motion(
    entities: View<(&mut MotorState, &mut Global, Option<&mut RigidBody>)>,
    clocks: Res<ClockStep>,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (state, global, body) in entities {
        match body {
            None => {
                state.update_velocity(delta_time);
                global.iso.translation.vector += state.find_translation(delta_time);
            }
            Some(body) => match body.body_type() {
                RigidBodyType::Fixed => {}
                RigidBodyType::Dynamic => {
                    let apply_force = state.apply_acceleration * body.mass();
                    body.apply_force(apply_force - state.force);
                    state.force = apply_force;
                }
                RigidBodyType::KinematicVelocityBased => {
                    state.update_velocity(delta_time);
                    let apply_impulse = state.apply_velocity * body.mass();
                    body.apply_impulse(apply_impulse - state.impulse);
                    state.impulse = apply_impulse;
                }
                RigidBodyType::KinematicPositionBased => {
                    state.update_velocity(delta_time);
                    global.iso.translation.vector += state.find_translation(delta_time);
                }
            },
        }
    }
}

pub fn make_motion_system() -> impl arcana::System {
    use arcana::IntoSystem;

    (
        infer_motion.into_system(),
        cancel_move.into_system(),
        do_motion.into_system(),
    )
        .into_system()
}
