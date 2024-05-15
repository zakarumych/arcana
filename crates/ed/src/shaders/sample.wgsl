
struct Constants {
    extent: vec2f,
}

var<push_constant> pc: Constants;
@group(0) @binding(0) var src : texture_2d<f32>;
@group(0) @binding(1) var s: sampler;

struct VertOutput {
    @builtin(position)
    position: vec4f,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertOutput {
    var pos: vec2f;
    switch index {
        case 0u: {
            pos = vec2f(-1.0, -1.0);
        }
        case 1u: {
            pos = vec2f(3.0, -1.0);
        }
        case 2u: {
            pos = vec2f(-1.0, 3.0);
        }
        default: {
            pos = vec2f(0.0, 0.0);
        }
    };

    return VertOutput(vec4f(pos, 0.0, 1.0));


    // let x = (-0.5 + (f32(index) * 0.5));
    // let y = (-(sqrt(3.0) / 6.0) + f32(index == 1u) * sqrt(3.0) / 2.0);

    // let a = 0.0;

    // let ca = cos(a);
    // let sa = sin(a);

    // let output = VertOutput(
    //     vec4<f32>((ca * x + sa * y), (ca * y - sa * x), 0.0, 1.0),
    // );
    // return output;
}

@fragment
fn fs_main(@builtin(position) id: vec4f) -> @location(0) vec4<f32> {
    // return vec4f(1.0, 0.0, 0.0, 1.0);

    let c = textureSample(src, s, id.xy / pc.extent).rgb;
    return vec4f(c, 1.0);
}   
