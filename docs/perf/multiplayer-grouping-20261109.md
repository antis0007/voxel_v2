# Multiplayer transaction grouping, 2026-11-09

## Result and acceptance

`Queue::stage` now buckets cells into chunk vectors instead of allocating a second tree node for every cell. A full chunk expands into a bounded 32,768-cell array, applies explicit overrides, then emits the original lexicographic order. Header validation, terrain validation, byte limits, staged draining, and atomic publication remain in place.

The frozen candidate passed 107 focused tests and the all-target check. Dense 256k-cell staging/grouping decreased by 33.8%. The first three-pair series failed the 8% per-phase gate because sparse64 decoding was 9.57% slower. A bounded, unchanged-binary repeat passed. All six recorded pairs, including the failed series, fit the same 8% allowance. This is an acceptance with measured tradeoffs, not a claim that every run passed.

## Full-frame CPU measurements

Values are milliseconds. Each series used three interleaved baseline/candidate process pairs, with five samples per case inside each process. The combined values below use the existing runner's upper-middle convention over all six run medians. No samples or failed series were dropped. The normal even-sample median is also recorded and reaches the same threshold decision.

| Case | Original group/stage | Candidate | Change |
| --- | ---: | ---: | ---: |
| dense4k | 0.665 | 0.485 | -27.1% |
| dense64k | 12.374 | 8.107 | -34.5% |
| dense256k | 51.536 | 34.123 | -33.8% |
| sparse64 | 0.022 | 0.019 | -13.6% |
| sparse512 | 0.146 | 0.122 | -16.4% |

The measured decode tradeoffs remain part of the result:

- Sparse64: 1.119 to 1.188 ms, +0.069 ms / +6.2%.
- Sparse512: 11.706 to 12.509 ms, +0.803 ms / +6.9%.
- Dense256k: 60.611 to 60.965 ms, +0.6%.

Decode source was not changed. These measured differences are not dismissed as noise; allocator and test-binary layout effects have not been isolated. The dense256k final publication is 25.657 to 25.347 ms. Sparse512 final publication remains 48.431 to 48.643 ms. Wire byte counts and measured staged peak byte counts are identical across both binaries and all recorded runs.

## Isolated grouping comparison

The candidate contains the old nested-tree implementation only as a test reference. The ignored `grouping_bucket_cost` test interleaves both algorithms in the same executable. It validates exact ordered cells and vector capacities after each pair. One initial pair is dropped from this diagnostic, then ten pairs are measured.

| Case | Tree reference, ms | Buckets, ms |
| --- | ---: | ---: |
| Dense4k | 0.452 | 0.229 |
| Dense64k | 8.691 | 3.842 |
| Dense256k | 39.866 | 16.042 |
| Sparse512 | 0.103 | 0.081 |
| Full mixed RLE chunk | 3.370 | 0.114 |
| Full alternating RLE chunk | 3.383 | 0.243 |

These diagnostic values exclude frame validation and do not replace the full-frame comparison above.

## Correctness and bounds

Six new non-ignored tests compare the old and new grouping behavior across shuffled cells, negative coordinates, empty and small allocations, maximum edit count, mixed and alternating RLE, explicit cell overrides, the 512-chunk limit, and seeded valid RLE inputs. They check exact chunk/cell order and allocation capacity. Rejection tests check duplicate cells, invalid blocks, excessive edit count, malformed RLE, excess chunks, and missing terrain. Failed staging leaves the receiving state and receipt unchanged. The stage test uses a publication callback that panics if called early.

Existing multiplayer regression tests cover coherent transactions, receipts, sequence/timeout rejection, late join, command acknowledgements, and multiplayer gameplay operations. Region, block-data, lighting, and world replica regression filters also pass. The six new tests are included in the 72-test multiplayer filter, not added a second time to the total.

- Multiplayer: 72 passed, 4 ignored.
- World replica: 15 passed.
- Regions: 10 passed, 1 ignored.
- Block data: 5 passed.
- Block light: 5 passed, 1 ignored.
- Total: 107 passed, 0 failed.

