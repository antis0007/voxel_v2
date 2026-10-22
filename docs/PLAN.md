# Revamp plan (approved 2026-10-22)

Audit numbers: `AUDIT.md`. Each phase lists LOC target, risk and its verification command. Actual deltas are appended to each phase as it lands.

Baseline: 121,941 Rust + 9,356 WGSL + 12,858 docs. Target after Phase 5: **≈ 96k Rust (−26k, −21%)**, ≈ 8.3k WGSL (−1k), ≈ 1.5k docs (−11k). Honest range: −20k to −30k Rust; the big unknowns are how much of the 5k review harness and 2.2k fluid stack the user keeps.

Deviations from the requested target (justified by the graph in §C):
- `world/` splits into **voxel** (storage/streaming/regions/save), **worldgen** (14k of generators; not in the requested list, too big and too separable to fold into `voxel`), **automation** (factory/inventory/expedition/habitats), **adventure** (requested name `magic`; it is spells + enemies + bosses + combat, one deterministic subsystem, 16.5k) and **sim** (World struct + tick order + weather/day cycle).
- **ui** crate (egui, 16k) separate from `app`; `app` stays thin as requested.
- `physics` = `simulation/` + `fluid/ripples` + `world/water`.
- `content` = loaders + validation; data in `assets/*.ron`.
- Keep a single `[workspace]` at root, crates under `crates/`.

### P0 — Freeze and baseline (prereq; no refactor)  · LOC 0 · risk: high if skipped
1. Confirm no other session is editing (`AGENTS.md` warns of concurrent edits). Refactor is blocked until the tree is single-writer.
2. Commit the working tree as the baseline (97 untracked src files, 26k LOC; else every later diff is unreviewable). Move root junk (`reviews/`, `review-artifacts/`, zips, patches, stale `horizon.rs`/`world_shader.wgsl`) **outside the repo** (not deleted). Commit `.cargo/config.toml`. Add `rust-toolchain.toml` (1.93.1).
3. Record timings: `cargo check`, `check --all-targets`, `test --no-run`, `nextest run`, `build --release` (cold run deferred — 14 GB target, 16 GB free; measure once, not repeatedly).
4. Green the suite before any refactor: diagnose the 5 non-layout failures (3 storage, 1 audio, 1 causeway). If a failure is a real regression it is fixed as its own commit; if it is a stale expectation it is updated with the reason in the commit. No `#[ignore]` without a linked debt entry.
Verify: `just test` (all 414) green; `cargo test repo_layout` green; `git status --porcelain | wc -l` == 0.

### P1 — Agent tooling  · LOC +300 (scripts) · risk: low
- `justfile`: `check`, `clippy`, `test`, `bench`, `run`, `loc`, `verify` as specified. Single crate until P3, so `[crate]` arg is optional and ignored until then.
- `scripts/filter.sh` (dedupe, strip ANSI, collapse notes, cap 60). PowerShell is the primary shell on this machine: provide `scripts/filter.py` as the implementation and let `filter.sh` be a thin wrapper (Git Bash exists). **Open question for user: python vs pure sh**.
- `.cargo/config.toml`: `-Clinker=rust-lld` on `x86_64-pc-windows-msvc` (ships with rustc, no install), `incremental = true` (already default in dev), keep existing opt levels. `mold`/`sccache`/cranelift not available on Windows; record as not applicable. Measure before/after.
- `.claude/settings.json`: PostToolUse(Edit|Write) → `just check`; Stop → `just verify`. Mirror `.codex/hooks.json` intent, then delete the `.codex` hook with the absolute `D:\AI` venv path or move it under `.claude/`.
- Tools to install (user machine, not repo): `just`, `tokei`, `cargo-machete`. Note in PLAN.
- Feature gates: `render`, `audio` as Cargo features now; real isolation comes with crates in P3.
- Kill the 114-warning replay: fix or `cfg(test)`-gate the trivial ones in this phase so `just check` output is empty on a clean tree (prerequisite for hooks being useful).
Verify: `just verify` green; `just check` prints 0 lines on clean tree; timing table in PLAN.

