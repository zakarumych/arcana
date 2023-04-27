
struct VertOutput {
    @builtin(position)
    position: vec4<f32>,

    @location(0)
    color: vec3<f32>,
}

fn linear_srgb_to_oklab(c: vec3<f32>) -> vec3<f32>
{
    let l = 0.4122214708f * c.r + 0.5363325363f * c.g + 0.0514459929f * c.b;
	let m = 0.2119034982f * c.r + 0.6806995451f * c.g + 0.1073969566f * c.b;
	let s = 0.0883024619f * c.r + 0.2817188376f * c.g + 0.6299787005f * c.b;

    let l_ = pow(l, 1.0 / 3.0);
    let m_ = pow(m, 1.0 / 3.0);
    let s_ = pow(s, 1.0 / 3.0);

    return vec3<f32>(
        0.2104542553f*l_ + 0.7936177850f*m_ - 0.0040720468f*s_,
        1.9779984951f*l_ - 2.4285922050f*m_ + 0.4505937099f*s_,
        0.0259040371f*l_ + 0.7827717662f*m_ - 0.8086757660f*s_,
    );
}

fn oklab_to_linear_srgb(c: vec3<f32>) -> vec3<f32>
{
    let l_ = c.x + 0.3963377774f * c.y + 0.2158037573f * c.z;
    let m_ = c.x - 0.1055613458f * c.y - 0.0638541728f * c.z;
    let s_ = c.x - 0.0894841775f * c.y - 1.2914855480f * c.z;

    let l = l_*l_*l_;
    let m = m_*m_*m_;
    let s = s_*s_*s_;

    return vec3<f32>(
		4.0767416621f * l - 3.3077115913f * m + 0.2309699292f * s,
		-1.2684380046f * l + 2.6097574011f * m - 0.3413193965f * s,
		-0.0041960863f * l - 0.7034186147f * m + 1.7076147010f * s,
    );
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertOutput {
    let colors = array<vec3<f32>, 3>(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 1.0));
    let rgb = colors[index];
    let output = VertOutput(
        vec4<f32>(-0.5 + (f32(index) * 0.5), -0.5 + f32(index == 1), 0.0, 1.0),
        linear_srgb_to_oklab(rgb),
    );
    return output;
}

@fragment
fn fs_main(@location(0) color: vec3<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(oklab_to_linear_srgb(color), 1.0);
}
