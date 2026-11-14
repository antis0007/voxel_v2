# V14: exact call-local terrain sampling reuse

## Delivered
Integrated 11 source/test files after guarded comparison with V13 and backed up prior files. Separate playable:
`D:\CodexArtifacts\axiom-generation-speed-20261110\native-build-v14\bin\axiom-generation-optimized-v14.exe`
SHA256 `85bc07f8475031ad5e56694fbbd3f2ae1e1e168e05fedaec6abc9612c6e127d8`.
Run with project working directory `C:\dev\voxel_v2_backup`. Existing launchers were not modified and do not automatically select this artifact. V13 remains unchanged. No interactive/native visual or FPS acceptance is claimed.

## Implementation
A stack-local SurfaceQuery binds one immutable World and exact direction. It lazily reuses that point's World-mesh interpolation stencil and scalar edifice/raw-volcanic-base results across a regional node's volcano shaping, ground height, historical column, collision, sediment cover and fault damage. There is no persistent cache, extra worker thread, heap map or added lock. Existing standalone interfaces remain wrappers.

The exact interpolation sum order, eased tangent-plane reconstruction, distinct volcanic-base formulas, material history early exits and TLS memo behavior are preserved. Crater callbacks at vent directions use the original independent World query, not the outer coordinate. The Drift mesh is never confused with the World mesh. Solver equations, generator versions, cache format, save serialization and Stage identities are unchanged.

The rejected retained-parent proposal is NOT included. Reconstruction-only's earlier 2.13% screen was not delivered separately; this is the larger coherent reduction.

## Measured default workloads
Three interleaved baseline/candidate pairs per scale and cache policy, fresh processes, seed 74 generator 287 Planet with no custom recipe. Six sequential production padded chunks, 235824 blocks. Global geology is already solved/loaded; cold means the regional relief cache starts empty. Timings cover chunk generation, NOT the entire first-ever planetary solve, native time-to-play, concurrent streaming or FPS.

| Scale/policy | V13 median | V14 candidate median | Change |
|---|---:|---:|---:|
|1m cold regional cache|26.872785s|23.540008s|-12.40%|
|1m warm regional cache|1.214241s|1.180479s|-2.78%|
|Quarter cold regional cache|9.634012s|8.472240s|-12.06%|
|Quarter warm regional cache|0.501888s|0.492224s|-1.93%|

Cold regional construction improves about 12% at both scales. Warm changes are small; this is not another claim of large warm-cache improvement. The quarter fixture's surface coverage was not independently established; the 1m fixture includes river surface height 31..64 m.

Actual Windows whole-process CPU time, including warm-geology setup: quarter cold median 22.578125 to 21.328125 CPU-seconds(-5.54%). Quarter warm CPU 14.687500 to 14.843750(+1.06%) is retained rather than hidden; no warm total-CPU gain is claimed. No additional parallelism was introduced.

Every benchmark checksum matches V13:1m `686900d727e4a31d` with 149328 solid blocks; quarter `17e3caeee32aecaf` with 90144. Cache operations are unchanged:1m: 439 unique cold stores plus 50 same-session hits; warm: 489 hits. Quarter: 220 cold stores or 220 warm hits. No reported cache errors. Prime runs are explicit and excluded from warmed-geology comparisons; no failed runs or outliers were silently removed.

## Verification
- Fresh production probe `01f5c0aed143ddf556b364adac3812b5747e94fce7622a6fba495aecd4a51987`; baseline `dc440990150bd05eb667c2e24ffa87e47ea0c5ae206f22a5766dbb65bb0f5244`. Compiler artifacts explicitly fresh=false and binaries archived before later test additions.
- Independent literal-original reconstruction unit passed, including regular/irregular meshes, all nodes, seams/corners and extreme/signed-zero fields.
- 12 actual-world cases passed: two independent tests each for 287 seeds 74/731 and 275/280/283/286 seed 74. Compare every Ground/Shaped/history-column bit and complete 32/16/8/4 m Stage arrays, parent chains, missing history, empty optional edifice, forced trim/deposit/lava branches, history lazy early exits and memo hits. Frozen references do not call SurfaceQuery or prepared helpers.
- Independent applied-code review accepted exact arithmetic, coordinate/mesh binding and side-effect boundaries.
- Fresh native worldgen/app artifacts, source manifest and binary hashes retained. 180-tick headless smoke passed, hash `64d10e01681ceed9`, 0 failures.
- Fresh app test binary passed real two-process private-save write/read, preserving edited blocks, neighbor cells, saved water volume 5 and inventory 73 stones. Read phase used warm geology and regional caches. No user save was accessed.
- Final 11 shared source hashes match integration receipts; V13 executable hash reverified.

Full workspace CI/Clippy was not rerun; this is focused engineering verification. Existing unrelated full-workspace Clippy issue in core/fast_hash.rs is not represented as fixed.

## Evidence and caveats
Private source and evidence are under `D:\CodexArtifacts\axiom-generation-speed-20261110`: `node-sampler-source`, `node-sampler-delivery-source`, `node-sampler-binary`, `node-sampler-tests-binary`, `node-sampler-screen`, `node-sampler-quarter-cpu`, `node-sampler-test-runs`, `node-sampler-save-app-tests`, `node-sampler-save-runs`, `native-build-v14`, and guarded integration backup/receipt.

`D:\CodexArtifacts\axiom-foundry-node-sampler-20261110` contains 24 validated raw-log ingestion reports and four like-policy comparisons. Source manifests there are explicitly retrospective, not invented benchmark-time source attestations; native build has its own pre-build source manifest. Hardware is this one test computer, not a cross-device performance promise.

The new executable conservatively invalidates executable-bound procedural caches. Its first launch may therefore solve geology again; subsequent launches retain V13's persistent reuse. Cache behavior and user-save overlays are otherwise unchanged. No renderer, climate realism, NPC performance, multiplayer behavior or overall gameplay rating improvement is claimed from this work.
