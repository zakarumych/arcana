# Blockbreaker

A self-contained Arcana game in the `main` plugin. The saved project configuration enables its update system and input filter and connects its single frame-rendering job directly to Present.

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

Clear the bricks before losing all three lives. The frame includes the playfield, paddle, ball, bricks, score, lives, and game-state prompts. The playfield keeps its proportions when the view is resized.

All game rules and drawing belong to `main`; the project configuration lives in `Arcana.bin` (JSON). The editor's window and tool layout are stored separately in its user configuration. The generated `crates` workspace is managed by Arcana.
