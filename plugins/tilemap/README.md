# Tilemap assets, importers, and rendering

The `tilemap` plugin registers `MapImporter` targeting `Map` and `TilesetImporter`
targeting `Tileset`, both JSON source importers. Enable the plugin to expose them
in the editor Assets tool. Importing a map requests its tileset; importing a
tileset requests its image as a `Texture` dependency; the asset store imports that
image with the built-in image importer and persists the assets and dependency links.

Source JSON follows `assets/tilemap/tileset.json`:

```json
{
  "image": "atlas.png",
  "tile_size": [16, 16],
  "tiles": [
    {"name": "ground", "region": [0, 0, 16, 16]},
    {"name": "platform", "region": [16, 0, 16, 16]}
  ]
}
```

`image` is a source path resolved relative to the tileset source by the engine
asset store. Imported JSON has the same shape, with `image` replaced by the
texture's numeric `AssetId`. `TilesetData` and `TileData` describe this artifact.
`tile_size` contains positive finite world-space sizes. Regions use top-left
atlas texel coordinates `[x, y, width, height]` with nonzero sizes and checked
extents. Tile names must be nonblank, contain no control characters, and be
unique. Unknown fields and non-unit importer configuration are rejected.

Map source JSON follows `assets/tilemap/map.json`:

```json
{
  "tileset": "tileset.json",
  "cells": [[1, 1, 1], [0, 2, 0]]
}
```

`tileset` is a nonblank source path without control characters, resolved relative
to the map source. The store recursively imports it as `Tileset`, including its
texture dependency. Imported JSON replaces `tileset` with its numeric `AssetId`;
`MapData` describes this artifact. `cells` must be a nonempty rectangular grid of
unsigned 32-bit integers. Rows remain in source order, bottom to top; zero is
empty and positive values index tiles in descriptor order starting at one.
Unknown fields and non-unit configuration are rejected. The importer does not
read dependency artifacts, so it does not validate cell indices against the
tileset's tile count.

The importer does not read the texture artifact or check region bounds against
image dimensions. Runtime loading performs these dependency-dependent checks.

`Map` and `Tileset` implement `Asset` and `AssetVersion` (version 1), with stable
type names `tilemap::Map` and `tilemap::Tileset`. Request a map from the shared
cache using `Assets::get::<Map>(map_id)` or `Assets::poll::<Map>(map_id, cx)`.
`Map::data()` exposes its imported ID and cells; `Map::tileset()` exposes the
resolved tileset. `Tileset::data()` exposes its imported texture ID, tile size,
and descriptors; `Tileset::texture()` exposes the resolved built `Texture`.
Clones share immutable storage and retain their resolved dependencies.

Runtime assets decode the imported JSON containing numeric IDs, validate local
structure before requesting dependencies, and resolve dependencies through
`Assets::poll`. A tileset waits for its texture to finish building and validates
every region against the image dimensions. A map waits for its tileset and
rejects cells exceeding the tile count. Decode, missing-dependency, and build
errors propagate to the requesting asset. Source paths belong to importing and
are rejected by runtime decoding.

The host must enter its Tokio runtime and pump the shared asset build queue
through `AssetBuildContext::build_assets(assets, gpu)` while requests are pending.
Each dependency and parent passes through that queue, including the map and
tileset's pass-through build steps. The plugin creates no GPU or runtime.
Existing handles retain their data after cache invalidation; requesting again
resolves dependencies through the refreshed cache. ECS unfolding is not
implemented by this plugin.

The registered `TileMap` render job draws runtime `Map` components spawned in
the world and updates one `Target` image. Connect an initialized color image, for example from
`NewColorImage`, to its input and connect its updated output to the next job.
The supplied world must contain a `CurrentRenderer` selecting an entity with
`scene::dim2::Camera`. The camera
helper includes that entity's optional 2D `Transform` and fits the target aspect
ratio. Missing or invalid camera data and a world without maps leave the target
unchanged. Each map entity's optional 2D `Transform` places that map in the world.
Map loading and spawning belong to the host's asset and scene lifecycle.
Maps draw in ascending entity-ID order; later IDs composite over earlier ones.

The map's top-left corner is at local `(0, 0)`, with X right and Y down.
The map entity's transform places this rectangle in the world.
Source row zero is the bottom row. Each cell occupies its tileset's world-space
tile size, independently of the texel dimensions of its atlas region. Empty
cells and pixels outside the map preserve the existing image. Atlas sampling
uses nearest filtering; straight texture alpha is premultiplied for source-over
blending. The job queries map components each execution and retains only GPU
pipeline and sampler state.

Planning suggests RGB8 and requires render-target usage while preserving the
connected image's format and extent. Compatible RGB8/RGBA8 UNORM and sRGB targets
use a pipeline for their actual format; the job does not convert the target.
The local Vulkan GPU reports no RGB8 color-attachment support, so native
rendering validation uses RGBA8. RGB8 execution requires a supporting device.

Focused validation:

```powershell
$env:ARCANA_PROFILE = 'debug'
$env:RUSTFLAGS = '-Zshare-generics=off -Cprefer-dynamic=yes'
cargo test -p tilemap --offline --locked
cargo test -p tilemap --test render native_tilemap_pixels_follow_renderer_camera --offline --locked -- --ignored --nocapture
cargo test -p tilemap --test runtime native_imported_map_resolves_built_dependencies_and_reloads --offline --locked -- --ignored --nocapture
cargo test -p arcana-ed --lib tileset_import_resolves_texture_dependency --offline --locked
cargo test -p arcana-ed --lib map_import_resolves_tileset_and_texture_dependencies --offline --locked
```

The plugin tests cover sample conversion, missing-dependency retries, full-width
asset IDs, malformed sources/imported artifacts, configuration rejection, plugin
registration, and terminal dependency errors waking map waiters without a GPU.
The editor tests exercise the real asset store and built-in texture importer,
including nested paths, recursive map dependencies, dependency reimport, stable
IDs, and failure publication.
The ignored native runtime test imports the sample map, tileset, and atlas,
builds their dependency chain through one shared GPU, checks cache identity,
region/cell preservation, bounds/index rejection, and cache invalidation/reload.
It flushes the asset encoder and waits for queue completion before GPU teardown.
It checks texture creation and dimensions; it does not read back pixels or
exercise the editor UI.

Validation on 2026-10-15 passed all seven device-free plugin tests and the
explicit native runtime test. Native Vulkan teardown emitted
`FreeListAllocator has free blocks on drop. Allocator should be cleaned` after
cache/helper cleanup and queue completion; it emitted no outstanding-memory-block
diagnostic. This run does not establish diagnostic-free backend teardown.

TileMap validation on 2026-10-15 passed nine device-free tests and the explicit
native render test above. Native RGBA8 pixel readback checks atlas orientation,
source row order, straight-alpha blending, preserved background/empty cells,
camera pan/zoom/rotation/aspect fitting, renderer ownership, transformed map
entities, multiple and overlapping instances, and missing/invalid camera or
absent maps. The render world contains no `Assets` resource. The native run
emitted the same free-list cleanup warning. RGB8 execution and interactive
editor use were not tested.
