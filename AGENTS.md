# Orientation for agents

Read this first. It records things that are not obvious from the code and have already cost people hours.

## Building and running

- `.cargo/config.toml` sets `target-dir = "target"`, so builds go to `./target`. This overrides a global `~/.cargo/config.toml` that points every project at Pyrenet's shared cache. Confirm with `cargo metadata --format-version 1 --no-deps | grep target_directory`. Older `.exe` copies made by earlier sessions (built when the global cache applied) may sit in `./target/release/`; rebuild before trusting one.
- The disk is nearly full. `./target` can reach 4-16 GB; do not create extra worktrees or `CARGO_TARGET_DIR` copies without need.
- `build.rs` parses and validates every composed WGSL program with naga. A shader mistake fails the *build*, not the run. The program lists are in `src/shader_sources.rs`; a new `.wgsl` file must be added there or it is silently unused.
- The working tree is often edited by several sessions at once. A WGSL error naming code you did not touch is usually a peer's half-saved file: wait a minute and rebuild before fixing it.
- HEAD does not build on its own. Tracked shaders reference files that are still untracked (for example `mood_shared.wgsl`, `underwater_shared.wgsl`). Do not trust a clean checkout of HEAD as a baseline until those are committed.

## Looking at a change (fixed-pose capture)

```powershell
$env:VOXEL_CAPTURE_DIR = "<some dir>"
$env:VOXEL_REVIEW_WAND = "2"          # optional: give the player a tier-2 wand
<cargo target dir>\release\minecraft_style_voxel_prototype.exe --benchmark-world=hills `
  --benchmark-weather=storm --benchmark-capture-frames --benchmark-capture-hud `
  --benchmark-view-chunks=6 --benchmark-width=1280 --benchmark-height=720 `
  --benchmark-terrain-camera=x,z,clearance,yaw,pitch --benchmark-warmup=4 `
  --benchmark-duration=40 --benchmark-review-time=0.45
```

- It writes one `camera.png` after streaming finishes. If `[camera-review] captured` never prints, the run ended before terrain was resident: raise `--benchmark-duration` or lower `--benchmark-view-chunks`.
- Weather presets: `clear`, `partly`, `overcast`, `storm`, `winter`. `--benchmark-review-time` is the fraction of the day (0.5 noon, ~0.96 night).
- `--benchmark-debug=cascades|terrain-shadow|cloud-alpha|cloud-density|cloud-shadow` selects a debug view.
- Several other flags are documented in `docs/MIRAGE_AND_COSMOS_UPDATE.md`.

## Rules that prevent the flicker bugs we have already had

1. **Anything glued to the camera must not be textured or masked in world space.** The first-person arm and wand move with the camera; a world-anchored grain or cell test makes them swim or strobe as you walk. The viewmodel path in `world_shader.wgsl` (`is_viewmodel`, `vm_position`) anchors material patterns to the view.
2. **Never discard or clip small moving geometry using the world voxel grid** (`fract(voxel_pos)`). A 2 cm spell mote lands in a different part of the cell every frame. Fade small pieces by shrinking their geometry (`Model::glow_voxel_with_dissolve`).
3. **Ambient motes (`mood_ambience`) are a pure function of world position and the simulation clock.** One mote per cell, bounded drift inside its own cell, ray walks the cells. Do not sample fixed-depth points along the ray or warp the lattice by the sample position: motes then pop as the camera moves.

## One pixel size

Everything procedural must sit on one pixel grid. Mixed resolutions are what made some blocks look like big chunky tiles beside fine grain.

- **World textures:** `PIXEL_RES` (20 texels per metre, 5 cm) in `world_shader.wgsl`. Every `floor(uv * N)` hash cell, dither and noise lookup uses it. Natural surfaces snap their sample point to texel centres (`q`, `texel_centre`), so smooth noise is flat inside a texel. Do not add another cells-per-metre constant, a `* 0.5` multi-pixel cell, or a "second, broader layer" of larger cells. Make a feature wider by hashing more texels together on purpose, not by changing the lattice.
- **Seams and joints** (planks, brick mortar) are exactly one texel wide. At texel centres a thin smoothstep threshold never fires, so use step thresholds that match the texel size.
- **Ambient motes** (`mood_ambience`) are exactly one native screen pixel at any distance, in a palette colour, with stepped brightness. No halo, no soft edge, no size that changes with range. A glint is a one-pixel plus sign. `pixel` (the angle one pixel subtends) comes from the caller's screen-space derivatives.
- **Wand orbs** are `voxel_sphere`s built from `WAND_PIXEL` cubes. Do not use `Model::orb` / `glow_orb` on the wand: the faceted octahedron looks like a different art style.
- Palette: `assets/pixel_palette.txt`, mirrored in `mood_shared.wgsl` as `MOTE_*` (linear light).

