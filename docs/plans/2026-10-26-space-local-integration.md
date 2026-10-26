# Local space integration, 2026-10-26

Base: `growth-xmb` at `93cd5089`. Integration branch:
`codex/space-integration-20261026`.

The three reviewed branches were merged in order in an isolated worktree.
No phase implementation was rewritten. The final merge retains the complete
reviewed `crates/core` tree from `space-phase1`.

| Phase | Reviewed tip | Merge commit | Full test result |
|---|---|---|---|
| 0 | `bb458f4f` | `ba4779bb` | 775 passed, 2 failed, 96 skipped |
| 1 | `8a2a4c87` | `a8ad2478` | 805 passed, 2 failed, 96 skipped |
| 5a | `e16acdda` | `f1d3e4dd` | 865 passed, 2 failed, 96 skipped |

Each complete run used `just test --no-fail-fast`. After the first run,
`INSTA_UPDATE=no` prevented new pending snapshot files. Builds used offline
Cargo, four build jobs, and a separate target directory on D:.

## Reproduced base failures

Both failures were rerun individually at the untouched `93cd5089` commit:

- `headless::tests::scenarios_pass_and_match_their_snapshots`: `sand_falls`
  expects hash `2a6d6444f0d05f49`, but produces `e34f1a326789628e`.
  The scenario itself reports zero failed checks. Its stored snapshot was
  left unchanged.
- `mesh::water_depth_probe::shared_border_vertices_agree_on_packed_water_data`:
  expects many shared border vertices but finds 5, on both base and merge.

Phase 0 also passed both native GPU origin regressions via
`just test-gpu render::origin`: draw-offset/first-instance readback and
far-versus-near chunk depth agreement. Other ignored GPU tests were not run.

`just check` passed with existing warnings; `just clippy` passed.

The performance gate was not rerun. The handoff reported an already-red
benchmark gate; its baseline and threshold policy remain open. Integration
made no baseline or threshold adjustments. Full validation remains red because
of the two reproduced base failures. No new visual captures were made.

## Documentation and remaining work

The integration task owns and resolves the design-document commit hold.
`CLAUDE.md` records generator 53 from `crates/voxel/src/config.rs` and save
format 34 from `crates/app/src/world/storage.rs`. No version constants or
historical generation gates changed. Decision 0025 records the human's
2026-10-26 integration authorization. The phase-5a plan now records the
460.8 s sidereal day, 480 s solar day, accepted eccentric-anomaly endpoints,
and the reviewed validation rules.

Remaining work retains its existing design gates: phase 0b scene-stream
models, phase 5b system map, integer simulation sun/sky integration, phase 2
C02/W01 design sections, and the system-scale spike. W01 still owns the
platform-libm determinism decision for the tangent projection.

## Preservation and local evidence

The shared checkout's dirty files are preserved separately from the committed
integration. Landing uses a private Git index and a three-way merge against a
snapshot of the latest tracked and untracked work, checks concurrent edits,
and leaves those edits uncommitted on `growth-xmb`. The recovery snapshot ref is
`refs/codex-backups/space-integration-20261026-dirty`.

Evidence and original file copies are outside the temporary worktree at
`D:\codex-builds\space-integration-20261026-01a17648\evidence`:
`phase0-test-complete.log`, `phase1-test.log`, `phase5a-test.log`,
`phase0-gpu.log`, both `growth-base-*.log` files, `check.log`, `clippy.log`,
`initial-dirty/`, and `landing/`. The shared `target/` was not used or cleaned.
No remote push, CI job, or new SoA translation was part of this integration.
