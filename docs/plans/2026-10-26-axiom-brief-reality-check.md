# Axiom revised agent brief: reality check

2026-10-26. Audit of the pasted "Axiom — Revised Agent Brief" (Oct 26, 2026)
against this repository. HEAD `0852b1c3` on `growth-xmb`; working tree as of
23:30. The pasted brief was truncated at 50,000 characters inside the
"quality bar" table; anything after that point is not audited.

Method: one clean build and test run of HEAD in a detached scratch worktree
(target dir on D:), three read-only source audits, spot checks of the claims
this document relies on. Line numbers are from the working tree and move as
peers edit.

Verdict words: CONFIRMED, PARTIAL, STALE (true when written, no longer),
WRONG, EXISTS (the brief asks to build something already present),
CONFLICTS (contradicts an ADR, CLAUDE.md or UI_RULES.md).

## 1. Summary

The brief's diagnosis of the working tree is right: hundreds of uncommitted
files that tracked code depends on, no frame-time baseline, no single visual
contract. Its prescriptions are written against an imagined codebase. It names
eleven crates that do not exist (`axiom_world`, `axiom_mesh`, `axiom_render_core`,
`axiom_jobs`, `axiom_sim`, `axiom_rigid`, `axiom_light`, `axiom_ui`,
`axiom_wand`, `axiom_net*`, `axiom_profiling`), TOML content where the repo is
RON by rule, and four `just` recipes that do not exist. About a third of what
it asks agents to build already exists, and Milestones 3 and 4 contradict
accepted ADRs.

Ten corrections that change what an agent would do:

1. **HEAD builds.** A clean checkout of `0852b1c3` passes `cargo check
   --workspace --all-targets` (1 m 41 s) and runs the full test suite: 898
   passed, 3 failed, 96 ignored. The breakage is in the working tree, not HEAD
   (§2).
2. **"1,053 tests pass" was never a pass count.** It is the sum of `#[test]`
   attributes found by grep in `docs/TRACKING.md` §4, untracked files included.
3. **The untracked dependency set is about five times what TRACKING lists.**
   71 `mod` declarations in tracked files, 2 `include_str!` shaders, 30+
   `include_bytes!`/`include_str!` assets and one non-optional Cargo
   dependency (`engine_net`) point at untracked files (§3).
4. **Wand facts are stale.** A live session rewrote the wand path at
   23:14–23:20: `assets/wands.ron` (untracked) now holds 12 families
   (6 Modeled, 6 Planned) with lineage and a 5.04 mm texel. A new
   `axiom_wand` crate would duplicate it and collide with that session (§5).
5. **Scale is one preset per world** (ADR 0007, `crates/core/src/scale.rs:1`).
   The brief's octree of mixed chunk scales in one view does not exist and
   contradicts the accepted design (§6).
6. **Profiling exists**: GPU timestamp queries per pass, per-system CPU timing,
   an F3 HUD with p95/p99, `--benchmark*` fixed-camera runs, and a criterion
   gate at +8 %. Missing: a committed frame-time baseline and a frame-time gate
   (§4).
7. **The sim milestone duplicates `engine_sim`** (Margolus phases, dirty
   bricks, pinned determinism hash) and breaks four ADRs with its 30 tps,
   u8 temperature, TOML materials and rapier3d (§7).
8. **The brief's text style contradicts UI_RULES.md**: em-dash in its model
   string, tier adjectives, flavour on machines, a second-person
   "Zork-flavored" lore layer (§9).
9. **"Axiom" already names a different milestone track.**
   `docs/plans/2026-10-26-axiom-execution.md` (untracked) is the live ledger for
   Axiom A–F (CA authority, observatory, checkpoints) and carries the standing
   constraint "do not add or run tests or hosted CI". The brief requires
   `just test-fast` and CI. The owner has to pick one (§11).
