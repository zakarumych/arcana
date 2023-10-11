

struct VertOutput {
    @builtin(position)
    position: vec4f,
    @location(0)
    sample: vec2f,
}

struct Constants {
    background: vec4f,
    shape_count: u32,
    camera: mat3x3f,
}

var<push_constant> pc: Constants;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertOutput {
    let pt = vec3f(f32(index >> 1u) * 4f - 1f, f32(index & 1u) * 4f - 1f, 1f);
    let st = pc.camera * pt;

    let output = VertOutput(vec4f(pt.xy, 0f, 1f), st.xy);
    return output;
}

struct Shape {
    inv_tr: mat3x3f,
    color: vec4f,
    kind: u32,
    payload: u32,
    layer: u32,
}


struct Circle {
    radius: f32,
}


struct Rect {
    half: vec2f,
}

@group(0) @binding(0) var<storage> shapes: array<Shape>;
@group(0) @binding(1) var<storage> circles: array<Circle>;
@group(0) @binding(2) var<storage> rects: array<Rect>;

fn sdf(shape: Shape, sample: vec2f) -> f32 {
    switch shape.kind {
        case 0u: {
            return circle_sdf(circles[shape.payload], sample);
        }
        case 1u: {
            return rect_sdf(rects[shape.payload], sample);
        }
        default: {
            return 0f;
        }
    }
}

fn circle_sdf(cirle: Circle, sample: vec2f) -> f32 {
    return length(sample) - cirle.radius;
}

fn rect_sdf(rect: Rect, sample: vec2f) -> f32 {
    let d = abs(sample) - rect.half;
    return length(max(d, vec2f(0f))) + min(max(d.x, d.y), 0f);
}

@fragment
fn fs_main(@location(0) sample: vec2f) -> @location(0) vec4f {
    for (var i = 0u; i < pc.shape_count; i++) {
        let shape = shapes[i];
        let shape_sample = shape.inv_tr * vec3f(sample, 1f);
        let d = sdf(shape, shape_sample.xy);

        if d <= 0f {
            return shape.color;
        }
    }

    return pc.background;
}
