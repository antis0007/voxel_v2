# Voxel engine: orientation for agents

Rust + wgpu voxel game: variable voxel scale, cellular water/sand, factory automation, wand magic. One workspace, nine crates.

## Crate map (`crates/`)
- `core` (`engine_core`): FNV hashes, `VoxelScalePreset`, `CHUNK_SIZE`/`floor_div`/chunk coords, `Budget` (time budget for a simulation pass).
- `content` (`engine_content`): `BlockId` and its property table, loaded from `assets/materials.ron`; `SizeClass`.
- `audio` (`engine_audio`): synth, mixer, waveOut thread. No engine dependencies.
- `voxel` (`engine_voxel`): `ChunkData`, `WorldType`, `WorldSampleConfig`, world height constants.
- `worldgen` (`engine_worldgen`): every generator (terrain, trees, sites, dungeons, valleys, undercrown plan). Pure functions of `WorldSampleConfig`; no runtime state.
- `sim` (`engine_sim`): integer 8³-brick cellular world, Margolus kernel, reactions; depends only on `core`. Not yet wired into the game.
- `sim_gpu` (`engine_sim_gpu`): WGSL port of `sim`, byte-identical to it; `gpu` (`engine_gpu`): the shared test adapter.
- `app` (binary): `World` hub with simulation, factory, adventure, inventory, storage, regions; meshing; renderer and shaders; egui UI; `headless.rs` scenario runner; `main.rs` loop.

Content lives in `assets/*.ron` (materials, spells, recipes, machines, bestiary) with round-trip tests; scenarios in `assets/scenarios/`.

## Commands (the only ones)
`just check` · `just clippy` · `just test [substring]` · `just test-gpu [substring]` (`gpu_*` tests, needs a GPU) · `just run [scenario.ron]` · `just bench [name]` · `just bench-save` (new baseline) · `just loc` · `just session-report` (end every session with it) · `just clean` (build outputs only) · `just verify` (pre-commit gate: check, clippy, tests, `just bench-check` perf gate, LOC). Output is filtered: errors first, warnings as counts, 60 lines max.

## Invariants
- Coordinates: voxel `IVec3` (cell = `VoxelScalePreset::meters()`), chunk `IVec3` (32³ cells, floor division), metres `Vec3` for player/camera/physics. Padded chunk = 34³ (one-voxel halo).
- Tick: fixed 1/120 s (`simulation::STEP_SECONDS`). Order per step: expedition → factory → player → adventure → cellular/rigid/thermal → day cycle. Headless (`headless::Session::tick`) and `main.rs` must agree.
- Every bounded pass takes a `Budget`; the window passes wall-clock budgets, tests and headless pass `Budget::UNBOUNDED`. No `Instant::now()` in simulation logic.
- Randomness is positional hashing seeded from the world seed (`engine_core::hash`); no `rand` in game code (proptest pulls it into test builds only).
- Saves: magic `VXWSAV28`, `SAVE_VERSION = 28`; `CURRENT_GENERATOR_VERSION = 35`. Old versions must keep generating identically: never edit a historical version gate.
- Shaders are composed in `crates/app/src/shader_sources.rs` and validated by `build.rs`; every `.wgsl` must be listed there (a test enforces it).
- Anything glued to the camera (arm, wand) is textured in view space; small moving geometry is never clipped by the world grid. See `.claude/skills/render/SKILL.md`.

## Don't
- No `unwrap`/`expect` outside tests and `main.rs` startup; no `Instant` in simulation; no glob re-exports; no new dependency without a line in an ADR.
- No content tables in Rust: add rows to `assets/*.ron`, then run the ignored `dump_*_ron` test if you changed the format.
- No new file over 300 lines when you can split it; no scratch files in the repo root (`repo_layout` test fails).
- No visual change without before/after captures (`docs/design/VISUAL_DIRECTION_EXECUTION.md`).
- Do not touch `egui_ui/wand_xmb.rs` or `adventure/growth.rs` dead code: in-progress work on this branch.

Docs: `docs/ARCHITECTURE.md`, `docs/TESTING.md`, `docs/PLAN.md` (status and debt), `docs/adr/`. Roadmap proposal (pending approval): `docs/ENGINE_CORE.md`, `docs/GAME_DESIGN.md`.
