struct Constants {
    source_size: vec2<f32>,
    curvature: f32,
}

var<immediate> constants: Constants;

@group(0) @binding(0) var source_sampler: sampler;
@group(0) @binding(1) var source: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, 1.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
    );
    let uv = corners[index];
    var output: VertexOutput;
    output.position = vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    output.uv = uv;
    return output;
}

fn rounded_pixel_distance(local: vec2<f32>) -> f32 {
    let radius = 0.06;
    let q = abs(local) - vec2(0.43, 0.38) + radius;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // let flat = input.uv * 2.0 - 1.0;
    // let curve = vec2(flat.y * flat.y, flat.x * flat.x) * constants.curvature;
    // let screen = flat * (vec2(1.0) + curve);
    // if any(abs(screen) > vec2(1.0)) {
    //     return vec4(0.0);
    // }

    // let source_uv = screen * 0.5 + 0.5;
    // let source_position = source_uv * constants.source_size;
    // let source_pixel = min(floor(source_position), constants.source_size - vec2(1.0));
    // let sample_uv = (source_pixel + vec2(0.5)) / constants.source_size;
    // let local = fract(source_position) - vec2(0.5);
    // let distance = rounded_pixel_distance(local);
    // let antialias = clamp(fwidth(distance), 0.001, 0.04);
    // let pixel_mask = 1.0 - smoothstep(-antialias, antialias, distance);
    // return textureSampleLevel(source, source_sampler, sample_uv, 0.0) * pixel_mask;

    var uv = input.uv;

    // Centered coordinates
    var p = uv * 2.0 - 1.0;

    // Barrel distortion
    let r2 = dot(p, p);
    p *= 1.0 + constants.curvature * r2;

    uv = p * 0.5 + 0.5;

    // Outside the CRT
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) {
        return vec4(0.0);
    }

    var color = textureSample(source, source_sampler, uv).rgb;
    return vec4(color, 1.0);

    // Scanlines
    let line = 0.5 + 0.5 * cos(
        uv.y * constants.source_size.y * 3.14159265
    );

    color *= 1.0 - 0.15 * line;

    // Very subtle phosphor/RGB modulation
    let phase = uv.x * constants.source_size.x * 3.14159265;

    color.r *= 0.98 + 0.02 * sin(phase);
    color.g *= 0.99 + 0.01 * sin(phase + 2.094);
    color.b *= 0.98 + 0.02 * sin(phase + 4.189);

    return vec4(color, 1.0);
}
