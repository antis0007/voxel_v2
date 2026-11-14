# Historical candidate verification

The following records the pre-integration candidate at 12:00 UTC on 2026-11-11. The wheel has since been integrated into the dormant kernel; see INTEGRATION.md for current status. The original-wheel measurements below refer to its pre-fix version.

# Verification: 2026-11-11

## Final isolated candidate

At 12:00:07 UTC, `Verify.ps1 -Measure` completed with all recorded inputs unchanged:

- 36 default-mode tests passed: 8 imported routine tests, 6 baseline agenda/wake diagnostics, 5 fixed-wheel regressions, 2 current-wake differential tests, and 15 app-policy/parity tests.
- Fixed-wheel coverage includes 200,000 duplicate-heavy operations and dense membership thresholds, removal, cloning, far migration, slot reuse, clearing and tick saturation.
- Wake candidate matched 200,000 evaluations of the actual compiled app function through lead/helper field paths, plus precedence and boundary cases. The actual PULSE expression and core fixed-step constant are also compiled, rather than copied as literals.
- `cargo clippy --offline --locked --all-targets -- -D warnings` passed. Imported app-policy tests retain the repository's existing `field_reassign_with_default` allowance. The field-stub module allows only its intentionally private reference interface.
- `git apply --check wheel-set-semantics.patch` passed. No patch application occurred.
- The explicit `integrated-wheel` duplicate regression failed against the unchanged original, as expected: four stored keys versus the reference's three. This verifies that the integration gate tests the real kernel, not the candidate copy.

Source and candidate SHA-256 values are in `target/verification/result.json`. Test/lint/measurement logs are in the same directory. The original wheel remains `444CF9AE410A04841C4E1F831456D897A0488C880020CAB88EDFF8B22A6A8C49`.

## Queue measurements

Release build; nine samples per workload; implementation order rotated. Each sample includes queue construction, insertion and complete draining. Times below are medians in milliseconds.

| Workload | BTreeSet reference | Fixed candidate | Original Wheel |
|---|---:|---:|---:|
| 8,192 keys, one due tick | 0.8185 | 0.4535 | 0.2044 |
| 8,192 spread keys | 0.6295 | 0.3146 | 0.2570 |
| 65,536 spread keys | 5.1918 | 1.3302 | 1.2656 |
| 8,192 keys inserted three times | 1.5626 | 0.6071 | Incompatible |

Other cargo/rustc processes were active when the final run started. These are contended diagnostic microbenchmarks, not an accepted performance gate or game-frame measurement. The fixed candidate is slower than the incorrect original on unique-key workloads, particularly the dense cohort, while remaining below BTreeSet in these samples. An earlier linear-only duplicate fix took 3.97 ms for the dense cohort and was replaced by the membership index.

## Readiness and boundaries

The isolated implementation and patch are ready for source-owner review. Production integration is not complete. Apply the reviewed wheel patch, run the integrated-wheel feature gate against the real kernel, preserve its regression tests, and then collect an uncontended representative agenda measurement. The pure wake candidate matches supplied current fields; a live facts adapter and its game-level validation remain separate work.

Concurrent app edits were detected and preserved, including activity scheduling and later navigation work. Reviewed hashes were refreshed only after checking the affected projection. The existing app still uses BTreeSet and now has a 64-event budget. Neither the original kernel nor the app was modified by this task.

No full-game aggregate gate, native playthrough, application save/load, multiplayer, live performance acceptance, commit, push or deployment was performed. The harness depends on pre-existing untracked kernel work, which must accompany any eventual source release.
