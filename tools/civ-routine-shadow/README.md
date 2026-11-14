# Civilization routine shadow harness

Offline comparison and isolated wake candidate. The wheel set-semantics fix is now integrated into the existing dormant `engine_civ` crate. The game still uses BTreeSet; no kernel activation, gameplay wiring, save format or multiplayer change was made.

## Integrated wheel

`crates/civ/src/agenda/wheel.rs` rejects duplicate keys in every queue region. Small unsorted buckets scan at most 64 existing keys; larger buckets add a membership set that is discarded when sorted draining starts. Membership iteration never affects event order. Sorted buckets use binary position checks.

Six crate-local regressions live in `crates/civ/src/agenda/wheel_set_tests.rs`, including private membership-index invariants. Existing kernel tests were preserved. The standard harness queue tests now target the real kernel Wheel, not its retained candidate copy. The older `integrated-wheel` feature flag remains accepted for command compatibility but is no longer needed.

`wheel-set-semantics.patch` is the historical applied implementation patch, not a pending action. It does not contain the subsequently migrated crate-local tests. Do not apply it again. The original source backup and integration hashes are under `target/verification`.

## Wake and routine comparison

`src/candidate/wake.rs` selects wakes from supplied facts. `build.rs` extracts the actual current app `crew::next_wake`, the PULSE expression and the core fixed-step constant without rewriting them. Field stubs compile this reference for comparison. This proves selection logic for supplied fields, not correctness of live-world fact collection. Current activity and navigation-wait precedence are covered; no app wake replacement was made.

`Probe` clones a supplied routine before-state, predicts one operation, compares a supplied after-state and retains the first mismatch. It never applies a prediction to the executor. The harness also compiles the dormant routine/time source and the actual app routine policy. The original dormant kernel wake helper still predates current activity/navigation scheduling.

## Local checks

From this directory:

- `./Verify.ps1` runs offline, locked harness tests/Clippy and kernel unit tests, all-target Clippy and all-target checks.
- `./Verify.ps1 -Measure` additionally runs bounded release queue microbenchmarks.

Logs, command times, toolchain and source SHA-256 hashes go under `target/verification`. The script refuses unreviewed external changes against `reference-inputs.json`, rejects changes during verification and replaces stale success evidence with failed attempts. Review relevant source changes before refreshing reference hashes. The standalone workspace has its own lockfile and build output. No network or hosted CI is used.

## Coverage and limits

- 100,000 routine operations and 20,000 meeting observations against compiled app-policy methods or identified reference expressions.
- 200,000 duplicate-heavy queue operations in both crate-local tests and the harness, plus dense thresholds, cancellation, clone, clearing, late/far migration, slot reuse and tick saturation.
- 200,000 wake evaluations against the compiled current app function through lead/helper field paths, plus precedence and saturation cases.
- Synthetic eight- and 64-event-budget traces check ordering and wake-table reconstruction, not complete community histories.
- Physical targets, routes, cargo, task ownership, home occupancy, terrain, actor attachment, activity completion, save files and networking remain outside the projection.
- Exact meeting mode differs intentionally from legacy capped dwell. Only capped mode is claimed to match.

See `INTEGRATION.md` for final integration checks and `VERIFICATION.md` for earlier candidate measurements. A passing library/harness suite is not a full-game, save/load, multiplayer or release acceptance result.
