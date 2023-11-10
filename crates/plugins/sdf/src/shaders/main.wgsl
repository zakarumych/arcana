

struct VertOutput {
    @builtin(position)
    position: vec4f,
    @location(0)
    sample: vec2f,
}

struct Constants {
    background: vec4f,
    camera: mat3x3f,
    shape_count: u32,
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
    tr: mat3x3f,
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
    half_box: vec2f,
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
    let d = abs(sample) - rect.half_box;
    return length(max(d, vec2f(0f))) + min(max(d.x, d.y), 0f);
}

@fragment
fn fs_main(@location(0) sample: vec2f) -> @location(0) vec4f {
    for (var i = 0u; i < pc.shape_count; i++) {
        let shape = shapes[i];
        let shape_sample = shape.inv_tr * vec3f(sample, 1f);
        let d = sdf(shape, shape_sample.xy);
        if d <= -0.001f {
            let dd = abs(vec2f(d / dpdx(d) * dpdx(sample.x), d / dpdy(d) *  dpdy(sample.y)));
            var ddd = vec2f(0f, 0f);
            if dd.x > 10000000f {
                ddd = vec2f(0f, dd.y);
            } else if dd.y > 10000000f {
                ddd = vec2f(dd.x, 0f);
            } else {
                ddd = (dd.x * dd.y / length(dd)) * normalize(dd.yx);
            }

            let dd_w = shape.tr * vec3f(ddd, 0f);

            if length(dd_w) < 0.1f {
                return vec4f(0f, 0f, 0f, 1f);
            }
            return shape.color;
        }
    }

    return pc.background;
}