P1 measurements (2026-10-22, warm `target/`, `touch src/fast_hash.rs` before each; best-of-2 steady passes after one discarded first pass):

| Command | link.exe (before) | rust-lld (after) |
|---|---|---|
| `cargo check --all-targets` | 1.7–2.2 s | 2.0 s |
| `cargo test --no-run` | 4.4–5.4 s | 3.6–3.9 s |
| `cargo build` (dev) | 16.2–22.0 s | 3.9–4.1 s |
| full rebuild (`cargo build` after config change) | 1 m 54 s | 1 m 33 s |
| `cargo nextest run` (413 tests) | 17.7 s | 5.3 s (after the shared-lock fix; the 25 s timing test is ignored) |
| `cargo clippy --all-targets` (incremental) | 12.2 s | — |

P1 actuals: Rust 121,941 → 119,864 lines (−2,077: unused fluid stack −2,209, clippy fixes −185 net, scripts +~150 outside `src/`). WGSL unchanged. `just verify` runs in ~20 s warm (check 2 s, clippy 12 s, test 5 s). `just check` prints one line on a clean tree (63 dead-code warnings collapsed; they are the P4 list). `just test` collapses the 48 test-profile warnings the same way.

Not applicable on this machine: `mold` (Linux only), `sccache`/cranelift (not installed; no shared cache to gain from on one workstation). `incremental` is already on for `dev`. Feature-gating `render`/`audio` inside one crate buys nothing (any edit rechecks the whole crate), so that moves to the P3 workspace split instead of adding stub modules now. `cargo machete`: no unused dependencies.

### P2 — Headless deterministic engine  · LOC −500 net (replaces env-var review hooks) · risk: medium
- `--headless`: build `World` + `Simulation` + `Factory` + `Adventure` without `Gpu`/winit; requires separating the per-step gameplay loop from main.rs:3074 into a `sim::tick(world, input, steps)` fn (move, not logic change).
- Fix the two wall-clock-dependent paths (extraction deadline, expedition budget) → per-tick work counts. Run factory/expedition per step, not batched `dt` (gameplay-visible; guard with a scenario test first). **Stop-and-ask item**: this alters gameplay timing.
- One `Rng` (splitmix64) in `core`, seeded from world seed + tick; replace 8 ad-hoc hashes where used for randomness (keep positional hashes for noise).
- Scenario runner: `assets/scenarios/*.ron` → actions (place, cast, build, advance N) + assertions; JSON summary; nonzero exit on failure. `insta` snapshots of world-state hash per scenario. Offscreen wgpu render → PNG hash only for the existing fixed-pose capture path (`--benchmark-capture-frames`), gated by a `gpu-tests` feature.
- Invariant checkers (debug): chunk/LOD residency consistency, item conservation per factory tick, no NaN in rigid/thermal.
- Design note (from reading the code, 2026-10-22): the whole `WorldSimulation::update` (src/simulation/world.rs) is wall-clock budgeted: `FRAME_BUDGET` 2.5 ms, `FACTORY_BUDGET` 0.75 ms, `RIGID_BUDGET` 0.6 ms, `water_budget(pending)`, plus `Instant`-based deadlines threaded into `factory::extraction::tick_world` and a 2 ms cap in `expedition.rs`. The live game must keep these (frame pacing), so the fix is one `Budget` value passed down: `Budget::Wall(deadline)` in the windowed loop, `Budget::Unbounded` in headless and tests. `Instant::now() < deadline` checks become `budget.has_time()`. Behaviour of the windowed game is unchanged; headless runs do the same work every time. The per-frame order to reproduce in `sim::tick` is: `expedition.tick(dt)` → `factory::step` → `factory.tick(dt)` → N × `update_player(STEP)` → `adventure::tick(dt)` → `activity.merge(seeds)` → `WorldSimulation::update(dt)` → `day_night.advance(dt)` (main.rs:3032–3731).
- Deps to add: `ron`, `serde`, `insta` (dev), `criterion` (dev). **Dependency decision for user.**
- `docs/TESTING.md` (1 page).
Verify: `just run assets/scenarios/smoke.ron` exit 0; `just test` green; snapshot of 3 scenarios committed.