10. **The "9 km hairline" is not a threshold.** Orbital fades are at 5–6 km
    (near corridors) and 16–20 km (far braid); corridors 20–24 km away are
    built on the CPU and fully veiled (§8).

## 2. Measured: clean HEAD

Detached worktree of `0852b1c3`, `CARGO_TARGET_DIR` on D:, `cargo test
--workspace --no-fail-fast`, 2026-10-26 23:17–23:27.

| Step | Result |
|---|---|
| `cargo check --workspace --all-targets` | pass, 1 m 41 s |
| Tests | 898 passed, 3 failed, 96 ignored |

| Failing test | Cause |
|---|---|
| `headless::tests::scenarios_pass_and_match_their_snapshots` | `sand_falls` snapshot drift; already listed in TRACKING §4, never accepted |
| `repo_layout::tests::agent_docs_name_only_existing_paths_and_commands` | committed `docs/TRACKING.md` (862eb41c) names 11+ untracked paths (`crates/net/`, `assets/runtime/viewmodels/...`, `crates/app/src/multiplayer.rs`) |
| `mesh::water_depth_probe::shared_border_vertices_agree_on_packed_water_data` | "expected many shared border vertices, found 5" (`mesh/water_depth_probe.rs:122`); not documented anywhere |

Consequence for Milestone 0: "unblock the test harness" is not the work.
The work is (a) committing working-tree changes in buildable sets and (b) three
real test failures on HEAD, one of them caused by the tracking document itself.

Disk: C: had 2.5 GB free during this audit; a clean build needs its target on
another drive.

## 3. Milestone 0 (integration)

| Brief claim | Verdict | Evidence | Correction |
|---|---|---|---|
| HEAD does not build | WRONG for HEAD, CONFIRMED for working tree | §2 | Scope M0 to the working tree |
| `just test` blocked | PARTIAL | TRACKING header: peers' in-flight files (`crates/sim/src/scenario_tests.rs`, `crates/content/src/fittings.rs:95`) | Verify per crate until peers land |
| 153 untracked paths | STALE | 156 collapsed entries, 492 files with `-uall`; plus 167 modified | Inventory by file, not entry |
| Hard build dependencies are orbital and multiplayer | WRONG (too narrow) | 71 tracked `mod` lines into untracked files across app, worldgen (15 modules incl. `meridian_terminal`), content (`fittings`), sim_gpu; `shader_sources.rs:64` includes 2 untracked WGSL; `crates/app/Cargo.toml:16` `engine_net` non-optional | Generate the manifest mechanically: grep `mod`, `include_*!`, Cargo deps, check `git ls-files` |
| Multiplayer crates need a decision | STALE | `crates/app/Cargo.toml:10-13` `default = ["multiplayer"]`; `mod network_player` (`mesh/models.rs:20`) and `online_browser` (`egui_ui.rs:24`) unconditional | Disabling the feature does not remove `engine_net` today |
| `just build`, `test-fast`, `capture`, `profile` | WRONG | `justfile` has check, clippy, test, test-gpu, bench, bench-check, run, verify, gate and others | Name real recipes or add them under M0 |
| Ownership map needed | EXISTS (partial) | `scripts/workflow.py` claim/adopt-ownership/owners, `.claude/verify/policy.json`, `just worktrees`, TRACKING §6 | Extend TRACKING §6; do not add a second registry |
| Worktrees classified | PARTIAL | 26 linked; `docs/recovery/2026-10-24-inventory.md` covers 13 | Classify the 13 newer ones |
| CI exists | WRONG | no `.github/` or other CI config | CI conflicts with the Axiom ledger constraint (§11) |
| Review script pins generator 25 | CONFIRMED | `tools/Review-OrbitalStructure.ps1:26` | Script itself is untracked |

## 4. Milestone 1 (performance)

