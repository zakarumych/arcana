struct Constants {
    local_to_clip: mat4x4<f32>,
}
var<immediate> constants: Constants;
@group(0) @binding(0) var atlas_sampler: sampler;
@group(0) @binding(1) var atlas: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = constants.local_to_clip * vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(atlas, atlas_sampler, input.uv);
    return vec4<f32>(color.rgb * color.a, color.a);
}