P2 actuals (2026-10-22): done — `Budget` threaded through every wall-clock-bounded pass (windowed behaviour unchanged, tests and headless unbounded); `WorldSimulation::settle`; `src/headless.rs` runner (`--headless=<file>`, `just run`), three scenarios (`smoke`, `sand_falls`, `water_falls`) with insta snapshots and a run-twice determinism test; `docs/TESTING.md`. Rust LOC 120288 (headless runner +330; the review-hook deletions that pay for it are P4). Not done, carried forward: criterion benches and offscreen PNG hashes need a library target (P3); invariant checkers beyond `Finite` (item conservation, residency consistency) need access to activity internals and land with the `physics`/`automation` crates in P4; the ignored generation-timing test stays as debt. Per-step factory/expedition ticking was not changed: the headless runner steps them per tick already, and the windowed game's batched `dt` is left as is until a scenario covers factory throughput.

### P3 — Structure (moves only, no logic)  · LOC −1,000 (dropped `pub` glue, dead re-exports) · risk: high (cycles)
Order forced by cycles: (1) `core` (fast_hash, coords/`floor_div`/chunk math, time, rng, ids) — break world↔mesh/simulation by moving shared types (`ChunkData`, `BlockId`, voxel accessors) into `voxel`; (2) `content`; (3) `voxel`; (4) `physics`; (5) `worldgen`; (6) `automation`; (7) `adventure`; (8) `sim`; (9) `render` (+ `mesh`, shaders; shared WGSL extracted into `*_shared.wgsl` includes, bind-group table in `render/README.md`); (10) `ui`; (11) `app`.
Each step: `git mv`, fix paths, `just verify`, commit `refactor(<crate>): move …`. Split files > 300 lines in the same crate pass only when the split is a pure move. Per-crate `thiserror` enum; `anyhow` only in `app`.
Verify: `cargo tree -e normal` shows a DAG; `just loc` per crate; `just verify`.

### P4 — Reduction (crate by crate, smallest risk first)  · LOC −23,000 · risk: medium
| Crate / item | Target −LOC | Verification |
|---|---|---|
| Delete never-compiled files (`mesh/lod`, `mesh/sculpted`, 6 adventure encounters) | −2,700 | `just verify` (nothing references them) |
| Review harnesses → scenarios or `cfg(test)`; delete the ~70 env-var hooks + 40 CLI flags, replace with `just run <scenario>` | −4,000 | scenario parity for each `Review-*.ps1` kept |
| `src/fluid` unused stack (owner decision) | −2,200 | `just verify` |
| Content → `assets/*.ron` (materials, spells, recipes, items, machines, bestiary) | −3,500 | `content` validation test loads all assets; scenario snapshots unchanged |
| Icons: ASCII sprites → PNG atlas generated by existing `tools/gen_*_icons.py` once, checked in | −700 | UI capture hash |
| Save codec: hand-rolled `encode/decode` ×16 → serde + versioned migration (bump to v29, keep v≤28 loader) | −1,500 | round-trip test on every tracked save fixture |
| WGSL dedupe (weather sampling ×3, srgb, sky_dir, Lighting struct) | −1,000 | `build.rs` naga + capture hash |
| Hash/RNG/lerp helper dedupe | −200 | unit tests |
| God-fn splits: `step_inner`, `Adventure::update`, `main` loop, `build_ui` (pure extraction) | −1,500 (dead branches surface) | `just verify` + captures |
| Horizon: 16 `#[path]` files → `render/horizon/` module dir; flatten single-impl traits | −800 | horizon capture hash |
| `tools/`: 17 `Review-*.ps1` → scenarios; keep gen scripts; delete summarisers superseded by `just run` JSON | −2,000 (tools, not Rust) | each kept script runs |
| Dead-code warnings, unused re-exports, `allow(dead_code)` sites | −1,000 | `just clippy` |
| Data-oriented hot-path passes (SoA chunk storage, flat CA grid) — **only with `just bench` wins** | 0 ± | criterion deltas |
Commit per crate: `refactor(<crate>): -<N> LOC, <why>`.

