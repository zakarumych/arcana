pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 720.0;
pub const LEFT: f32 = 40.0;
pub const RIGHT: f32 = 920.0;
pub const TOP: f32 = 100.0;
pub const PADDLE_Y: f32 = 624.0;
pub const PADDLE_HALF: f32 = 60.0;
pub const RADIUS: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Ready,
    Playing,
    Won,
    Lost,
}

pub struct Brick {
    pub x: f32,
    pub y: f32,
    pub row: usize,
    pub alive: bool,
}

pub const BRICK_W: f32 = 76.0;
pub const BRICK_H: f32 = 24.0;

pub struct Game {
    pub bricks: Vec<Brick>,
    pub paddle: f32,
    pub ball: [f32; 2],
    pub velocity: [f32; 2],
    pub keys: [bool; 4],
    pub phase: Phase,
    pub score: u32,
    pub lives: u32,
    pub trail: Vec<[f32; 2]>,
    trail_time: f32,
}

impl Game {
    pub fn new() -> Self {
        let bricks = (0..5)
            .flat_map(|row| {
                (0..10).map(move |col| Brick {
                    x: 64.0 + col as f32 * 84.0,
                    y: 154.0 + row as f32 * 34.0,
                    row,
                    alive: true,
                })
            })
            .collect();

        Self {
            bricks,
            paddle: WIDTH / 2.0,
            ball: [WIDTH / 2.0, PADDLE_Y - RADIUS - 1.0],
            velocity: [0.0; 2],
            keys: [false; 4],
            phase: Phase::Ready,
            score: 0,
            lives: 3,
            trail: Vec::new(),
            trail_time: 0.0,
        }
    }

    pub fn launch(&mut self) {
        if self.phase == Phase::Ready {
            self.phase = Phase::Playing;
            self.velocity = [190.0, -365.0];
        }
    }

    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 || matches!(self.phase, Phase::Won | Phase::Lost) {
            return;
        }
        // Limit catch-up and movement per collision step to less than a ball radius.
        let dt = dt.min(0.1);
        let steps = (dt / (1.0 / 240.0)).ceil() as u32;
        for _ in 0..steps {
            self.step(dt / steps as f32);
        }
    }

    fn step(&mut self, dt: f32) {
        let direction =
            (self.keys[2] || self.keys[3]) as i32 - (self.keys[0] || self.keys[1]) as i32;

        self.paddle = (self.paddle + direction as f32 * 610.0 * dt)
            .clamp(LEFT + PADDLE_HALF, RIGHT - PADDLE_HALF);

        if self.phase == Phase::Ready {
            self.ball = [self.paddle, PADDLE_Y - RADIUS - 1.0];
            return;
        }

        if self.phase != Phase::Playing {
            return;
        }

        let previous = self.ball;

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

        if self.velocity[1] > 0.0
            && previous[1] + RADIUS <= PADDLE_Y
            && self.ball[1] + RADIUS >= PADDLE_Y
            && (self.ball[0] - self.paddle).abs() <= PADDLE_HALF + RADIUS
        {
            let angle = ((self.ball[0] - self.paddle) / PADDLE_HALF).clamp(-1.0, 1.0) * 1.05;
            let speed = self.velocity[0].hypot(self.velocity[1]).min(640.0);
            self.velocity = [speed * angle.sin(), -speed * angle.cos()];
            self.ball[1] = PADDLE_Y - RADIUS - 0.01;
        }

        for brick in &mut self.bricks {
            if !brick.alive {
                continue;
            }

            let closest = [
                self.ball[0].clamp(brick.x, brick.x + BRICK_W),
                self.ball[1].clamp(brick.y, brick.y + BRICK_H),
            ];

            let delta = [self.ball[0] - closest[0], self.ball[1] - closest[1]];
            if delta[0] * delta[0] + delta[1] * delta[1] > RADIUS * RADIUS {
                continue;
            }

            brick.alive = false;
            self.score += (5 - brick.row as u32) * 10;

            if previous[1] <= brick.y - RADIUS {
                self.ball[1] = brick.y - RADIUS - 0.01;
                self.velocity[1] = -self.velocity[1].abs();
            } else if previous[1] >= brick.y + BRICK_H + RADIUS {
                self.ball[1] = brick.y + BRICK_H + RADIUS + 0.01;
                self.velocity[1] = self.velocity[1].abs();
            } else if previous[0] < brick.x {
                self.ball[0] = brick.x - RADIUS - 0.01;
                self.velocity[0] = -self.velocity[0].abs();
            } else {
                self.ball[0] = brick.x + BRICK_W + RADIUS + 0.01;
                self.velocity[0] = self.velocity[0].abs();
            }
            let speed = self.velocity[0].hypot(self.velocity[1]);
            let factor = (speed + 5.0).min(640.0) / speed;
            self.velocity[0] *= factor;
            self.velocity[1] *= factor;
            break;
        }

        if self.bricks.iter().all(|brick| !brick.alive) {
            self.phase = Phase::Won;
        }

        if self.ball[1] > 672.0 + RADIUS && self.phase == Phase::Playing {
            self.lives -= 1;
            self.phase = if self.lives == 0 {
                Phase::Lost
            } else {
                Phase::Ready
            };
            self.velocity = [0.0; 2];
            self.trail.clear();
            self.ball = [self.paddle, PADDLE_Y - RADIUS - 1.0];
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
