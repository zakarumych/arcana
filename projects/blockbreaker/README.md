# Blockbreaker

An Arcana game whose gameplay lives in the `main` plugin and whose authored HUD is rendered by the Arcana Pixie UI plugin. The saved project configuration enables the gameplay and Pixie transform-sync systems, then composites Pixie's transparent image over the Blockbreaker frame before Present.

## Run

From PowerShell:

```powershell
Set-Location C:\code\arcana\blockbreaker\crates
$env:ARCANA_PROFILE = 'debug'
$env:RUSTFLAGS = '-Zshare-generics=off -Cprefer-dynamic=yes'
cargo run -p ed ../blockbreaker.arcana
```

Open **New tool > Scene Editor**, then press **Play** and click inside the game view to focus input.

- **A / D** or **Left / Right**: move the paddle.
- **Space**: launch the ball.
- **R**: restart.
- **Escape**: release editor input focus.

Clear the bricks before losing all three lives. `BlockbreakerFrame` draws the playfield, paddle, ball, and bricks. Pixie draws the title, objective, score, lives, instructions, and game-state prompts. The playfield and 960x720 UI keep their proportions when the view is resized.

All game rules and playfield drawing belong to `main`; the reusable Pixie renderer and root-transform synchronization come from `arcana-pixie-ui`. The project configuration lives in `Arcana.bin` (JSON). The editor's window and tool layout are stored separately in its user configuration. The generated `crates` workspace is managed by Arcana.

## Edit the scene

The Scene Editor seeds one authored layout when the plugin is enabled. Select an object in the scene tree or viewport. Move or rotate it with the gizmo; edit its local `Rect` ranges to change block or paddle geometry. The renderer camera uses the engine's 2D camera projection.

Create an entity and add **Block**, **Paddle**, or a widget component. The add action supplies missing Name, LocalTransform and Rect components and records them as authored. Removing an entity or its game component removes it from drawing and collision. Removing bounds or transforms makes that object inactive until those components are supplied again.

Each widget is a separate component: **TitleWidget**, **ObjectiveWidget**, **ScoreWidget**, **LivesWidget**, **StatusWidget**, and **InstructionWidget**. Runtime maintenance attaches Pixie's Widget, RootWidget, Attributes, and Text components to those same authored entities. Text content comes from each widget's fixed purpose and current game data. Font size selects a cached, integer-scaled 5x7 bitmap font; transform and local bounds remain editable. Widgets can be added or removed independently, and removing an authored widget component removes its runtime Pixie components.

Play uses a copy of the authored scene. Restart resets score, lives, destroyed blocks, and the paddle positions captured from that copy. It does not restore objects deleted in the editor. Paddle controls move along its local horizontal axis; authored placement and rotation are respected without clamping to fixed paddle dimensions.

Validation (2026-09-10): `cargo check -p main --offline`, `cargo test -p main --offline`, and `cargo build -p plugins -p ed --offline` passed from the generated workspace with the dynamic-link flags above. Six unit tests cover rotated local collision bounds, authored block additions/deletions and restart, rotated paddle movement/removal, missing-only component defaults, Pixie widget creation/data updates/removal, and bitmap-font scaling. The regenerated editor launched with both plugins and the saved graph without a startup error; its rendered output was not visually inspected because the window was unavailable to UI automation. Native automated gizmo drags remain inconclusive; separate-frame egui tests cover gizmo dragging. Scene file save/load and undo are not implemented.