### P5 — Docs  · LOC −11,000 docs · risk: low
`CLAUDE.md` ≤ 60 lines (replaces `AGENTS.md`; keep the flicker/pixel-grid invariants, they are real), `docs/ARCHITECTURE.md` ≤ 1 page, `docs/TESTING.md`, `.claude/skills/{render,physics,automation,adventure,voxel}/SKILL.md` ≤ 40 lines, `docs/adr/NNNN-*.md` for: workspace split, RON content, serde saves, headless scenarios, deleted review harness. Move the 110 other docs to an archive **outside the repo** (or a single `docs/history/` tarball-free dir if the user wants them kept in git — **ask**). Every path/command grepped before writing.
Verify: script that extracts `src/...`/`crates/...` paths and `just ...` commands from docs and checks each exists/runs.

### Decisions taken (2026-10-22, user-approved)
- P0 baseline: commit the whole working tree, move root junk outside the repo, green all 6 failing tests before any refactor.
- Crate deviations accepted: `voxel`, `worldgen`, `automation`, `adventure`, `sim`, `ui`, thin `app`, plus `core`, `physics`, `content`, `render`.
- Dependencies approved: `serde`, `ron`, `insta` (dev), `criterion` (dev), `thiserror`. Tools: `just`, `tokei`, `cargo-machete` (installed on the machine, not vendored).
- Review harness (~4k LOC, ~70 env hooks, ~40 flags) is replaced by `assets/scenarios/*.ron` with per-script parity before deletion. The unused `src/fluid/{catalog,containers,machine,network,processes,tank_view}` stack is deleted.

### First execution step after approval
Write `docs/AUDIT.md` (sections A–H above) and `docs/PLAN.md` (section P), commit them, then start P0.1 (confirm single-writer tree) and P0.2 (baseline commit).

### Risks and stop-points
- Concurrent sessions editing the tree (P0). Hard stop until resolved.
- Gameplay-affecting changes flagged for approval: per-step factory/expedition tick, wall-clock deadlines removal, save version bump, review-harness deletion.
- Dependencies to approve: `serde`, `ron`, `insta`, `criterion`, `thiserror`; tools `just`, `tokei`, `cargo-machete`.
- Disk: 16 GB free; workspace split multiplies nothing (same target dir) but cold rebuilds are ~14 GB. No worktrees.

### Remaining debt (post-plan, LOC-per-effort)
Filled in at the end of P4. Entries opened during execution:
- `world::dungeon::tests::terrain_generation_cost_stays_close_to_the_previous_generator` is `#[ignore]`d: it asserts a wall-clock ratio and reads 1.07x-1.98x on the same binary under parallel test load. Replace with a criterion bench in P2 (`just bench terrain_generation`), then delete the test.

### P0 log
- 2026-10-22: baseline commit `55c7f90`. Root scratch moved to `C:\dev\voxel_v2_scratch\root-junk-2026-10-22`. Pre-existing failures fixed: audio cue gain now equal-power (`1/sqrt(n)`, the test's spec); save-version test asserts `SAVE_VERSION + 1` instead of a literal; causeway test pinned to generator version 24 (Prism Sanctum replaces variant-2 caverns from 25); storage tests passed the temp *directory* as the save slot, so every test shared one `Temp\regions\test-world` lock under nextest parallelism; now they pass the save file, as production does.

