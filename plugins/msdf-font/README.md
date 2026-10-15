# Arcana MSDF font plugin

`arcana-msdf-font` imports TrueType outlines into an `.arcfont` asset containing
an RGBA8 distance-field atlas, per-character glyph bounds and font metrics.
The plugin registers `FontImporter` with target `msdf_font` and source extension
`ttf`. Add the plugin package to an Arcana project's plugin dependencies to
discover its importer. `Font` implements Arcana `Asset`; loading decodes CPU
data and building is an identity operation. Cloning a font shares immutable data.

```rust,no_run
use arcana_msdf_font::{FontImporter, ImportConfig};

let font = FontImporter.import_bytes(&std::fs::read("font.ttf")?, &ImportConfig::default())?;
std::fs::write("font.arcfont", font.to_bytes()?)?;
let glyph = font.rasterize_glyph('A', 96.0)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Run the importer and optionally write a grayscale PGM preview of `A`:

```text
cargo run -p arcana-msdf-font --example import -- font.ttf font.arcfont msdf preview.pgm
```

`ImportConfig` selects `FieldKind::Msdf` (default) or `FieldKind::Sdf`, characters,
pixels per em (default 48), full distance range (default 4 atlas texels), atlas
width (default 512), maximum atlas height (default 4096), and face index (default
0). Default characters are printable ASCII including space. Characters are
sorted and deduplicated. A missing cmap entry is an explicit error; no fallback
glyph is silently substituted. Empty outlines retain their advance with zero
plane/atlas bounds and no raster pixels.

Generation normalizes outlines, orients contours including holes, and uses
deterministic edge coloring and the generator's error correction for MSDF.
Single-channel SDF replicates the distance into RGB. A shelf packer places fields
in sorted character order. Each field includes `ceil(distance_range/2)+1` texels
of padding around rounded outline bounds, with one texel of gutter between fields.
Atlas dimensions are at most 4096, character count at most 4096, source input at
most 64 MiB, pixels per em 1..512 and distance range 1..64. Oversized glyphs or
atlases return an error. No atlas resizing or multiple pages is performed.

## Coordinates and rendering

Font metrics and advances are in em units. `Glyph::plane_bounds` are
`[left, bottom, right, top]`, Y upward relative to the baseline, and include field
padding. `atlas_bounds` are `[x, y, width, height]` in a top-down atlas.
Texture bytes are **linear RGBA8**, never sRGB. Alpha is 255. RGB encodes distance
with the contour at 0.5 and positive distance inside. The full stored range
maps `[-distance_range/2, +distance_range/2]` atlas texels to `[0, 1]`.
The MSDF distance is the median of RGB; SDF uses red.

`Font::rasterize_glyph` bilinearly samples within the glyph rectangle and returns
top-down, one-byte grayscale coverage. `GlyphBitmap::offset` is the pixel-aligned
top-left position relative to the baseline with Y downward; `advance` is in output
pixels. Coverage is `clamp((distance-0.5)*max(full_screen_range,1)+0.5,0,1)`,
where `full_screen_range = distance_range * output_pixels_per_em / font.pixels_per_em()`.
Raster scale accepts 1..4096 pixels per em and dimensions are limited to 4096.

For GPU rendering, `font.upload(device, encoder, mev::PipelineStages::FRAGMENT_SHADER)` creates and uploads a linear
RGBA8 sampled `mev::Image2D`. Append `RASTERIZER_WGSL` to a render Job's shader
source. The snippet supplies distance decoding and coverage functions; it owns no
bindings or entry points. Bind the atlas with a linear clamp sampler and use
`arcana_font_coverage(sampled_rgba, full_screen_range, is_msdf)` with a boolean
mode (`false` SDF, `true` MSDF). See the shader source for derivative-based sampling
helpers and exact argument order. The Job owns its layout, glyph quads, resource
lifetime, submission and synchronization. Sample only the glyph rectangle with
the documented padded quad bounds; ordinary atlas filtering needs no mipmaps.

For example, inside a fragment entry point, with `uv` interpolated across the
padded quad and the atlas rectangle supplied by the Job:

```wgsl
let range = arcana_font_screen_range(uv,
    vec2<f32>(textureDimensions(atlas)), distance_range);
let alpha = color.a * arcana_font_sample(atlas, atlas_sampler,
    uv, rect, range, is_msdf);
return vec4<f32>(color.rgb * alpha, alpha); // premultiplied alpha
```

This plugin provides individual Unicode glyphs and horizontal metrics, not text
shaping, kerning, ligature selection, bidirectional layout, fallback fonts,
hinting, color glyphs or variable-font axis selection. Import size determines the
available field detail; strong minification is not area-filtered. Importing a
font does not grant permission to redistribute that font or its derived atlas.

## Asset format

Version 1 is little endian with exact byte length, no trailing data:

* Eight magic bytes `ARCFONT\0`.
* Twelve `u32` words: version (1), kind (0 SDF / 1 MSDF), width, height,
  distance range and pixels per em as IEEE-754 `f32` bits, five metrics
  (ascender, descender, line height, underline Y, underline thickness) as `f32`
  bits, and glyph count.
* Each glyph is eleven words (44 bytes): Unicode scalar, glyph ID (must fit
  `u16`), advance bits, four plane-bound floats, four atlas-bound integers.
* Exactly `width*height*4` bytes of top-down RGBA8 pixels.

The decoder validates version, mode, finite bounded metrics, dimensions, byte
counts, sorted unique characters, atlas rectangle bounds and plane/atlas scale
consistency before allocating the pixel buffer. Corrupt input returns `FontError`.

## Attribution and verification

The multi-channel signed-distance technique and reference implementation are by
**Viktor Chlumsky**: [Chlumsky/msdfgen](https://github.com/Chlumsky/msdfgen),
[MIT license](https://github.com/Chlumsky/msdfgen/blob/master/LICENSE.txt).
Generation uses the pure-Rust port
[Briany4717/bymsdfgen](https://github.com/Briany4717/bymsdfgen), crate
`bymsdfgen-core` pinned to 0.1.1, also MIT licensed. Font parsing uses
`ttf-parser` 0.25 directly, with a private adapter for em-normalized outlines.
No C++ compiler or FreeType installation is required. This port is not a claim
of bit-identical upstream output or exhaustive compatibility with every outline.

`cargo test -p arcana-msdf-font --offline` covers an authored portable TTF fixture,
deterministic import, distinct MSDF channels, SDF channel equality, sharp contours,
hole signs, whitespace, quadratic/cubic outline conversion and closure, malformed
assets, default importer configuration, asset
loading and WGSL parsing/validation. The optional native graphics test is run with:

```text
cargo test -p arcana-msdf-font --offline native_upload_and_raster_match_cpu -- --ignored
```

That test uploads a synthetic atlas, renders SDF and MSDF coverage and compares
readback against CPU pixels; it requires an available graphics device.