## UI

- The look-at (HWYLA/Jade-style) panel is `src/egui_ui/look_at.rs`. It takes plain data (`LookAt`), reads no world state, and is driven from `egui_ui.rs` where `target_block` is set. The tool readout (`tool_hud.rs`) sits below the compass so the quest label under the compass bar does not run into it.

Known open issues:

- Fixed in the working tree but **not yet committed or A/B-captured**: moving torch lights seed their flicker from time only (`moving` flag in `world_shader.wgsl`, `eval_torch_light`), and wand trail particles fade in over the first sixth of their life (`born` in `mesh/models/wand.rs`). Capture before/after before calling the look settled.
- Scene models (wand, entities) store absolute world positions in `f32`. Far from the origin (several km) the vertex rounding no longer matches the camera's `f64` residual, so thin viewmodel parts can jitter by sub-millimetres. Near the spawn area this is invisible.

## Rendering map

| Area | Where |
| --- | --- |
| Terrain/entity lighting, shadows, materials | `src/shaders/world_shader.wgsl` |
| Sky, sun, rainbow/aurora/meteors, mote field | `sky_shader.wgsl`, `mood_shared.wgsl`, `src/world/mood.rs` |
| Clouds (shape, march, composite) | `cloud_morphology.wgsl` (shape), `cloud_shader.wgsl` (march), `cloud_composite.wgsl` (denoise/upsample), `cloud_shared.wgsl` (layer heights), `weather_volume.wgsl` (rain shafts) |
| First-person wand, arm, hand | `src/mesh/models/wand.rs`; models share `Model` in `src/mesh/models.rs` |
| Held-tool 2D overlay (no wand) | `draw_held_tool_overlay` in `src/egui_ui.rs`; skipped when a wand is held |
| Directional shadow cascades | `src/render/shadow.rs` |

## Docs

- `docs/README.md` is the index. It says which notes are current, which are dated session notes (they describe the build they were written against), and which are superseded. Start with `ATMOS_CLOUD_REWRITE_STATUS.md` for clouds and `ENGINE_AUDIT_AND_REFACTOR.md` for engine layout.
- `docs/legacy/` and `docs/legacy/root-2026-03/` describe **superseded** code (March 2026 cloud fixes and LOD plans). They are kept for history. Do not follow them.
- Save-format constraint: new machines or resources need a save version bump. Prefer modes or fields on existing items.

## Repo hygiene

- The repo root holds only project entries. `src/repo_layout.rs` fails `cargo test repo_layout` on anything else. Scratch files, logs, zips, patches, run launchers, exes and review snapshots go outside the repo. Captures and logs go under `target/`.
- Review evidence: `tools/Review-VisualDirection.ps1` writes to `target/visual-direction/<label>/` and records the git HEAD and a hash of the working-tree state. Never overwrite evidence; choose a new label.
- Every `.wgsl` under `src/` must be included by `src/shader_sources.rs`. The same test enforces this. An unlisted shader is silently unused.
- Code used only by tests is `#[cfg(test)]`, not dead code in the shipping build. Check `cargo check --all-targets` warnings before and after a change. Warnings for items used by no program, test or not, are real dead code.
- `src/fluid/{catalog,containers,machine,network,processes,tank_view}.rs` are untracked and not wired into the game (`cargo check` reports them as dead). Their owner decides whether to finish or remove them. Do not delete or wire them on your own.
- `.cargo/config.toml` is untracked. It pins `target-dir` (see above). Commit it with the rest of the build setup.

@RTK.md

## Local code advisor

- RTK is enabled for this project, overriding the older user-global disabled default. Preserve raw benchmark JSON/traces and recover full failure logs when needed.
- Kev supplies optional local review hints after Rust/WGSL `apply_patch` edits; read `docs/CODE_ADVISOR.md` for setup and manual final-patch review. Its scores never establish correctness or authorize actions. Keep existing test/CI restrictions.
