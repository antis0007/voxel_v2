# 01: Land the untracked Manyfold, portal, gate and hell-gate work

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first.

## Goal

Get the maze/portal/hell-gate feature set out of the shared working tree and
into git as a small series of buildable, tested commits on a branch, without
touching other sessions' work. Nothing in this prompt adds features.

## Situation (verified 2026-10-30)

- HEAD `a678a81c` on branch `growth-xmb`. `git ls-files` returns nothing for
  any of the paths below. `git status` shows 334 modified and 331 untracked.
- Snapshot branch `wip/whole-tree-20261030` holds the whole tree (about 2,600
  files, many are review captures and scratch; it is a safeguard, not a
  source of commits). Memory `whole-tree-snapshot-and-mp-landing` describes
  an earlier snapshot, `wip/whole-tree-20261028`, and the piecewise-landing
  approach already agreed with the owner.
- Earlier sessions recorded that these changes "cannot land on HEAD alone"
  because they depend on each other and on other sessions' untracked files.
  Your job is to find the true dependency closure and land it in order.

Files in scope (all untracked unless marked M for modified):

worldgen (`crates/worldgen/src/`)
- `manyfold.rs`, `manyfold/{architecture,districts,undercroft,weave}.rs`
  (VERSIONs 84, 87, 81 in the sub-modules; `CENTER`, `RADIUS = 24`,
  `OUTER_RADIUS = 32`, `HALF_LENGTH = 24`)
- `hell_gate.rs`, `hell_gate/{approach,tests}.rs` (gate 89)
- `lib.rs` (M: module registration), `nether.rs` if untracked, check.

voxel (`crates/voxel/src/config.rs`, M): `WorldType::Manyfold`,
`WorldType::Nether`, `CURRENT_GENERATOR_VERSION` (read the real value).

app (`crates/app/src/`)
- `manyfold.rs`, `manyfold/{weave,undercroft}.rs`, `manyfold/weave/presentation.rs`
  (check which exist)
- `player/radial.rs` and `player.rs` (M: `gravity_frame`)
- `world/portals.rs`, `world/portals/{gates,gates_tests,realm,sidecar,traverse,views}.rs`
- `world/hell_gate.rs`, `world/hell_gate/appearance.rs`
- `render/portals.rs`, `render/portals/{scene,composite}.rs`
- `shaders/portal_composite.wgsl` plus M edits to `world_shader.wgsl`,
  `camera_lighting_shared.wgsl`, `shader_sources.rs`
- M: `render.rs`, `main.rs`, `world.rs`, `simulation/fixed.rs`,
  `world/storage.rs`, `world/storage/{library,paths,background}.rs`,
  `world/adventure/{chronicle,inscriptions,quest}.rs`,
  `multiplayer.rs`, `multiplayer/{authority,gameplay,open,replication}.rs`,
  `egui_ui/{front_end,world_browser,map/perception}.rs`

Docs: `docs/design/MANYFOLD_TUBE_STAGE_A.md`, `docs/worldgen/GENERATOR_{80,81,84,87,90}.md`,
`docs/decisions/0039-*.md` (edited this session), the two `docs/superpowers/specs/2026-10-29-*gap-analysis.md`.

Treat the list as a starting point. Compute the real closure with the build.

## Method

1. Create a worktree on a new branch off HEAD (not off `growth-xmb`'s dirty
   state): `git worktree add <scratchpad>/land-maze -b maze-landing HEAD`.
   Work in it. Build to a target dir with free space (D: if C: is full).
2. Copy files from the shared tree in dependency layers. After each layer run
   `just check`, `just clippy`, and the tests for that layer, and commit.
   Layers, in this order:
   a. **Leaf worldgen, dormant.** `engine_worldgen::manyfold::*` and
      `hell_gate::*` with their own tests. `WorldType` variants only if the
      worldgen needs them (then the voxel crate change and every `match`
      over `WorldType` in the workspace, found by compiling). Keep
      `CURRENT_GENERATOR_VERSION` at its HEAD value in this layer; expose gates
      as `pub const`.
   b. **Portal data and traversal, no renderer.** `world/portals*` without the
      render hooks; run `portals/gates_tests.rs`.
   c. **Portal renderer.** `render/portals*`, `portal_composite.wgsl`,
      `shader_sources.rs`, the `render.rs` hunks. This is the contract-heavy
      layer: check uniform struct layouts against their WGSL declarations,
      binding indices and formats. Run the shader test
      (`every_wgsl_file_is_included_by_shader_sources`).
   d. **Gates sidecar and gameplay.** `gates.ron`, prompts, F interaction,
      `simulation/fixed.rs` hooks.
   e. **Radial tube.** `player/radial.rs`, `gravity_frame`, `manyfold.rs`,
      weight, weave, undercroft, save tag 5 header extension, multiplayer
      pose quaternion, storage reads. This layer touches the save format: read
      `world/storage.rs` first, and run `just test save` and `just test migrate`.
   f. **Hell gate runtime.** `world/hell_gate*`, `quest.rs` compass hooks.
   g. **Generator bump.** Only if the owner-recorded plan says generator 89 is
      live. Check `GENERATOR_GATES.md` and `GENERATOR_90.md`; do not bump
      `CURRENT_GENERATOR_VERSION` unless the bump and baseline re-save
      (`just bench-save`, golden hashes) can land in the same commit.
3. For files that mix your hunks with a peer's (`render.rs`, `main.rs`,
   `world.rs`, `storage.rs` are known mixed), use `git diff -U0` against HEAD,
   keep only hunks whose symbols belong to the layer, and renumber the hunks
   (recipe in memory `commit-mine-via-clean-worktree`). Hunks you cannot
   attribute stay out; list them in the report.
4. Peer-owned failures: 4 known failing tests are not yours (chips save test,
   UI text rules, repo-root files, `docs/TRACKING.md`). Run them on a clean
   HEAD worktree to confirm they fail there too; report them as pre-existing.
5. After the last layer, run `just verify` in the worktree. Then `just bench-check`
   may be slow or need a quiet machine; if it cannot run, say so.
6. Landing: compare-and-swap. Fetch the tip of the target branch, rebase or
   merge your commits, and update the ref only if it has not moved
   (`git update-ref <ref> <new> <old>`). Do not force. Do not push.
7. Reset only paths you committed in the shared tree so the tree matches
   HEAD for them; never `git checkout .` or `git clean`.

## Acceptance

- Branch builds from a clean checkout: `just check`, `just clippy`, `just test`.
- `git ls-files` lists every file in scope; `git status` in the worktree is clean.
- Old saves load: `just test migrate`, `just test save` pass; an older save
  fixture still generates identically (generator gates untouched).
- No shader or host-struct mismatch: portal renderer starts and
  `portal-review`-style capture (see `tools/Review-VisualDirection.ps1`, flags in
  memory `crystal-gate-portals`) shows the window and veil.
- A short `docs/recovery/2026-10-30-maze-landing.md` records the commit list,
  what was left out and why, and pre-existing failures.

## Do not

- Do not delete `wip/whole-tree-20261030` or any worktree you did not create.
- Do not "fix" peers' unrelated failures or reformat files.
- Do not squash everything into one commit: one layer per commit, message says
  the layer and the dependency.
