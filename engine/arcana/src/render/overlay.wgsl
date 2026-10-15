struct Constants { size: vec2<f32> }
var<immediate> constants: Constants;
@group(0) @binding(0) var image_sampler: sampler;
@group(0) @binding(1) var image: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let uv = corners[index];
    var output: VertexOutput;
    output.position = vec4((uv * 2.0 - 1.0) * constants.size * vec2(1.0, -1.0), 0.0, 1.0);
    output.uv = uv;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, input.uv);
}
