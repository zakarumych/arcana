const NUM_PARTICLES: u32 = 1500u;

struct Particle {
  pos : vec2<f32>,
  vel : vec2<f32>,
}

struct SimParams {
  deltaT : f32,
  rule1Distance : f32,
  rule2Distance : f32,
  rule3Distance : f32,
  rule1Scale : f32,
  rule2Scale : f32,
  rule3Scale : f32,
}

struct Particles {
  particles : array<Particle>
}

@group(0) @binding(0) var<storage> params : array<SimParams>;
@group(0) @binding(1) var<storage> particlesSrc : Particles;
@group(0) @binding(2) var<storage,read_write> particlesDst : Particles;

// https://github.com/austinEng/Project6-Vulkan-Flocking/blob/master/data/shaders/computeparticles/particle.comp
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_invocation_id : vec3<u32>) {
  particlesDst.particles[0] = particlesSrc.particles[0];
  particlesDst.particles[0].pos = vec2<f32>(params[0].deltaT, params[0].rul;
}