| Brief claim | Verdict | Evidence | Correction |
|---|---|---|---|
| No baseline or gate | PARTIAL | `just bench-check` (+8 %, in `just verify`), `scripts/bench.py`, baselines for core/sim/space/worldgen, ADR 0006 | Frame-time baseline is "not started": `docs/plans/2026-10-22-render-arena-and-horizon-evidence.md` Task 2; ADR 0016 sets p95 60 fps but its budgets file does not exist |
| Add tracy/puffin | CONFLICTS (new dependency) | CLAUDE.md: no new dependency without an ADR line | Existing: GPU QuerySet per pass (`render.rs:241-256`), `perf.rs:10-25` per-system CPU, F3 HUD (`egui_ui.rs:1871-1972`) |
| Fixed-camera benchmark scenes | PARTIAL | `--benchmark*` flags `main.rs:265-569`, p50/p95/p99 report line `main.rs:1355` | Needs window and GPU; `--headless` has no frame timing. Scenes become RON + flags, not a new format |
| Is meshing on the main thread | PARTIAL | terrain on `chunk-mesh-N` workers (`render/streaming.rs:987-1026`) | Entity, wand and orbital models rebuilt every frame on the main thread: `mesh::models::build` + `upload_scene` (`main.rs:4623-4626`). First audit target |
| Culling | PARTIAL | CPU sphere-frustum, far plane ignored (`streaming/visibility.rs:375-431`); `multi_draw_indexed_indirect` with CPU args | No occlusion; shadows and horizon cull separately |
| Texture binding strategy | WRONG premise | terrain is vertex colour plus procedural surface (`world_shader.wgsl:21-29`); no block atlas | Bind-group counting matters less than the brief assumes |
| Reference hardware "mid-range 2023 GPU" | not measurable here | owner's machine is RTX 4090 plus UHD 770 iGPU (`VOXEL_GPU_POWER=low`) | Use the iGPU as the low tier stand-in |

## 5. Wand foundation

| Brief "verify" item | Verdict | Evidence |
|---|---|---|
| Six seeded styles map to three appearances | STALE | `% 3` aliasing removed; `wand_appearance.rs:99-101` maps through `family_index`; test `every_head_style_has_its_own_family` (`:227`) |
| Three of twelve families exist | STALE | `assets/wands.ron`: 6 Modeled, 6 Planned, with lineage |
| PNG extrusion plus glTF hand | PARTIAL | `wand_kit.rs:42-95` extrudes ASCII sprites from `wands.ron`; PNG no longer loaded; glTF hand confirmed (`wayfarer.rs:208`) |
| Texel about 3.4 mm | STALE | `wands.ron:16` `pixel_m: 0.006` × 0.84 = 5.04 mm, inside the 4.5–5.5 mm target |
| Assets untracked but `include_bytes!`d | CONFIRMED, wider | `viewmodel_review.rs`, `wand_appearance.rs:87`, `wand_pose.rs:42`, `wand_checks.rs:102`, `machine_casing.rs`, `conduit.rs`, `voxel_asset.rs`, `ground_detail/library.rs:35`, `cpu_region/checkpoint.rs`, `storage/session/actors.rs` |
| One shared glow cube | STALE | `glow_voxel` now only on the review-GLB path (`wand.rs:61-66`); families light their own emitter texels (`wand_kit.rs:137-139`) |
| Where identity lives in saves | PARTIAL | `WandRecord` (`world/adventure/wand_items.rs:15`) stores tier, mode, seed, slots; appearance derived from seed (`:71-83`); crafted wands (seed 0) follow the global atelier style (`:87`), so they all change look together |
| Tinkers-style parts schema | EXISTS (untracked, unused) | `crates/tools` + `assets/tools/{parts,materials,traits}.ron`, ADR 0022 "library only"; no wand schema |

