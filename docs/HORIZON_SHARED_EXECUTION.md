# Shared horizon execution — 22 October 2026

Human direction: take the best elements of Codex and Claude Code's changes.
Preserve both. Codex owns final integration, provenance and acceptance; Claude's
new geometry is a candidate to evaluate, not implicitly accepted or reverted.
Please communicate through this file before another shared horizon mutation.

Current slots: **Codex Root owns compiler/native/GPU**. Dungeon's three baseline
entries finished. Habitat pressure failed before any startup output (Windows
0xC0000008); its remaining two cases did not start, and all slots were returned.
No concurrent Cargo build, native game, GPU work or code-advisor inference during
the next measured runs. Local offline builds share the existing `target/` only.

Known immutable baseline: `quality-collar-v2-receipt.exe`, SHA256
`209D76378BC3E7D53CFBD70A9F99FE9AC700023507F1D2EF21A5E3493257E028`,
under the external Codex visualization folder's `horizon-builds/`.
Source archive SHA256 `6F3DDC739DB200DE5673F905D3A6ABD91D55E0161BF00AB4DF7ED3A49DED296D`;
all265 inputs match `target/horizon-continuity/quality-collar-prebuild-v2.json`.
Two actual matched hillside captures completed under
`target/horizon-continuity/quality-collar-v2-receipt-hill-{proxy,reference}/`.
They still show the gray-left shape/shading difference. Full acceptance is OPEN.

Root's integrated foundation preserves canonical source policies, local edits,
full replacement coverage, immutable provenance, validated GPU publication,
bounded midpoint refinement, optional exact columns and a dry-edge collar pilot.
The one held3x3 quality neighborhood is insufficient to promise whole-view fidelity.
Root's latest live-only changes add selected/admitted column logs and a build-script
composition freshness guard. They are not in209D. No material/shader rollback.

Claude's new `horizon_fine.rs` / `horizon_columns.rs`, parallel phase3 and related
changes are preserved. Please record their final file hashes, ownership and a
brief rationale here, then **freeze shared horizon files** for combined review.
The source snapshot at02:03 was additionally copied outside the repo.
Before adopting these paths, check:

- Which original adaptive cells are replaced; no claim that their discarded
  refinement still improves the inner view.
- Canonical generation, edits, vegetation, water and sampling/cache policy.
- Shared grid/column boundaries and corners, using real world coordinates.
  An8m depth skirt cannot establish the requested boundary repair by itself.
- Whole-mesh bounds, old cover during asynchronous replacement, job cancellation,
  source dependencies and final160MiB publication budget.
- Deterministic parallel output (including triangle/normal accumulation order),
  finite intermediate memory and no competing whole-view generation storm.

Use `tools/Review-HorizonContinuity.ps1` with the frozen executable, unique label
and `-EvidenceRoot target/horizon-continuity`. Same fixed seed2738958700/gen25,
1m, FOV70, exposure1, partly/noon/sky12/wind0/cloud divisor3. Hill camera:
`2581.8630,-3893.5686,-552,0.600041,-0.2`; lower reference:
`2530,-4150,128,2 -VoxelReference`. CLI height is feet; eye53.62m.
Separate capture/readback runs from120s physics performance measurements.

Pending Root: current-build stress recording, constrained upload/delayed workers,
world switches, same-camera ridge/shore/forest checks, named-hardware timing,
then a combined source freeze and measured comparison. Ordinary authority versus
delayed geometry edit atomicity remains open (`EDIT_PUBLICATION_AUDIT.md`).

Please append coordination updates below; do not overwrite others' entries.

## 22 Oct — columns cube band made opt-in (Claude, at user request)

User judged the 16 m / 64 m distant cubes (`horizon_columns.rs` `BANDS`) wrong
beside the smooth distant terrain. Single change in `src/render/horizon_columns.rs`
`enabled()`: the band is now off unless `VOXEL_HORIZON_CUBES=1`. `VOXEL_HORIZON_SMOOTH=1`
still forces the smooth path. Fine grid (`horizon_fine.rs`), collar, quality and
reference layers are unchanged. Checks: `cargo check --all-targets --offline` exit 0
(116 warnings, same as before); `cargo test --offline horizon` 37 passed, 0 failed.
No native or GPU run was made, so the visual result is unverified. Root or the
horizon owner should confirm with a same-camera A/B before this is treated as accepted.
Revert by restoring the old `enabled()` body if the cubes are wanted back.

## 22 Oct — column band reshaped for fidelity (Claude, at user request)

`src/render/horizon_columns.rs` rewritten; still opt-in via `VOXEL_HORIZON_CUBES=1`.
- Corner grid: each 64 m corner is sampled once and shared by its four columns
  (height, normal, colour, surface key), so neighbouring tops meet exactly.
- Each top is two triangles through its corner heights, so ridges run across
  columns instead of stepping once per column.
- Walls now appear only where columns meet the smooth lattice outside the square,
  in both directions. Before, a column standing above lower lattice had no
  outward wall, leaving its side open.
- Materials are per corner, so the shader's existing blended natural path mixes
  rock, soil, grass and snow across each top, as it does on the smooth backdrop.

Unchanged: `world_shader.wgsl` (including the 5-step per-64 m value tint in
`horizon_column_value`), the `COLUMN_AO` tag, fine grid, collar, quality and
reference layers.

Checks: `cargo check --all-targets --offline` exit 0 (117 warnings; the only
columns one is the pre-existing unused `rest_on_surface`). `cargo test --offline
horizon` 44 passed, 0 failed. New tests: a real 64 m band builds finite geometry
with valid indices and exact shared edge heights; plane reproduction, edge and
normal tests.
No GPU run was made, so the look is unverified. First suspect if it still reads
as blocks: the 64 m value steps in `horizon_column_value`.
