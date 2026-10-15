// Distance-field decoding follows Viktor Chlumsky's msdfgen reconstruction.
// https://github.com/Chlumsky/msdfgen
// Distances are positive inside; 0.5 is the outline in both field modes.

fn arcana_font_median(value: vec3<f32>) -> f32 {
    return max(min(value.r, value.g), min(max(value.r, value.g), value.b));
}

fn arcana_font_distance(sampled: vec4<f32>, is_msdf: bool) -> f32 {
    return select(sampled.r, arcana_font_median(sampled.rgb), is_msdf);
}

// screen_range is the full encoded distance span measured in screen pixels.
fn arcana_font_coverage(sampled: vec4<f32>, screen_range: f32, is_msdf: bool) -> f32 {
    let distance = arcana_font_distance(sampled, is_msdf) - 0.5;
    return clamp(distance * max(screen_range, 1.0) + 0.5, 0.0, 1.0);
}

// Fragment-only. Call in uniform control flow using UVs before glyph clamping.
// atlas_range is the full encoded distance span measured in atlas texels.
fn arcana_font_screen_range(uv: vec2<f32>, atlas_size: vec2<f32>, atlas_range: f32) -> f32 {
    let footprint = max(fwidth(uv), vec2<f32>(1e-8));
    return 0.5 * dot(vec2<f32>(atlas_range) / atlas_size, vec2<f32>(1.0) / footprint);
}

// rect = (x, y, width, height) in atlas texels. Empty glyphs have no coverage.
// Explicit LOD allows compute use and avoids derivative-uniformity requirements.
fn arcana_font_sample(
    atlas: texture_2d<f32>,
    atlas_sampler: sampler,
    uv: vec2<f32>,
    rect: vec4<u32>,
    screen_range: f32,
    is_msdf: bool,
) -> f32 {
    if any(rect.zw == vec2<u32>(0u)) {
        return 0.0;
    }
    let size = vec2<f32>(textureDimensions(atlas));
    let first = (vec2<f32>(rect.xy) + vec2<f32>(0.5)) / size;
    let last = (vec2<f32>(rect.xy + rect.zw) - vec2<f32>(0.5)) / size;
    let sampled = textureSampleLevel(atlas, atlas_sampler, clamp(uv, first, last), 0.0);
    return arcana_font_coverage(sampled, screen_range, is_msdf);
}
