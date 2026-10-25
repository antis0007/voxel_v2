# Voxel engine: orientation for agents

Rust + wgpu voxel game: variable voxel scale, cellular water/sand, factory automation, wand magic. One workspace, ten crates.

## Crate map (`crates/`)
- `core` (`engine_core`): FNV hashes, `VoxelScalePreset`, `CHUNK_SIZE`/`floor_div`/chunk coords, `Budget` (time budget for a simulation pass).
- `content` (`engine_content`): `BlockId` and its property table, loaded from `assets/materials.ron`; `SizeClass`.
- `audio` (`engine_audio`): synth, mixer, waveOut thread. No engine dependencies.
- `voxel` (`engine_voxel`): `ChunkData`, `WorldType`, `WorldSampleConfig`, world height constants.
- `worldgen` (`engine_worldgen`): every generator (terrain, trees, sites, dungeons, valleys, undercrown plan). Pure functions of `WorldSampleConfig`; no runtime state.
- `sim` (`engine_sim`): integer 8³-brick cellular world, Margolus kernel, reactions; depends only on `core`. Invoked by the isolated headless CPU-region option; window play remains legacy.
- `sim_gpu` (`engine_sim_gpu`): WGSL port of `sim`, byte-identical to it; `gpu` (`engine_gpu`): the shared test adapter.
- `chip` (`engine_chip`): player-designed microcontrollers: module registry (`assets/chip_modules.ron`), floorplan/nets/vias, bounded router, design checks, characterisation, firmware tasks, runtime state (ADR 0021). The app installs parts in pumps (`world/factory/chips.rs`).
- `app` (binary): `World` hub with simulation, factory, adventure, inventory, storage, regions; meshing; renderer and shaders; egui UI; `headless.rs` scenario runner; `main.rs` loop.

Content lives in `assets/*.ron` (materials, spells, recipes, machines, bestiary) with round-trip tests; scenarios in `assets/scenarios/`.

## Commands (the only ones)
`just check` · `just clippy` · `just test [substring]` · `just test-gpu [substring]` (`gpu_*` tests, needs a GPU) · `just run [scenario.ron]` · `just bench [name]` · `just bench-save` (new baseline) · `just loc` · `just session-report` (end every session with it) · `just worktrees [remove]` (every linked worktree and why it is kept; `remove` drops the clean, idle, branch-saved ones) · `just clean` (build outputs only) · `just verify` (pre-commit gate: check, clippy, tests, `just bench-check` perf gate, LOC). Output is filtered: errors first, warnings as counts, 60 lines max.

Task gate: hooks in `.claude/settings.json` run `scripts/workflow.py`; policy in `.claude/verify/policy.json`. `just gate_status` lists the checks the current diff requires, `just gate` runs and records them (manual ones via `python -I scripts/workflow.py attest`). Path-scoped rules in `.claude/rules/`; `/voxel-improve` runs one bounded improvement pass.

Finishing work in a shared tree: other sessions edit this tree at the same time. Own your scope and finish it: build, test and commit your own files (or work on a branch/worktree), then report. Never wait on, poll for, or block behind another session's or agent's unfinished work; the gate is advisory and does not hold a session open. Checks that fail only because of someone else's files are reported as unverified with the reason, not chased. Subagents return a final result; do not leave background work without a final report. Create scratch worktrees under your session scratchpad, put review evidence outside `target/` of a worktree you will delete, and `git worktree remove` your own before the final report; never remove one you did not create.

## Invariants
- Coordinates: voxel `IVec3` (cell = `VoxelScalePreset::meters()`), chunk `IVec3` (32³ cells, floor division), metres `Vec3` for player/camera/physics. Padded chunk = 34³ (one-voxel halo).
- Tick: fixed 1/120 s (`simulation::STEP_SECONDS`). Both drivers call `simulation::fixed` per step: expedition → factory → player → adventure → tools → cellular/rigid/thermal → day. Legacy wall-time scheduling is not deterministic-equivalence proof. The opt-in `--headless=<path> --ca-cpu-region=x,y,z` path uses 60 Hz CA, exact two-CA-tick publication, sealed boundaries and no legacy writers (ADR 0018); it cannot use ordinary saves.
- Every bounded pass takes a `Budget`; the window passes wall-clock budgets, tests and headless pass `Budget::UNBOUNDED`. No `Instant::now()` in simulation logic.
- Randomness is positional hashing seeded from the world seed (`engine_core::hash`); no `rand` in game code (proptest pulls it into test builds only).
- Saves: magic `VXWSAV34`, `SAVE_VERSION = 34` (34 appends the chip library); `CURRENT_GENERATOR_VERSION = 47` (source checked 2026-10-24). Read the constants in `world/storage.rs` and `voxel/src/config.rs` before any migration; older planning numbers are historical. Old versions must keep generating identically: never edit a historical version gate.
- Shaders are composed in `crates/app/src/shader_sources.rs` and validated by `build.rs`; every `.wgsl` must be listed there (a test enforces it).
- Anything glued to the camera (arm, wand) is textured in view space; small moving geometry is never clipped by the world grid. See `.claude/skills/render/SKILL.md`.

## Writing (UI text, comments, docs, logs)
- All player-visible text follows `crates/app/src/egui_ui/UI_RULES.md`: states and nouns, numbers with units, no prose that explains a screen, no second person, no exclamation marks. If a screen needs a sentence, change the widget. `just lint-ui` reports violations; files listed in `egui_ui/text_rules.rs` are enforced by `just test`, so add a file there once it is clean.
- No rule-of-three lists for rhythm; give the real count. No "not just X, it's Y". No em-dash asides in comments.
- Banned in any text: seamless, robust, leverage, delve, elevate, comprehensive, crucial, ensure, streamline, powerful, elegant. In player text also: amazing, ultimate, epic, unleash, journey, immersive, explore, harness, mysterious, ancient, forgotten, echo, essence, resonance, unless the word is part of a real item or material name.
- No new proper nouns, factions or lore concepts without the owner's approval. Lore lives only on items, blocks or guidebook entries and names something the player can do or observe.
- Comments state why or an invariant, never restate the code. Default to no comment. Doc comments open with what the item does in under 12 words. No closing summary paragraphs.

## Don't
- No `unwrap`/`expect` outside tests and `main.rs` startup; no `Instant` in simulation; no glob re-exports; no new dependency without a line in an ADR.
- No content tables in Rust: add rows to `assets/*.ron`, then run the ignored `dump_*_ron` test if you changed the format.
- No new file over 300 lines when you can split it; no scratch files in the repo root (`repo_layout` test fails).
- No visual change without before/after captures (`docs/design/VISUAL_DIRECTION_EXECUTION.md`).
- Do not touch `egui_ui/wand_xmb.rs` or `adventure/growth.rs` dead code: in-progress work on this branch.

Docs: `docs/ARCHITECTURE.md`, `docs/TESTING.md`, `docs/PLAN.md` (status and debt), `docs/adr/`. Start roadmap work at `docs/MASTER_PLAN.md`; source briefs are preserved in `docs/specs/`. `docs/ENGINE_CORE.md` and `docs/GAME_DESIGN.md` remain detailed proposals, with implementation gaps tracked separately. Recovery provenance and worktree dispositions: `docs/recovery/2026-10-24-inventory.md`.
