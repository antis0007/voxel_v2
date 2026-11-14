# Dormant kernel integration: 2026-11-11

## Scope

Integrated the reviewed set-semantics implementation into `crates/civ/src/agenda/wheel.rs` and added `crates/civ/src/agenda/wheel_set_tests.rs`. The hash guard matched the original snapshot immediately before application. Existing inline kernel tests were preserved byte-for-byte.

- Before wheel SHA-256: `444CF9AE410A04841C4E1F831456D897A0488C880020CAB88EDFF8B22A6A8C49`
- After wheel SHA-256: `4B47B76C051A175106EDA3A7A014D9D893ECBDA2B21C706D793B867AD5A5F4F7`
- Crate regression file SHA-256: `8D191B36AA3B45F6582D239F5A244DE4B73FCFF72CA4543A6513AAE67C99F242`

Kernel exports, kernel manifest and app manifest were hash-checked unchanged through application. The app still has no `engine_civ` dependency and continues using its BTreeSet agenda. Dormant routine/time modules were not exported. No gameplay, save format, multiplayer or physical-executor code was changed by this integration.

## Review

Reviewed duplicate admission and index maintenance across insertion, removal, sorted draining, empty-slot reuse, far-window migration, cloning and clearing. The membership index contains only unsorted-bucket keys; it does not determine ordering. Counters and occupied bits change only for newly admitted keys. A new crate-local test inspects private index/slot invariants after each transition, including release of membership storage at first sorted drain.

The standalone harness now tests the real integrated Wheel by default. The retained candidate source is comparison material only. The historical implementation patch is already applied and must not be applied a second time. A reverse dry-run succeeds.

## Final checks

At 12:12:36 UTC, the recorded source hashes remained unchanged through the complete focused verification pass:

- Kernel unit tests: 20 passed, including six new set-semantics/index regressions and all existing tests.
- Harness tests: 37 passed. Default queue tests use `engine_civ::agenda::Wheel`.
- Kernel Clippy, all targets, with warnings denied: passed.
- Kernel Cargo check, all targets: passed.
- Harness Clippy, all targets, with warnings denied: passed.

All commands used local cached dependencies with `--offline --locked` and dedicated target directories. Details: `target/verification/result.json`, `integration.json`, and the named test/check logs.

Another session added a navigation-wait wake branch during this work. The isolated wake facts model and compiled-reference fixture now cover its 600-tick wake and precedence below an active commitment and above path/body work. The actual app source was preserved. Source drift remains guarded by `reference-inputs.json`.

## Remaining limits

The library fix is integrated and focused checks are green. No full-game aggregate gate, native playthrough, save/load, multiplayer or release acceptance was claimed. Prior queue timings in VERIFICATION.md are historical, contended diagnostics; no uncontended performance acceptance was obtained. The existing `crates/civ/benches/civ.rs` still has no `baselines.json`, so the repository-wide perf gate is not certified here.

The kernel crate was already untracked before this task. No shared staging, commit, push or deployment was performed; eventual source publication must include the full kernel and its dependencies, not only this patch.