`just verify` passed its all-target check, then stopped at the pre-existing `crates/core/src/fast_hash.rs:35` Clippy `chunks_exact` lint. Later aggregate recipes did not run. The full shared tree is not declared green.

## Reproduction and evidence

Evidence is preserved at `D:\CodexArtifacts\multiplayer-grouping-20261109`:

- `source/` is the frozen source snapshot (3,499 initial files), with only the candidate grouping changes applied before the tested build.
- `snapshot-input-manifest.json` records initial source identity; `integration-manifest.json` records exact scoped integration identity.
- `baseline-tests.exe` SHA-256: `16E582BAAEB7D8A5B78565351B9E5D717406741BCCE0D97872C1EAA29D0E7591`.
- `candidate-forced-tests.exe` SHA-256: `1FC197040A243A49DA84A72A9EBB3C801E72C73A02EDF2D143AB1DE3A153C5A4`.
- `paired-01/` is the failed first series; `paired-02/` is the passing repeat. Both retain raw stdout, stderr, measurements, identity, and summary files.
- `combined-six-run-summary.json` retains all run medians, both original statuses, and both six-run aggregate conventions.
- `grouping-cost.log` contains the same-binary diagnostic.
- `test-*.log`, `grouping-tests-forced.log`, and `verify-final.log` retain correctness/check output.
- `scoped.patch` and `preimages/` preserve the source change independently of unrelated shared-tree edits.

The binaries were built offline with `multiplayer-sdr` in the same development/test profile, using the same frozen tree and cache. All tests used evidence-local `VOXEL_SAVE_DIR` directories. No user save was opened or overwritten. Build and native-app owners held their work for both timed series; the runner detected no matching contender. These are CPU test measurements, not release-frame or rendered gameplay measurements.

Re-run a new series, in a reserved quiet lane, using `tools/Measure-MultiplayerCpu.ps1` with the two exact EXEs, a new output directory, `-SourceDirectory` pointing to the frozen source, `-Runs 3`, and `-MaxRegressionPercent 8`. Run `grouping_bucket_cost --ignored --nocapture --test-threads=1` against the candidate EXE for the same-binary diagnostic. Place the matching `steam_api64.dll` next to the EXEs.

## Profiling guard improvements

An initial candidate build incorrectly reused the baseline because copied files retained timestamps older than the baseline compile. Its binary hash matched baseline and its new-test filter ran zero tests. That artifact was rejected before timing. `candidate-tests.exe` and its original build/test logs are retained only as rejected evidence; use `candidate-forced-tests.exe`. Fresh writes forced the actual compile, and the six-test count plus differing SHA-256 identified the correct result.

The shared runner now rejects identical binary hashes unless explicitly requested for an A/A control, records its own hash, detects Rust test executables as contenders, excludes only its controller and its current child, and writes explicit empty process arrays. Synthetic negative checks cover identity rejection, test-process contention, controller exclusion, and empty-array persistence without launching benchmarks.

The original empty PowerShell pipeline omitted `initial-processes.json` and `contenders.json` when there were no matches. The two measured series used that older behavior; their non-contended statuses report zero detected matches, but the omitted empty files have not been backfilled. Later guard tests prove the corrected output. The expanded process matcher is for future runs and is not retroactively claimed for these measurements.

## Remaining limits

Dense staging is still about 34 ms and decoding about 61 ms in this stress case; sparse512 publication remains about 49 ms. This change does not meet a 16.7 ms frame target or move decode/staging off the frame thread. It does not reduce the 512-machine chest payload or its serialization work.

There is no new native host/two-client, rendered gameplay, GPU, or WAN acceptance for this grouping revision. Existing multiplayer unit/regression tests and the CPU fixture are the acceptance evidence. No launcher executable or release build was replaced. The broader shared tree has later independent edits and has not been rebuilt as one final release. Only the scoped grouping, tests, profiler tool, and this report are integrated; no mixed-scope commit is created.
