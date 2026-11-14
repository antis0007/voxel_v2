# Independent city authority verification (format 68)

## Outcome

This package repairs stale tests and establishes stronger coverage of the existing physical city authority. It does not add gameplay features or change save format, construction policy, actor movement, material ownership or checkpoint publication. It is independent of the restricted format-69 experiment. No executable is promoted.

A fresh source snapshot of C:/dev/voxel_v2_backup still declares VXWSAV68 and SAVE_VERSION=68. The before binary reproduced both quarter failures and two malformed legacy codec fixtures. The final candidate passes all 146 selected city/storage/multiplayer tests plus 24 prerequisite regressions, with zero failures/timeouts. Full workspace verification remains blocked by unrelated worldgen test compilation errors.

## Classification and changes

- Paid placement: the stale test provided carry without a worker, source or owned claim. The production commit correctly refused it. The fixture now spawns/assigns a resident, designates a real chest, proves empty fetch cannot supply materials, funds the chest with two planks and withdraws that exact stock before establishing source-tagged claims. All original Air/Planks, carry 2 to 1, queue-empty and duplicate-no-charge assertions remain. Chest + cargo + paid placement is checked against the original two planks.
- Paused/obstructed parcel: the stale test supplied only a worker ID, but the current planner requires an ID paired with saved position. It now uses the real assignment API and a surface parcel. Original pause and obstruction assertions remain, with unchanged demand, completed count, layer and queue checks. A separate missing-position control still expects No Builder.
- Two synthetic old-version save headers incorrectly included newer traversal fields. The already-reviewed test-only fixes align them with the decoder version gate. No decoder, writer, migration or namespace code changed.
- Fifteen existing compilation/codec prerequisites were carried independently from earlier reviewed test work. Fourteen are test setup/signature/assertion repairs. The production seam in streaming.rs extracts the existing pending-source-priority expression into a shared helper without changing its rules, so tests exercise the real expression. There are no format-69 boundary files in this package.

## New controls

The two city test paths add wrong claim owner, wrong cargo source, occupied target, removed support, player-edited target and cancellation rejection cases. Rejections leave cargo and target unchanged. Farming-sidecar encode/decode retains claims and carried goods, followed by repeated placement attempts that debit only once.

The outer jobs service is exercised with absent, dead and distant actors, plus a live nearby actor that has not arrived. The latter must retain its Build task/path and say Walking to build. No remote placement or material debit is accepted. This is not a claim that a direct lower-level place call validates actor distance; the production caller owns physical reach checks.

The existing crew_check is now included as a regular isolated test. It exercises real fixed gameplay, exclusive competing claims, repeated fetch/commit, player edits, cancellation and sidecar reload. Final result: 10 constructors all contributed; 80 paid blocks completed in 1,264 fixed ticks; final stock and cargo both zero. The logged fixed-gameplay loop took 41.7 ms on this machine. This tiny headless scenario is not a gameplay FPS or city-scaling benchmark.

## Exact test results

- Independent before: 141 selected tests, 137 passed and four failed, zero timeouts; 9.406 seconds runner wall time.
- First repaired candidate: 146 selected, 144 passed and the two malformed legacy-header fixtures failed; zero timeouts. This intermediate result is retained.
- Final: 86 farming-jobs, 19 storage and 41 multiplayer tests all pass (146 total), 9.063 seconds runner wall time. Five new authority tests are included.
- A further 24 tests pass for portals, typography, source priority, seed/loader helpers, material/model inputs and harvesting prerequisites, 5.266 seconds runner wall time.
- Each selected test runs in its own process with fresh save/temp/profile roots, two processes at a time, a 120-second per-test limit and disk admission guard. No production timeout was changed, and no snapshot was accepted.
- Before test build passed in 2m11s; final test build passed in 1m48s, with 377 existing warnings. Offline locked release-fast, application opt-level 1/debug 0/codegen-units 64, four jobs and incremental off were used.
- just verify was attempted offline. It stopped at check --all-targets on five unrelated worldgen source-level test errors: missing WorldSampleConfig.planet, two removed WorldSampleConfig::new calls, missing lobes helper, and relief_gradients importing a missing planetary module. Clippy, workspace tests and later aggregate stages did not run. These files were not changed.

## Safety and coverage limits

The format-69 worktree and its previous source/binaries remain separate. Original saves and playable executables are not inputs to these tests. Preservation hashes are checked against the retained audit, not a newly observed baseline. The isolated source baseline includes the existing vendored dependency only for reproducible builds; the exported integration patch excludes that baseline/vendor snapshot.

Reload tests here are farming-sidecar round trips while the world remains present, plus existing storage codec tests. They do not prove atomic whole-world interruption recovery, power-loss durability, cross-realm transaction safety or safe migration of a damaged legacy world. No such policy is changed or bypassed. The wider application suite still has known failures outside this selected scope. The original designer/posed/wildlife follow-up fixes are not included in this package.

Independent read-only review found no weakened original assertion or production authority defect. Its recommendations to assert the walking/task/path branch, avoid temporary-directory PID reuse, and label repeated attempts accurately were incorporated before the final build.

## Evidence and integration

Evidence: D:/codex-scratch/city-authority-20261107.
Source baseline: 7da6d119554e9b4b2bdaf1120c2efb89f1d5dd45, a private current-build-input snapshot.
Final test executable SHA256: ca47cbe7584e8e5a3a281a72c1a883b74be98a07eccc9ef4d42757a7714db69e.
Before test executable SHA256: cacf90e297b8dcd8c15b342708b3a19f27b00aae5aa038729f5d43000144be1e.

source-freeze.json binds the 17 code paths. The selected-before, selected-after-v1, selected-after-v2 and prerequisites-v2 folders retain exact logs/results. Build logs and verify.log retain failed and successful stages. Any guarded shared-source application is recorded separately in shared-integration.json with pre/post hashes and backups; check that record rather than assuming a prepared patch was applied. Binary promotion is not part of this task.
