use arcana_msdf_font::RASTERIZER_WGSL;

#[test]
fn shader_helpers_validate_in_fragment_and_compute() {
    let source = format!(
        "{RASTERIZER_WGSL}\n{}",
        r#"
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let range = arcana_font_screen_range(uv, vec2<f32>(textureDimensions(atlas)), 8.0);
    return vec4<f32>(arcana_font_sample(atlas, atlas_sampler, uv, vec4<u32>(0, 0, 8, 8), range, true));
}

@compute @workgroup_size(1)
fn compute() {
    output[0] = arcana_font_sample(atlas, atlas_sampler, vec2<f32>(0.5), vec4<u32>(0, 0, 8, 8), 4.0, false);
}
"#
    );
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{error:?}"));
}