Corrections: no `axiom_wand` crate and no TOML. Phase A becomes "commit the
live wand session's `wands.ron` path with its dependents", owned by that
session. A per-wand family field needs a save bump with a migration test
(working tree is at save 40). The `wand_appearance.rs:2` comment ("save keeps
`WandStyle` indices") is wrong.

## 6. Milestone 3 (engine)

| Brief claim | Verdict | Evidence |
|---|---|---|
| 32³ chunks at every scale | CONFIRMED (32³), CONFLICTS (every scale) | `core/src/coords.rs:4`; padded 34³ |
| Variable scale as an octree of `2^k` chunks | CONFLICTS | ADR 0007 "variable scale is LOD first"; `core/src/scale.rs:1` "Every world is built at one preset"; fine sub-bricks (ADR 0023) are storage only, unreferenced from `crates/app` |
| Palette compression per chunk | WRONG (absent) | `voxel/src/chunk.rs:7-15` flat `[BlockId; 32768]` + light; regions use RLE varints (`world/regions.rs:246-266`) |
| Floating origin | EXISTS | `render/origin.rs` `RenderOrigin`, f64 eye, GPU tests |
| Job system | PARTIAL | named std-thread pools with mpsc; no rayon |
| Save migration under sign-off | CONFIRMED rule | working tree save 40 / generator 65 uncommitted; HEAD and CLAUDE.md say 34 / 53 |

The 5 mm base and `2^k` ladder in the brief do not match `VoxelScalePreset`.
Seams between scales exist only as render LOD 0–3 and the horizon heightfield
(ADR 0012). An engine rewrite is an owner decision against ADR 0007, not a
milestone an agent starts.

## 7. Milestone 4 (simulation)

| Brief item | Verdict | Evidence |
|---|---|---|
| 30 ticks/s | CONFLICTS | fixed 1/120 s (`core/src/time.rs:5`); 60 Hz CA in the opt-in region (ADR 0018) |
| 2×2×2 checkerboard | EXISTS | Margolus two-phase (ADR 0008, `sim/src/kernel.rs:1-7`); eight phases only if spike S1 shows ≥25 % (MASTER_PLAN) |
| `assets/sim/materials.toml` | CONFLICTS | `assets/materials.ron`, `reactions.ron` (ADR 0002) |
| rapier3d rigid bodies | CONFLICTS | ADR 0011 CPU voxel assemblies; new dependency |
| u8 temperature | CONFLICTS | `sim/src/cell.rs:29` u16 in 0.1 K (ADR 0007 rule 6, energy conservation) |
| Dirty-region sleeping | EXISTS | brick `u64` dirty mask, `world/activity.rs:166` wake |
| Cross-machine determinism | EXISTS (single machine) | `the_canonical_scenario_hash_is_pinned`; sim_gpu byte-identical |

This milestone is already PLAN2 and MASTER_PLAN M1 (`dam_break_200k`, spike
S1). Route it there instead of opening a parallel track.

## 8. Meridian distance

| Brief claim | Verdict | Evidence |
|---|---|---|
| 6 km pop | CONFIRMED | `world/orbital.rs:15` `SIGHT_RANGE_M = 6000`; `world_shader.wgsl:3658-3660` smoothstep 5000–6000; `mesh/models/orbital.rs:86-100` far branch |
| 9 km hairline | WRONG | no 9 km constant; far braid fades 16–20 km (`world_shader.wgsl:3663-3664`); 9 km is the TRACKING capture pose |
| Megastructures have no fade | STALE | 16–20 km coverage fade exists, not aerial perspective |
| One trajectory per tether | PARTIAL | near and far share `suspension()`; anchorage offset hand-copied in 4 places (`mesh/models/orbital.rs:63`, `dish.rs:36`, `receiver_review.rs:390,457`); no asset |
| Add camera-relative rendering | EXISTS (terrain), PARTIAL (orbital) | orbital vertices built in absolute f32 metres; `orbital/skyhook.rs:62` subtracts planet radius in f32 (~0.125 m steps) | 
| One fog implementation | WRONG | 9 fog/AP paths (atmosphere_shared, LUT AP, height fog, cloud fog, local mist, underwater haze, cosmos fog, cloud AP, streaming edge fade) |
| G65 handoff exists | WRONG | `meridian_terminal*` never committed on any branch |
| Depth convention | reverse-Z main, forward-Z shadows | projection built twice (`main.rs:4160-4170`, `:4374-4384`) |

Corrections: data file is `assets/*.ron`, not TOML; extend `RenderOrigin` to
model meshes instead of adding a scheme; the fade design uses the real
thresholds 5–6, 16–20 and 24 km; drop "sub-pixel" claims about 9 km.

## 9. Milestone 2 (art, UI, text)

| Brief item | Verdict | Evidence |
|---|---|---|
| No art contract | PARTIAL | `docs/design/PIXEL_ART_DIRECTION.md`, `VISUAL_DIRECTION_EXECUTION.md` (tracked); `ART_DIRECTION.md`, `PIXEL_WORKSHOP_RESET_20261026.md`, Prompt Pack (untracked) |
| Decide texel density | CONFLICTS between docs | PIXEL_WORKSHOP_RESET: 4.5–5.5 mm held assets; Prompt Pack `:21,:54`: 48–64 px/m (15.6–20.8 mm). Code follows 4.5–5.5 mm |
| Master palette | EXISTS | `assets/pixel_palette.txt` (tracked, `egui_ui/pixel_art.rs:72`) |
| UI component crate | PARTIAL | helpers scattered and duplicated: two `slot`s (`icons.rs:915`, `grimoire.rs:269`), `gauge`, `panel`, `hud_frame`; no shared tooltip widget |
| Tooltip contract | EXISTS | `docs/systems/tooltips.md` |
| Side configuration | PARTIAL | `world/factory/sides.rs` (untracked), conduits only |
| `assets/lang/en_us.toml` | CONFLICTS | RON-only content, no `toml` dependency; existing `assets/descriptions.ron` + `just lint-descriptions` |
| "Basic Energy Cube — 4,000 FE/t" | CONFLICTS | UI_RULES.md:30 machines named for what they do, never tier adjectives |
| Italic flavour, `lore.*`, Zork layer | CONFLICTS | UI_RULES.md:9 (never narrates), :20 (no second person), :32 (lore one italic line on items/blocks/guidebook, never machines), :107; CLAUDE.md: no new proper nouns or lore without owner approval |
| Mekanism-style GUI | owner call | machine body look "Mekanism" was rejected 2026-10-25 for casings; GUI layout conventions are a separate question |

Corrections: write the contracts as amendments to UI_RULES.md and
PIXEL_ART_DIRECTION.md, resolve the 4.5–5.5 mm vs 48–64 px/m conflict first,
grow the `text_rules.rs` CHECKED list instead of a new string table.

## 10. Milestone 5 (lighting)

| Brief item | Verdict | Evidence |
|---|---|---|
| Coloured flood-fill light | PARTIAL | `world.rs:2111` `propagate_light` (4-bit, torches), never read by the mesher; colour from 64 dynamic point lights (`render.rs:128`) |
| Shadow cascades | EXISTS | `render/shadow.rs:21-29`: 4 near + 1 far, 2048², far 8192 |
| Indirect | EXISTS | `render/indirect.rs` half-res screen-space gather |
| Sky model | EXISTS | Hillaire-style LUTs incl. 3D aerial perspective |
| One shared atmosphere include | WRONG | 9 paths (§8) |

First lighting task is consolidation of the 9 fog paths and the two projection
setups, not new tiers.

## 11. Conflicts the owner must resolve

1. **Tests and CI.** The Axiom execution ledger says "do not add or run tests
   or hosted CI"; the brief makes `test-fast`, `test-full` and CI a gate for
   every task. (This audit ran HEAD's existing suite once, in an isolated
   worktree, to measure §2.)
2. **Which Axiom.** Ledger milestones A–F (CA authority, observatory,
   checkpoints) or the brief's M0–M5. They share a name and no tasks.
3. **Texel density.** 4.5–5.5 mm (workshop reset, code) or 48–64 px/m
   (Prompt Pack).
4. **Engine direction.** Keep ADR 0007 (one scale per world, LOD first) or
   supersede it with mixed-scale chunks.
5. **Text register.** UI_RULES.md as written, or the brief's pack register
   with flavour lines and a lore namespace.

## 12. Brief reordered against the repo

1. **M0, narrowed.** Generated dependency manifest (tracked → untracked), then
   buildable commit sets per owner: wand path (live wand session), orbital and
   G64/G65 (orbital session), `crates/net` + multiplayer (gated, with
   `engine_net` optional), worldgen modules with their generator gates, CA
   session files. Fix the 3 HEAD failures: accept or fix `sand_falls`,
   remove untracked paths from committed TRACKING.md or commit them,
   investigate `water_depth_probe`.
2. **M1, narrowed.** Commit a frame-time baseline using the existing
   `--benchmark*` path and F3 timers (render-arena plan Task 2); add a
   frame-time gate next to `bench-check`. First audit target: per-frame
   `mesh::models::build` on the main thread.
3. **M2.** Resolve §11.3 and §11.5, then amend existing docs; consolidate the
   duplicated egui helpers into one module before any new crate.
4. **Meridian distance.** Starts after G65 is committed; uses the real
   thresholds, `RenderOrigin`, one RON trajectory replacing the 4 copies.
5. **M3, M4, M5.** Not startable as written. M4 is MASTER_PLAN M1; M3 needs
   an ADR 0007 decision; M5 starts as fog consolidation.

## 13. Progress (2026-10-27)

Owner decisions in `docs/decisions/0036-axiom-brief-scope.md`: tests and local
gates, this reordered plan, 4.5–5.5 mm texels, the pack text register.

| Item | State | Commit |
|---|---|---|
| M0 clone completeness: `just deps [rev]` in `just verify`, `just deps-inventory` | done; HEAD 0 edges, working tree 75 edges in 64 commit sets | ae618c79 |
| M0 untracked inventory | `docs/integration/untracked-inventory.md` (generated snapshot) | this commit |
| M0 HEAD failure `water_depth_probe` | fixed: fixture pinned to generator 52 (gen 53 moved the coast) | efaa19ee |
| M0 HEAD failure `sand_falls` | fixed: bisect names c0e4c288; all three scenarios pinned to 52 and reproduce committed hashes; `smoke` and `water_falls` had drifted too | 08e11ae3 |
| M0 HEAD failure `repo_layout` (TRACKING.md names untracked paths) | open: TRACKING.md has uncommitted peer edits; fixed when its owner commits the named files or rewords | — |
| M0 buildable commit sets per owner | open: owners' files; the inventory lists each set and its referrers | — |
| M0 `engine_net` optional under `multiplayer` | open: multiplayer files are a live peer's | — |
| M1 frame-time scenes, baseline, gate | done: `just frame-bench[-check\|-save]`, 5 scenes, contention-aware | 3cbfca6f |
| M1 audit | `docs/perf/audit-08e11ae3.md`: shadow cascades, render submission, idle streaming | 3cbfca6f |
| M2 text register | `UI_RULES.md` rules 12, 14, 15 and tooltip contract; CLAUDE.md lore line | 00245107, ad3b6d5b |
| M2 texel density | recorded in decision 0036 | be6db90f |
| M2 egui helper consolidation | open: helpers sit in panels peers are editing; needs before/after UI captures | — |
| Meridian distance | blocked: generator 65 uncommitted | — |
| M5 fog consolidation | open: touches `world_shader.wgsl`, which the orbital session is editing; needs A/B captures | — |
