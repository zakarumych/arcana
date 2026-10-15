# Arcana roadmap

Draft for prioritization · 2026-09-12

## Target

Release the first **3D multiplayer game built with Arcana**. Completion means
players can obtain the released game, connect from separate machines, and play
its complete intended game loop together.

Engine work serves that release. The game is the acceptance project for 3D
rendering, content authoring, networking, runtime, packaging, and operation.
Blockbreaker remains a useful regression fixture for existing capabilities.

Game concept, player count, session model, target platforms, distribution channel,
and hosting requirements must be settled in M0. Milestones describe outcomes;
implementation boundaries, network authority, and persistence formats need
separate design decisions.

## Starting point

The current foundation includes plugin registration and loading, ECS systems and
flows, editable system/input/render graphs, reflected scene components,
hierarchies, picking and transform gizmos, and independent Play/Stop sessions.
Blockbreaker exercises gameplay, authored objects, and Pixie UI rendering.
Reusable CRT and distance-field font plugins also exist.

The complete workflow is unfinished:

- Authored scene snapshots support Play and plugin reload, but have no scene-file
  save/load path. Editor configuration persistence does not save authored scenes.
- Undo is absent. Native interaction and reload coverage have remaining gaps.
- Source importing and runtime asset loading have separate infrastructure, but
  durable import/reopen and editor asset selection remain incomplete.
- Generated games call an absent `arcana::game::run` entry point; cooking is
  unimplemented.
- Local sibling dependencies and generated-workspace validation complicate a
  fresh developer setup.

These are source/documentation findings, not a fresh build or runtime validation.
See [project knowledge](PROJECT_KNOWLEDGE.md) for contracts and dated evidence.

## Milestones

| Order | Outcome | Completion demonstration |
| --- | --- | --- |
| M0 | Define the release game | Agree on its core loop, multiplayer requirements, platform, and release scope. |
| M1 | Preserve authored work | Edit a scene, save, exit, reopen, and play the same scene. |
| M2 | Play a standalone 3D slice | Play the game's smallest local loop in an authored 3D level outside the editor. |
| M3 | Prove multiplayer | Players on separate machines complete that loop together under representative network conditions. |
| M4 | Produce game content reliably | Import and edit the game's assets, undo changes, and rebuild plugins without losing authored work. |
| M5 | Complete the release candidate | External testers install a package, join sessions, and play the complete game. |
| M6 | Release the first 3D multiplayer game | Publish the game and operate its multiplayer access for real players. |

### M0 — Define the release game

- [ ] Choose the core game loop and smallest complete release scope.
- [ ] Define cooperative/competitive play, players per session, session length,
  joining/leaving, and any persistent progression.
- [ ] Choose release platforms, distribution channel, minimum hardware, and
  measurable frame-time and network-condition targets.
- [ ] Set hosting, session discovery, identity, and trust requirements; decide
  whether dedicated server operation is needed.
- [ ] Identify essential 3D content, movement/collision, audio, UI, and animation.

Done: a short game brief supplies concrete acceptance criteria for M2–M6.
Networking feasibility work starts here; full content production follows proof
that the multiplayer loop works.

### M1 — Preserve authored work

- [ ] Define scene-file identity/versioning and its relationship to project
  configuration, renderer graphs, and plugin dependencies.
- [ ] Save/load authored entities, component values, hierarchy, and renderer
  ordering; preserve empty authored entities and deliberately removed components.
- [ ] Reserve loaded entity IDs before allocating new entities.
- [ ] Define handling of unavailable components/plugins and incompatible values;
  failed loading must preserve the open scene and source file.
- [ ] Add explicit Save/Open actions, dirty state, and unsaved-work handling.
- [ ] Make failed/interrupted saves recoverable without destroying the last
  valid saved scene.

Done: an authored scene, including 3D transforms, hierarchy, camera, and renderer
selection, survives a full editor restart and reconstructs correctly. Round-trip and failure tests
cover references, IDs, missing descriptors, and invalid files; native save/reopen
is demonstrated.

Entry points: `ed/src/{scene_editor,entity_ids,project,app}.rs`,
`engine/plugin/src/{component,scene}.rs`.

### M2 — Play a standalone 3D slice

- [ ] Establish the runtime host needed to initialize plugins, load authored
  content/configuration, tick systems and flows, route input, and render.
- [ ] Use the engine's plugin, scene, scheduling, input, and work-graph
  capabilities for the game host.
- [ ] Make generated game source and launcher build/run commands agree with the
  implemented runtime, including debug/release artifact paths.
- [ ] Load the same saved gameplay configuration and scene used in the editor.
- [ ] Supply reusable 3D rendering for the slice's geometry, camera, depth,
  materials, and required lighting through engine render jobs.
- [ ] Implement the game's minimal movement, interaction/collision, objective,
  and session reset; use placeholder content where sufficient.
- [ ] Verify native 3D picking, manipulation, camera interaction, and Play.
- [ ] Verify standalone start, input, resizing, the core loop, and clean shutdown.

Done: the release game's smallest local loop builds and plays in a saved 3D level
outside the editor in debug and release, with matching editor Play behavior.

Depends on M0 and M1. Entry points: `engine/project/src/{generator,wrapper}.rs`,
`engine/arcana/src/{lib,render}.rs`, `engine/arcana/src/scene/dim3.rs`,
`ed/src/{simulation,render_view,scene_viewport}.rs`, `launcher/src/cli.rs`.

### M3 — Prove multiplayer

