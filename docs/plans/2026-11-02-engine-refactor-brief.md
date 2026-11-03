# Engine refactor brief (2026-11-02): status

Source: the pasted "AXIOM: Engine Architecture, Performance, and Maintainability
Refactor" brief. Decisions 0036 and 0039 govern scope where it differs: M3 engine
rewrite and M4 sim rework stay out, and frame-time work uses the existing
`just frame-bench` scenes (docs/perf/audit-08e11ae3.md), not a new profiler.

## Phase 0: tree protection

- Checkpoint branch `wip/whole-tree-20261102` (29207de6): HEAD 67100136 plus every
  tracked edit and untracked source, built through a temporary index, so the shared
  index and working tree were not touched. Excludes `MC-RS/` (third-party assets and
  DLLs), `*.zip`, `*.blend*`, `*.dll`, `*.exe`.
- Restore one file: `git checkout wip/whole-tree-20261102 -- <path>`.
- Diff against HEAD: `git diff HEAD wip/whole-tree-20261102 --stat`.

## Baseline (dirty tree, 2026-11-02)

- `cargo check --workspace --all-targets`: passes, 102 warnings. 94 are dead code
  (36 functions, 13 constants, 11 methods, the rest fields, variants and types),
  3 are unused imports. 14 sit in `egui_ui/wand_xmb.rs` and `adventure/growth.rs`,
  which CLAUDE.md reserves. Most others are in files with uncommitted peer edits,
  so they are not classified further here.
- Frame-time baselines: `just frame-bench` scenes and the 2026-10-27 audits in
  memory; this machine runs about ten peer cargo jobs at once, so new timings were
  not recorded.

## Phase 3 slice: World owns mesh invalidation

Before: core writers bumped the revision, woke activity and marked the save
region, but did not mark meshes. About 45 call sites in 20 files each repeated
`blocks.insert(p); mark_chunk_and_neighbors_dirty(chunks, p)` by hand, and a new
writer that forgot to do so left stale geometry.

Now: `set_many_with`, `commit_resident_water_cell` (every water path) and
`set_replica_cell` append to `World::changed_cells`; `World::publish_changes`
moves them into the dirty sets with the halo. It runs at the end of
`fixed::finish` (window and headless, every step) and before the window's
`rebuild_dirty` (frames with no step). Tests: `world/change_tests.rs`.

Migrated callers:
- `simulation/cellular.rs` water transfers and granular swaps: `commit_transfer`
  and `transfer` lost their set parameters. `step` keeps them for thermal and
  drainage. `dam_break_throughput` now publishes per tick like `fixed::finish`.
- `simulation/rigid.rs` body moves. `tick` keeps its sets for
  `expedition::drop_log_cells`, the next caller to migrate there.
- `simulation/springs.rs` (sets parameter and mark removed). That file is
  untracked peer work, so the edit sits in the shared tree and lands with it.

Remaining callers, in order of independence:
1. `simulation::cellular::changed` users (thermal, soil, sediment, drainage) and
   `expedition::drop_log_cells`, then the `blocks, chunks` parameters of `cellular::step` and
   `sim.update`. Thermal goes together with drainage (untracked): dropping only
   thermal's sets leaves `step`'s unused in HEAD.
2. `Commit::publish(world, blocks, chunks)` in `world/effects.rs`. Not a plain
   removal: player paths (`mining::harvest`, `expedition::{harvest, spell_blast,
   blast_harvest}`, `effects::place`) pass `player_edit_chunks` as `chunks`, and
   that set carries fast-lane priority. Non-player callers (tnt, blueprint apply,
   factory extraction, smeltery, multiplayer authority) can drop the sets; player
   paths need priority to come from the journal first, for
   example a journal mark taken before the input handlers and published into
   `player_edit_chunks`.
3. Input handlers in `main.rs`. Player edits keep `player_edit_chunks` because it
   carries fast-lane priority, not correctness.
4. Multiplayer replica and replication paths (`multiplayer/*`, untracked).

Most of these files hold uncommitted peer edits; migrate each once its owner
commits, then delete the halo calls in that file.

Not changed: atomicity (writes are still per-cell under prior admission), the
global `revision`, per-chunk revisions on the render side (`player_edits.rs`),
and network `network_changes`.
