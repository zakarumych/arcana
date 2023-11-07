use std::f32::EPSILON;

use arcana::{
    edict::{ActionEncoder, Component, Entities, EntityId, Res, Scheduler, View, Without, World},
    gametime::ClockStep,
    plugin::ArcanaPlugin,
};
use physics::Body;
use scene::Global2;

arcana::export_arcana_plugin!(MotionPlugin);

pub struct MotionPlugin;

impl ArcanaPlugin for MotionPlugin {
    fn name(&self) -> &'static str {
        "motion"
    }

    arcana::feature_ed! {
        fn dependencies(&self) -> Vec<(&'static dyn ArcanaPlugin, arcana::project::Dependency)> {
            vec![scene::path_dependency(), physics::path_dependency()]
        }
    }

    fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
        world.ensure_component_registered::<Motion2>();
        world.ensure_component_registered::<MoveTo2>();
        world.ensure_component_registered::<MoveAfter2>();

        scheduler.add_system(motion_after_system_initial);
        scheduler.add_system(motion_after_system);
        scheduler.add_system(move_to_system);
        scheduler.add_system(move_to_system_initial);
        scheduler.add_system(motion_system);
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
fn motion_system(view: View<(&mut Motion2, &mut Global2)>, clocks: Res<ClockStep>) {
    let delta_time = clocks.step.as_secs_f32();

    for (m, g) in view {
        m.velocity.y -= 9.8 * delta_time;

        g.iso.translation.vector += m.step(delta_time);
    }
}

struct MoveTo2State {
    prev_position: na::Vector2<f32>,
    integral: na::Vector2<f32>,
}

impl Component for MoveTo2State {
    fn name() -> &'static str {
        "MoveTo2State"
    }
}

/// Motion modifier that moves entity to a position.
#[derive(Clone, Copy, Debug)]
pub struct MoveTo2 {
    /// Target of the motion.
    pub target: na::Point2<f32>,

    /// Maximum velocity.
    pub velocity: f32,

    /// Maximum acceleration.
    pub acceleration: f32,

    /// Deceleration threshold.
    pub threshold: f32,
}

impl MoveTo2 {
    fn initial_state(position: na::Vector2<f32>) -> MoveTo2State {
        MoveTo2State {
            prev_position: position,
            integral: na::Vector2::zeros(),
        }
    }

    /// Calculate required motion for the entity to eventually reach the target.
    fn update(
        &self,
        position: &na::Vector2<f32>,
        state: &mut MoveTo2State,
        delta_time: f32,
    ) -> na::Vector2<f32> {
        let target = self.target.coords - *position;
        let target_mag = target.magnitude();

        let velocity = (position - state.prev_position) / delta_time;
        state.prev_position = *position;

        let target_velocity;

        if target_mag < self.threshold {
            target_velocity = self.velocity * target / self.threshold;
        } else {
            target_velocity = self.velocity * target / target_mag;
        }

        let error = target_velocity - velocity;

        state.integral += error * delta_time;
        state.integral = state.integral.cap_magnitude(self.acceleration);
        state.integral *= 1.0 - delta_time;

        const KP: f32 = 1.0;
        const KI: f32 = 0.5;

        let correction = KP * error / delta_time + KI * state.integral / delta_time;

        correction.cap_magnitude(self.acceleration)
    }
}

impl Component for MoveTo2 {
    fn name() -> &'static str {
        "MoveTo2"
    }
}

/// Applies motion to entities.
fn move_to_system_initial(
    view: View<(Entities, &mut Motion2, &MoveTo2, &Global2), Without<MoveTo2State>>,
    clocks: Res<ClockStep>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, m, mt, g) in view {
        let mut state = MoveTo2::initial_state(g.iso.translation.vector);

        let acc = mt.update(&g.iso.translation.vector, &mut state, delta_time);
        m.acceleration = acc;
        encoder.insert(e, state);
    }
}

/// Applies motion to entities.
fn move_to_system(
    view: View<(
        Entities,
        &mut Motion2,
        &MoveTo2,
        &mut MoveTo2State,
        &Global2,
    )>,
    clocks: Res<ClockStep>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, m, mt, ms, g) in view {
        let acc = mt.update(&g.iso.translation.vector, ms, delta_time);
        m.acceleration = acc;
    }
}

/// Motion modifier that moves entity to a position of another entity with
/// specified offset.
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

    /// Maximum velocity.
    pub velocity: f32,

    /// Maximum acceleration.
    pub acceleration: f32,

    /// Distance to target at which motion is stopped.
    pub threshold: f32,
}

impl Component for MoveAfter2 {
    fn name() -> &'static str {
        "MoveAfter2"
    }
}

impl MoveAfter2 {
    fn update_motion(&self, move_to: &mut MoveTo2, globals: &View<&Global2>) -> bool {
        let Ok(g) = globals.try_get(self.id) else {
            return false;
        };
        let target = g.iso.translation.vector
            + self.global_offset
            + g.iso.rotation.transform_vector(&self.local_offset);

        move_to.target = na::Point2::from(target);
        move_to.velocity = self.velocity;
        move_to.threshold = self.threshold;
        true
    }
}

/// Initial system to start MoveAfter2 motion.
fn motion_after_system_initial(
    view: View<(Entities, &Global2, &MoveAfter2, &mut Motion2), Without<MoveTo2State>>,
    globals: View<&Global2>,
    clocks: Res<ClockStep>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, g, ma, m) in view {
        match globals.try_get(ma.id) {
            Ok(tg) => {
                let target = tg.iso.transform_vector(&ma.local_offset) + ma.global_offset;

                let mut ms = MoveTo2::initial_state(g.iso.translation.vector);
                let mt = MoveTo2 {
                    target: target.into(),
                    velocity: ma.velocity,
                    acceleration: ma.acceleration,
                    threshold: ma.threshold,
                };
                let acc = mt.update(&g.iso.translation.vector, &mut ms, delta_time);
                encoder.insert(e, ms);
                m.acceleration = acc;
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
    view: View<(
        Entities,
        &Global2,
        &MoveAfter2,
        &mut MoveTo2State,
        &mut Motion2,
    )>,
    globals: View<&Global2>,
    clocks: Res<ClockStep>,
    mut encoder: ActionEncoder,
) {
    let delta_time = clocks.step.as_secs_f32();

    for (e, g, ma, ms, m) in view {
        match globals.try_get(ma.id) {
            Ok(tg) => {
                let target = tg.iso.rotation.transform_vector(&ma.local_offset)
                    + tg.iso.translation.vector
                    + ma.global_offset;

                let mt = MoveTo2 {
                    target: target.into(),
                    velocity: ma.velocity,
                    acceleration: ma.acceleration,
                    threshold: ma.threshold,
                };
                let acc = mt.update(&g.iso.translation.vector, ms, delta_time);
                m.acceleration = acc;
            }
            Err(_) => {
                // Remove motion. Target is no longer exists or invalid.
                encoder.drop::<MoveAfter2>(ma.id);
            }
        }
    }
}