- [ ] Decide network authority, simulation ownership, transport, replicated
  identity/state, and compatibility rules against the M0 game requirements.
- [ ] Implement connection/session lifecycle and required discovery or direct
  joining, including actionable connection failures.
- [ ] Synchronize player actions, entity creation/destruction, and shared game
  outcomes; provide interpolation/prediction/reconciliation where the game needs it.
- [ ] Define and exercise late join, disconnect, host/server loss, and reconnect
  behavior according to the game's session rules.
- [ ] Validate incoming data and enforce authority at the receiving boundary.
- [ ] Test the target player count, latency, jitter, packet loss, bandwidth, and
  session duration; include separate-machine internet play.

Done: players complete the M2 loop together on separate machines under the
agreed network conditions, with consistent shared outcomes and defined failure
recovery. Test headless operation if the chosen hosting model requires it.

Depends on M0 and M2. Network feasibility prototypes may run alongside M1/M2.
Implementation entry points and plugin ownership follow architecture review;
the roadmap does not assume an existing operational networking subsystem.

### M4 — Produce game content reliably

- [ ] Complete importer dependency/retry handling and consistent source metadata,
  artifact registration, and registry reconstruction after restart.
- [ ] Connect asset browsing/import configuration and typed asset selection to
  the editor authoring workflow.
- [ ] Complete the game's 3D model, material, texture, and font paths through
  import, runtime load/build, and rendering; add animation/audio where required
  by the game brief.
- [ ] Make changed sources reimport with stable references and visible errors.
- [ ] Make the standalone host resolve the same authored asset references.
- [ ] Undo/redo entity, component, value, hierarchy, and gizmo edits; keep dirty
  state and history coherent across save/load, Play/Stop, and plugin reload.
- [ ] Diagnose invalid system/render graphs and unsupported editable values
  without hangs or panics in those authoring paths.
- [ ] Verify successful and failed native plugin rebuild/reload, preserving
  authored work and stopping running previews on accepted reload.
- [ ] Verify sustained native 3D editing, viewport focus, multiple previews,
  resize, and tool/window closure.

Done: produce a representative level for the release game, import and assign its
assets, save/reopen, change sources, reimport, and play the result in editor and
multiplayer builds. Undo and failed loads/rebuilds preserve authored work.
Missing and corrupt assets produce useful diagnostics.

Depends on M1/M2; multiplayer proof depends on M3. Entry points:
`ed/src/assets/`, `engine/assets/src/`, `engine/graphics/src/texture.rs`,
`plugins/msdf-font/`, `ed/src/{scene_editor,scene_tree,scene_viewport,model}.rs`,
`ed/src/plugins/{manager,container}.rs`.

### M5 — Complete the release candidate

- [ ] Complete the scoped gameplay, levels, onboarding, menus, settings, audio,
  and session flow from the game brief.
- [ ] Implement cooking/packaging of runtime configuration, scenes, asset
  artifacts, executable, and required runtime libraries.
- [ ] Remove development-machine paths from the distributed game's requirements.
- [ ] Provide a reproducible developer setup with an explicit toolchain and
  dependency acquisition instructions or automation.
- [ ] Automate focused core checks plus generated editor/plugin/game builds.
- [ ] Document project creation, plugin creation, authoring, Play, and packaging
  using commands exercised on the acceptance project.
- [ ] Package and deploy any server/session services required by the hosting
  model; test client/server version compatibility and update behavior.
- [ ] Meet agreed performance and network budgets on target hardware with the
  intended player count and representative session duration.
- [ ] Have external testers install on clean target machines without Rust or the
  editor, connect over the internet, and complete the full multiplayer game loop.

Done: a distributable release candidate passes complete multiplayer playtests,
including session failures and recovery, with no release-blocking defects.
Root workspace checks alone do not satisfy this gate.

Depends on M3 and M4. Entry points: `launcher/src/cli.rs`,
`engine/project/src/`, project/asset persistence, and the release game's plugins.

### M6 — Release the first 3D multiplayer game

- [ ] Publish the game through the chosen distribution channel with installation,
  multiplayer access, and support instructions.
- [ ] Make required multiplayer services or player-hosting tools available.
- [ ] Verify the published build with players on separate machines.
- [ ] Establish crash/error reporting, an update/recovery procedure, and ownership
  of any hosted services for the intended release audience.

Done: the first Arcana 3D multiplayer game is released, obtainable by its intended
audience, and playable together through its supported multiplayer path.

Depends on M5. Publishing and external service changes require authorization at
execution time; this roadmap defines the release outcome.

## Scope control

Visual scripting, general-purpose prefab tooling, extra platforms, and live
runtime-state migration enter the release scope only when the chosen game needs
them. Rendering, networking, content, and performance work are measured against
that game's requirements. No milestone requires restoring every inactive plugin.

## Execution rules

- Pull the next task from the earliest unfinished milestone; choose a slice with
  an observable user outcome and a bounded completion check.
- Review real API, persistence, and ownership decisions before implementation.
- Reuse engine capabilities in plugins. Keep game-specific rules in game plugins.
- Validate generated projects separately from the root workspace. Report unit,
  build, GPU-readback, and native-interaction evidence separately.
- Mark a milestone complete only when its demonstration passes. Keep dates and
  estimates unset until scope is agreed and the first slices establish pace.

Recommended next task: write the M0 game brief, particularly the core loop,
player count, and session/hosting requirements. Then settle M1's scene-file
semantics while investigating the highest-risk multiplayer requirements.
