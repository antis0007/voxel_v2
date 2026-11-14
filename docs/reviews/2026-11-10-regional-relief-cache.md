# V13: persistent completed regional relief

Implemented in shared source; separate playable executable built and headless-smoked. Existing launchers and user saves were not changed. V12 is preserved.

## Measured repeat-load benefit and first-population cost

Three fresh-process samples per condition, same production probe executable, warm geology, seed74/generator287/Planet/planet=None. Six sequential padded chunks,235,824 blocks. These are cache-policy workloads, not an algorithm/FPS comparison.

| Scale | Cache disabled median | Empty cache populated median | Existing cache median |
|---|---:|---:|---:|
|1m|24.921s|26.741s|1.234s|
|0.25m|8.394s|10.072s|0.509s|

Warm reductions: 95.05% and 93.93%. First population costs: +7.30% and +19.99%. Do not hide this tradeoff. First-ever geology, GPU rendering, multiplayer throughput and FPS were not improved or measured by this experiment. Setup is excluded from these chunk timings. A new executable conservatively misses both executable-bound caches.

All output blocks matched:1m `686900d727e4a31d`, quarter `17e3caeee32aecaf`. The1m fixture includes river-surface chunks Y31..64; quarter surface coverage was not independently established. Actual concurrent game workers may skip contested writes, so their initial cache coverage and time-to-play require separate measurement.

The1m cold run stores439 unique Stages (~34.1MB payload) and reuses50 same-run intermediates; a warm run hits all489 requests. Quarter stores/hits220 Stages (~11.6MB). Bounded buffering reduces seven writes per nonempty frame to one, but its interleaved cold timing advantage was only1.30%, within the observed variation. No separate meaningful speedup is claimed for buffering.

## What is cached

Only completed procedural `Stage` Grid + exact rock_delta/soil/optional base arrays. `Stage::solve` remains unchanged and callable without persistence. Canonical Levels closures retain their existing OnceLocks and fine-parent publication. No voxel edits, NPC state, save files or mutable app World state enter this cache.

Authority attaches to the immutable planetary World only after full build completion and before publication. Direct builds and direct Stage solves remain uncached. Keys bind executable/schema, all raw construction and query configuration fields, shelf/radius, completed World checksum, face/tile/spacing/pass/distant identity, and SHA256 of the complete parent Stage. Unsupported versions use the original path. Query voxel scale is separate from construction scale.

## Storage and policy

Separate `relief-v1` namespace,512MiB logical file-byte cap and8192 entry cap including temporary/lock peak. Complete file <=512KiB, exact Stage payload <=400KiB. These are logical sizes, not a physical NTFS allocation guarantee. A single session-held writer lease, one initial inventory, nonblocking writer contention, bounded metadata, SHA256 integrity, same-directory atomic replacement and strict shape/finite/length/trailing checks. No background thread, unbounded queue, per-entry fsync or repeated full inventory. Buffered writes use at most512KiB transient buffering.

Only recognized owned entries are evicted; unknown files and reparse points are protected. I/O failure/corruption/missing entries recompute normally. Valid-envelope semantic corruption can be repaired. Unexpected external changes disable writes for that process; restart restores normal inventory/lease acquisition. This assumes a private cooperative cache directory; SHA256 provides integrity, not authentication against a malicious local writer. Power-loss durability is deliberately unnecessary because data is disposable.

Interactive startup enables eligible287 relief caching by default. `VOXEL_NO_RELIEF_CACHE` presence disables it. Nonempty absolute `VOXEL_RELIEF_CACHE_DIR` selects an explicit root. Otherwise use `VOXEL_CACHE_DIR/relief-v1`, or `%LOCALAPPDATA%/VoxelWorkshop/cache/relief-v1` on Windows. Benchmarks remain disabled unless the dedicated relief override is supplied; a generic cache root alone does not enable benchmark reuse. No save-directory fallback; geology and relief opt-outs are independent.

## Verification

- 21 active Rust storage/codec/key tests, including real process writer exclusion, concurrent atomic reads, quotas, corruption/repair, foreign-file protection, exact bits, buffered-write count and injected failures.
- 2 actual-world authority tests: every32/16/8/4m Stage array equals direct solves,1m and quarter queries on construction1m World, face seams/mixed footprints, fine-first parent publication, and mutable/direct-World bypass.
- Native cache policy:3 tests passed.
- Real fresh-process save test:34 Stage writes followed by36 hits; saved block, water volume5 and inventory preserved while procedural base remains unchanged. Original geology save test retained; new dedicated test is `world::storage::relief_cache_tests::relief_cache_preserves_saved_edits_and_water_across_processes`.
- A copied full-size cache entry was deliberately corrupted:488 hits +1 recompute/store, exact repair bytes and unchanged235,824 output blocks.
- Separate V13 headless smoke:180 ticks,0 failures, hash `64d10e01681ceed9`, matching V12. Not interactive/visual/FPS validation.

An initial private save harness omitted its counter receipt; write/read state assertions succeeded but were not accepted as the complete gate. The dedicated corrected test was freshly rebuilt and both phases rerun with explicit hit/write assertions.

## Foundry evidence

Current shared Foundry parses complete regional-cache counters without inventing FPS or per-chunk timings. Workload identity separates bypass-only/mixed/hit-only observations, so unlike cache states cannot silently pass an algorithm-regression comparison.28 distinct headless/diagnostic/native analyzer tests passed. Reports/AI packet: `D:/CodexArtifacts/axiom-foundry-regional-reports-20261110`. Those3 selected reports are individual observations, not the three-sample acceptance summary; source reconstruction is retrospective and hardware metadata is not run-attested.

## Delivery and evidence

Executable: `D:\CodexArtifacts\axiom-generation-speed-20261110\native-build-v13\bin\axiom-generation-optimized-v13.exe`
SHA256: `8e5f46e8714f6f1f44747779c957bbcf30466c721f85f6dfeb841bbdcdc518a7`
PDB and compiler/source bindings: `native-build-v13`. Both app/worldgen compiler artifacts were fresh. New runtime markers verified in the actual archived executable. This executable was not automatically launched and is not selected by existing launchers. To test it manually, use the project as the working directory (`C:/dev/voxel_v2_backup`) and launch this executable. No renderer/gameplay window was opened by this validation.

Evidence beneath `D:\CodexArtifacts\axiom-generation-speed-20261110`:
- `regional-cache-integration-receipt.json` and `regional-cache-integration-backup`.
- `regional-buffered-screen`, `regional-buffered-quarter`, `regional-buffered-corruption`.
- `regional-authority-tests`, `regional-buffered-units`.
- `regional-save-final-app-tests`, `regional-save-final-runs`.
- `regional-cache-delivery-source`, `native-build-v13`.

Next first-load hypothesis, not implemented: avoid the50 repeated intermediate16/8m solves with bounded in-memory retention. This requires a query-config lifetime audit because existing Levels keys omit voxel scale. Do not assume an extra OnceLock alone preserves all mixed-query behavior.
