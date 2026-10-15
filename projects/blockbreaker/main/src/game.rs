use crate::scene::{Block, Paddle};
use arcana::ecs::query::Entities;
use arcana::{
    athena::Vector2,
    ecs::{entity::EntityId, world::World},
    scene::{
        self,
        dim2::{LocalTransform, Rect, Transform},
    },
};
use std::collections::{HashMap, HashSet};
pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 720.0;
pub const LEFT: f32 = 40.0;
pub const RIGHT: f32 = 920.0;
pub const TOP: f32 = 100.0;
pub const RADIUS: f32 = 8.0;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Ready,
    Playing,
    Won,
    Lost,
}
pub struct Game {
    pub ball: [f32; 2],
    pub velocity: [f32; 2],
    pub keys: [bool; 4],
    pub phase: Phase,
    pub score: u32,
    pub lives: u32,
    pub trail: Vec<[f32; 2]>,
    pub destroyed: HashSet<EntityId>,
    initial_paddles: HashMap<EntityId, LocalTransform>,
    trail_time: f32,
    pub restart: bool,
}
impl Game {
    pub fn new() -> Self {
        Self {
            ball: [480.0, 615.0],
            velocity: [0.0; 2],
            keys: [false; 4],
            phase: Phase::Ready,
            score: 0,
            lives: 3,
            trail: Vec::new(),
            destroyed: HashSet::new(),
            initial_paddles: HashMap::new(),
            trail_time: 0.0,
            restart: false,
        }
    }
    pub fn launch(&mut self) {
        if self.phase == Phase::Ready {
            self.phase = Phase::Playing;
            self.velocity = [190.0, -365.0];
        }
    }
    pub fn advance(&mut self, world: &mut World, dt: f32) {
        for (id, local, _) in world.view::<(Entities, &LocalTransform, &Paddle)>().iter() {
            self.initial_paddles.entry(id.id()).or_insert(*local);
        }
        if self.restart {
            for (&id, &local) in &self.initial_paddles {
                if world.get::<&Paddle>(id).is_ok() {
                    let _ = world.insert(id, local);
                }
            }
            let initial = std::mem::take(&mut self.initial_paddles);
            *self = Self::new();
            self.initial_paddles = initial;
        }
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.1);
        let steps = (dt * 240.0).ceil() as u32;
        for _ in 0..steps {
            self.step(world, dt / steps as f32);
        }
    }
    fn step(&mut self, world: &mut World, dt: f32) {
        if matches!(self.phase, Phase::Won | Phase::Lost) {
            return;
        }
        let direction =
            (self.keys[2] || self.keys[3]) as i32 - (self.keys[0] || self.keys[1]) as i32;
        // Controls move along each paddle's authored local horizontal axis.
        for (paddle, local) in world.view::<(&Paddle, &mut LocalTransform)>().iter_mut() {
            if direction == 0 {
                continue;
            }
            let mut p = local.position();
            let delta = (direction as f32 * paddle.speed * dt) as f64;
            p.x += delta * local.angle().cos();
            p.y += delta * local.angle().sin();
            *local = LocalTransform::new(p, local.angle());
        }
        if scene::update_transforms(world).is_err() {
            return;
        }
        let paddles: Vec<_> = world
            .view::<(Entities, &Paddle, &Transform, &Rect)>()
            .iter()
            .map(|(id, _, t, r)| (id.id(), *t, *r))
            .collect();
        let anchor = paddles
            .iter()
            .min_by_key(|(id, _, _)| *id)
            .map(|(_, t, r)| {
                t.transform_point(Vector2::new(
                    (r.min.x + r.max.x) * 0.5,
                    r.min.y - RADIUS as f64 - 1.0,
                ))
            });
        if self.phase == Phase::Ready {
            if let Some(p) = anchor {
                self.ball = [p.x as f32, p.y as f32];
            }
            return;
        }
        self.ball[0] += self.velocity[0] * dt;
        self.ball[1] += self.velocity[1] * dt;
        if self.ball[0] < LEFT + RADIUS {
            self.ball[0] = LEFT + RADIUS;
            self.velocity[0] = self.velocity[0].abs();
        }
        if self.ball[0] > RIGHT - RADIUS {
            self.ball[0] = RIGHT - RADIUS;
            self.velocity[0] = -self.velocity[0].abs();
        }
        if self.ball[1] < TOP + RADIUS {
            self.ball[1] = TOP + RADIUS;
            self.velocity[1] = self.velocity[1].abs();
        }
        for (_, t, r) in paddles {
            collide(&mut self.ball, &mut self.velocity, t, r);
        }
        let mut remaining = 0;
        for (id, block, t, r) in world.view::<(Entities, &Block, &Transform, &Rect)>().iter() {
            if self.destroyed.contains(&id.id()) {
                continue;
            }
            remaining += 1;
            if collide(&mut self.ball, &mut self.velocity, *t, *r) {
                self.destroyed.insert(id.id());
                self.score = self.score.saturating_add(block.points);
                remaining -= 1;
            }
        }
        if remaining == 0 {
            self.phase = Phase::Won;
        }
        if self.ball[1] > 680.0 && self.phase == Phase::Playing {
            self.lives = self.lives.saturating_sub(1);
            self.phase = if self.lives == 0 {
                Phase::Lost
            } else {
                Phase::Ready
            };
            self.velocity = [0.0; 2];
            self.trail.clear();
        }
        self.trail_time += dt;
        if self.trail_time >= 0.018 {
            self.trail_time = 0.0;
            if self.trail.len() >= 10 {
                self.trail.remove(0);
            }
            self.trail.push(self.ball);
        }
    }
}
/// Resolve circle against an oriented local rectangle, reflecting only approaching velocity.
fn collide(ball: &mut [f32; 2], velocity: &mut [f32; 2], t: Transform, r: Rect) -> bool {
    if r.min.x >= r.max.x || r.min.y >= r.max.y {
        return false;
    }
    let p = t
        .inverse()
        .transform_point(Vector2::new(ball[0] as f64, ball[1] as f64));
    let q = Vector2::new(p.x.clamp(r.min.x, r.max.x), p.y.clamp(r.min.y, r.max.y));
    let mut n = Vector2::new(p.x - q.x, p.y - q.y);
    let distance = n.x.hypot(n.y);
    if distance > RADIUS as f64 {
        return false;
    }
    let penetration;
    if distance > 1e-9 {
        n.x /= distance;
        n.y /= distance;
        penetration = RADIUS as f64 - distance;
    } else {
        let distances = [p.x - r.min.x, r.max.x - p.x, p.y - r.min.y, r.max.y - p.y];
        let index = (0..4)
            .min_by(|&a, &b| distances[a].total_cmp(&distances[b]))
            .expect("four rectangle faces");
        n = [
            Vector2::new(-1.0, 0.0),
            Vector2::new(1.0, 0.0),
            Vector2::new(0.0, -1.0),
            Vector2::new(0.0, 1.0),
        ][index];
        penetration = RADIUS as f64 + distances[index];
    }
    let origin = t.transform_point(Vector2::new(0.0, 0.0));
    let normal = t.transform_point(n) - origin;
    ball[0] += (normal.x * (penetration + 0.01)) as f32;
    ball[1] += (normal.y * (penetration + 0.01)) as f32;
    let dot = velocity[0] as f64 * normal.x + velocity[1] as f64 * normal.y;
    if dot < 0.0 {
        velocity[0] -= (2.0 * dot * normal.x) as f32;
        velocity[1] -= (2.0 * dot * normal.y) as f32;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::rect;
    #[test]
    fn rotated_offset_bounds_reflect_in_world_space() {
        let t = Transform::new(Vector2::new(400.0, 300.0), std::f64::consts::FRAC_PI_2);
        let r = rect(10.0, 20.0, 60.0, 12.0);
        let p = t.transform_point(Vector2::new(40.0, 14.0));
        let mut ball = [p.x as f32, p.y as f32];
        let mut velocity = [-100.0, 0.0];
        assert!(collide(&mut ball, &mut velocity, t, r));
        assert!(velocity[0] > 0.0);
        let mut far = [700.0, 600.0];
        assert!(!collide(&mut far, &mut velocity, t, r));
    }
    #[test]
    fn runtime_uses_added_moved_resized_blocks_and_restart_keeps_deletions() {
        let mut world = World::new();
        let deleted = world
            .spawn((
                Block::default(),
                LocalTransform::default(),
                rect(0.0, 0.0, 10.0, 10.0),
            ))
            .id();
        world.despawn(deleted).expect("test entity alive");
        let hit = world
            .spawn((
                Block { points: 70 },
                LocalTransform::new(Vector2::new(300.0, 300.0), 0.0),
                rect(0.0, 0.0, 100.0, 30.0),
            ))
            .id();
        let mut game = Game::new();
        game.phase = Phase::Playing;
        game.ball = [350.0, 294.0];
        game.velocity = [0.0, 100.0];
        game.advance(&mut world, 1.0 / 240.0);
        assert!(game.destroyed.contains(&hit));
        assert_eq!(game.score, 70);
        assert_eq!(game.phase, Phase::Won);
        game.restart = true;
        game.advance(&mut world, 1.0 / 240.0);
        assert!(game.destroyed.is_empty());
        assert_eq!(game.score, 0);
        assert!(world.get::<&Block>(deleted).is_err());
        world.despawn(hit).expect("test block alive");
        game.launch();
        game.advance(&mut world, 1.0 / 240.0);
        assert_eq!(game.phase, Phase::Won);
    }
    #[test]
    fn paddle_authored_position_rotation_and_removal_are_honored() {
        let mut world = World::new();
        let id = world
            .spawn((
                Paddle::default(),
                LocalTransform::new(Vector2::new(20.0, 400.0), std::f64::consts::FRAC_PI_2),
                rect(-20.0, -4.0, 40.0, 8.0),
            ))
            .id();
        let mut game = Game::new();
        game.advance(&mut world, 0.01);
        let original = world.get::<&LocalTransform>(id).expect("paddle").position();
        assert!((original.x - 20.0).abs() < 1e-8);
        assert!((game.ball[0] - 33.0).abs() < 1e-4);
        assert!((game.ball[1] - 400.0).abs() < 1e-4);
        game.keys[2] = true;
        game.advance(&mut world, 0.01);
        let moved = world.get::<&LocalTransform>(id).expect("paddle").position();
        assert!((moved.x - original.x).abs() < 1e-8);
        assert!(moved.y > original.y);
        game.restart = true;
        game.advance(&mut world, 0.01);
        assert_eq!(
            world.get::<&LocalTransform>(id).expect("paddle").position(),
            original
        );
        world.despawn(id).expect("paddle alive");
        game.restart = true;
        game.advance(&mut world, 0.01);
        assert!(world.get::<&Paddle>(id).is_err());
    }
}
