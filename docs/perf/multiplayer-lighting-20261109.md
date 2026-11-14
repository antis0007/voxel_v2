# Multiplayer block-light publication cost

The chunk-local flood candidate preserves the synchronous terrain/gameplay publication boundary. It reduces repeated chunk accounting and region dirty-map updates; it does not defer lighting, drop edits, weaken admission checks, or change the wire/save format.

## Reproduction and scope

`D:/CodexArtifacts/multiplayer-lighting-20261109` contains the frozen source, source/input hashes, original lighting file, separate baseline and candidate test executables, raw logs, and the interleaved benchmark results. The source freeze recorded 3,344 files with no drift during copying. Six exact `include_bytes!` assets omitted by the snapshot exclusion were copied and separately hashed. Tests use the optimized development profile, not a shipping release.

The existing historical executable reproduced the earlier sparse512 stall at 178.136 ms median / 180.955 ms max. The final comparison uses two newly compiled executables from the same frozen source, with only the candidate lighting implementation and its tests changed. Three interleaved baseline/candidate runs each collect five samples per case. The regression runner checks for competing compiler/game processes throughout and retains binary SHA-256 identities. This run had no detected contention.

## Measurements

Final poll includes its last staging remainder, admission, atomic commit, relighting, and gameplay publication. These are CPU latency measurements, not rendered FPS or network/WAN acceptance.

| Case | Baseline median | Candidate median | Change |
| --- | ---: | ---: | ---: |
| dense4k | 0.658 ms | 0.648 ms | -1.5% |
| dense64k | 6.041 ms | 6.315 ms | 4.5% |
| dense256k | 25.977 ms | 26.556 ms | 2.2% |
| sparse64 | 23.737 ms | 6.850 ms | -71.1% |
| sparse512 | 179.031 ms | 47.726 ms | -73.3% |

Encode, decode, grouping and final publication all remained within the runner's 8% regression allowance. Raw measurements, not just the passing summary, are retained. The separate same-process oracle benchmark measured center/face/corner floods at 58.0/64.2/80.1 us versus the original 321.3/324.5/305.3 us, with equal light arrays after every sample. A competing-entry boundary maze measured 46.3 us versus 222.8 us, with identical light arrays.

## Correctness and ownership

The previous cell-wise flood is retained only in tests as a differential oracle. Coverage includes negative chunk coordinates, faces, corners, missing chunks, repeated/overlapping sources, wall and lamp edits, snapshot COW, resident allocation accounting, skylight preservation and untouched-neighbor ownership. Final coverage also includes levels 0 and 15, an opaque source, already-dirty version freshness, and the competing-entry boundary maze.

The final candidate passed 65 multiplayer tests, 15 atomic-replica tests, 10 region/cache tests, 5 block-data tests and 5 lighting tests: 100 focused tests total. The ignored lighting comparison and chest-cache diagnostics also passed. This includes chest/machine transfer conservation and cross-chunk relighting after all staged replacements are installed. A separate read-only review found no correctness blocker for the production level-14 call and checked region-version/writeback freshness.

Repeated dirty writes now produce fewer numeric region versions. The invariant is freshness, not a version increment per lit cell: each genuinely modified chunk receives a new version before control returns, and prior writeback snapshots retain their old Arc/version. Lighting still completes before gameplay publication and receipt acknowledgement.

## Repeatable checks

Run `tools/Measure-MultiplayerCpu.ps1` with `-Baseline <old-tests.exe> -Candidate <new-tests.exe> -OutputDirectory <new-directory> -SourceDirectory <frozen-source>`. It refuses to overwrite evidence, requires the complete 30-row benchmark output, records executable hashes, isolates disposable storage through `VOXEL_SAVE_DIR`, interleaves three runs, checks competing processes, and exits 1 for >8% regression or 3 for detected contention. Supply the existing Steam runtime beside test executables built with `multiplayer-sdr`.

Run `just test block_light` for differential correctness. Run the ignored `block_light_chunk_cost` test only in a quiet CPU lane to compare the chunk flood to the prior algorithm. The coherent-frame benchmark is `coherent_frame_dense_sparse_cost`.

## Explicit remaining limits

- The extreme sparse512 commit still takes 47.726 ms, above a normal frame budget. This is an improvement, not a complete solution to large burst stalls.
- Dense256k decode remains 61.285 ms and grouping 50.959 ms; dense final publication is 26.556 ms. These phases remain synchronous.
- Forced factory serialization at 512 machines remains 2.694 ms median / 2.806 ms p95 per chest-triggered refresh. Changing its cadence without preserving the matching chest/machine image would reintroduce the prior stock-coherence bug, so that path was left unchanged.
- No rendered gameplay, GPU performance, WAN test, long soak, launcher update or user-save modification was performed for this patch.
- The shared checkout has extensive unrelated work. Only guarded scoped files should be integrated; no mixed commit or broad formatting is appropriate.

## Final gates and integration

`just verify` passed all-target compilation, then stopped at the existing `crates/core/src/fast_hash.rs:35` Clippy lint for `chunks_exact` with a constant chunk size. That unchanged file is outside this patch. Full tests, aggregate benches and later verification stages did not run through that command. The final focused tests above ran separately against the preserved final test executable. This is not a full-tree green gate.

The baseline test executable SHA-256 is `BEAE0E14DCC62A374E3F962DD681F4DAB0C2AA2E6FED1729C0AF4911DA4E21FE`. The final candidate is `33F6B0026EA779458A1888F8DBBF12DFF36A3B1DE080F7FEB14643A295F77916`. The final paired evidence is `paired-final/summary.json` with raw samples and process checks alongside it.

Initial snapshot compilation failed only because six included art inputs were excluded by the snapshot copier; exact source assets fixed that. The first copied-executable launch failed before tests started because its existing Steam DLL was absent; it was copied beside both executables. Failed attempts are retained and are not counted as benchmark samples. A synthetic contention negative control proved the runner refuses to start a benchmark when a compiler is reported active.

Guarded integration checks the live lighting file against its original input hash, refuses existing destination files, and saves the exact live preimage. Scope is `world/block_light.rs`, its new `block_light/chunk_tests.rs`, the regression runner and this report. No unrelated source is copied back, no mixed commit is made, and the launcher is unchanged.
