# Arcana Pixie UI

Add this plugin to the project's plugin list. It reexports `pixie_ui` and
`pixie_ui_mev`; widgets live in the same `World` as Arcana entities. Initialization
preserves an existing `UiState`.

For this checkout, add this entry to the project's `.arcana` manifest, then enable
the plugin in the editor:

```toml
[[plugins]]
name = "arcana-pixie-ui"
description = "Pixie UI widgets and rendering"

[plugins.dependency]
path = 'C:\code\arcana\plugins\pixie-ui'
```

Enable the **Sync Pixie root transforms** system. It synchronizes entities with
`RootWidget`, `Widget { parent: None }`, `Attributes`, and `scene::dim2::Transform`.
The fixed position **offset** maps to Arcana world translation, in pixels with Y
down. Existing anchors and Arcana rotation are preserved; Pixie does not rotate
widgets. First observation takes the Arcana position; automatic positions become
fixed top-left offsets. Each later synchronization adds the Pixie offset delta to
the current Arcana translation, combining concurrent edits. Fractional Arcana
coordinates survive pixel rounding. Changes also update `LocalTransform` when
present. Run scene transform propagation before this system for parent movement;
run this system before propagation when directly editing a root world transform.
Entities that stop qualifying lose their synchronization baseline.

The **Pixie UI** job exports a `SampledImage2D` named `Image`. Integer `width` and
`height` parameters default to 320 and 180 for nonpositive values. The job owns its
renderer and public `style`; `renderer_mut()` exposes initialized texture
registration after its first execution. It updates the UI rectangle, synchronizes
parents, resolves style/layout, then clears and renders RGBA8 transparent output.
Connect it to Arcana's built-in image overlay job to composite into a viewport.
Draw uploads, clear, and draw commands share the graph encoder. Unconsumed outputs
are not allocated; an external image must match configured dimensions, format,
and usage. Input event routing remains the application's responsibility.

In the render graph, connect `Image` to **Built-in / Overlay image**'s `Source`,
connect an initialized scene image to `Target`, and connect the updated output to
Present. `Scale` selects Stretch, Width, Height, Min (contain), or Max (cover).
The image is centered and sampled with nearest filtering. `Integer scale` floors
the scale with a minimum of one; targets smaller than the source therefore crop.
Source colors must use premultiplied alpha, as Pixie's output does.

Run CPU tests with `cargo test -p arcana-pixie-ui --offline`. The native GPU test
requires a graphics device:

```text
cargo test -p arcana-pixie-ui --test gpu --offline -- --ignored
```

It verifies compositing, clearing an empty frame, and lazy glyph-atlas uploads.
