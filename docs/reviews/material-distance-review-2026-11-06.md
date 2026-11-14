# Material distance review, 2026-11-06

The stone-brick floor now retains its 1.2 x 0.6 m courses after its 5 cm grain becomes subpixel. Native matched captures reproduce the user's close texture cutoff before the change and show legible courses toward the 16 m crosswall afterward at 1, 0.5 and 0.25 m voxel scales.

## Diagnosis and scope

- The stone-brick branch (surface kind 12) previously mixed the entire material into one mean color using the footprint of a 5 cm texel. The larger stone courses disappeared with the grain. Grazing horizontal surfaces reach that footprint earlier than upright walls. This reproduces the second reference without changing lighting, texture loading, or geometry.
- The first reference strongly matches `RenderDebugView::Clay` (32), labeled `Neutral Clay (fixed diffuse light)`. That branch deliberately returns fixed gray geometric-normal shading and bypasses material colors and scene lighting. A paired native clay/normal diagnostic reproduces the gray terrain and restores material colors in normal mode. The reference has no configuration manifest, so its exact debug state is an inference, not recovered metadata.
- Native clay captures also show prominent orbital cables. `mesh/models/orbital.rs::profiled_tether` builds minimum projected-width strips and stores physical coverage in the surface data. Clay returns before normal material appearance and makes that geometry conspicuous. Existing celestial projection code separately excludes those chart longitudes from the planet atlas. No cable or celestial geometry was deleted.
- Dotted near-terrain terraces and coarse near/far shape changes are separate voxel/heightfield representation issues. Existing horizon underlap is explicitly a compatibility overlap rather than stitched meshes. This material repair does not change that geometry, world generation, default generator version, residency decisions, exposure, palettes, material IDs, or player saves. Other materials retain their existing filters.

## Implementation

`world_shader.wgsl` now filters grain and courses at their own physical wavelengths. Mortar uses an analytical periodic box integral so its coverage survives after individual grain texels become unresolved. Coordinates remain world anchored; wavelengths do not scale with distance. Albedo, roughness and reflectance fade together. Course detail still fades when the course itself is unresolved, rather than retaining aliasing at the horizon.

The shader skips fine grain, chips and bevel work when its weight is zero, and skips course/mortar evaluation when courses are unresolved. This bounds unnecessary work but does not make the added visible detail free. A measured cost is recorded below.

`terrain_review.rs` adds an opt-in generated benchmark fixture (`VOXEL_MATERIAL_DISTANCE_REVIEW=1`) with stone, stone-brick and cobble lanes. It uses ordinary voxel edits and dirty-chunk tracking. It is disabled by default and is only called from the benchmark review path. At 0.25 m its one-time setup edits 359,424 cells; warmup excludes this setup from steady-state timings. The runner gives every case its own scratch save directory. Never enable the fixture against a player save.

## Reproduction and dev tools

```powershell
.\scripts\review-material-suite.ps1 -Executable C:\path\before.exe -Prefix before -EvidenceRoot D:\review
.\scripts\review-material-suite.ps1 -Executable C:\path\after.exe -Prefix after -EvidenceRoot D:\review
python scripts/summarize-material-review.py D:\review
```

Build both binaries from the same source snapshot with the fixture enabled only through the runner; vary only the material shader. The suite fixes the seed, camera, scale, viewport, weather, sun time and backend. Each run writes arguments, binary hash, environment, competing native/compiler processes at launch, shader build fingerprint, frame CSV and PNGs. `-Repo` and `-EvidenceRoot` support an isolated checkout on another drive.

The summary reports p50/p95/p99/max timings, worst frames with workload counters, pass ownership hints, missing visible/near chunks, mismatched arguments, shader provenance, changed draw/triangle counts and measured cost increases. GPU values cover selected timestamped passes, not total GPU frame time. CPU intervals overlap and must not be summed. Process contamination is sampled at launch, not monitored throughout.

## Captures and measurements

Evidence root: `D:\CodexArtifacts\visual-task17\evidence`. Final candidate: `optimized.exe`. The `after-*` directories contain an earlier implementation; use `optimized-*` for the integrated shader. All images are native 1280 x 720 captures, not mockups.

RTX 4090, Vulkan, view radius 12 chunks, seed 2738958700, 16 s warmup and 12 s measurement per static case. Numeric means are milliseconds. CPU frame values include scheduling/driver waits. These scenes do not benchmark 20 builders.

| Case | Frame before / after | Frame p95 before / after | GPU terrain before / after | GPU selected passes before / after | Compiler at either launch |
|---|---:|---:|---:|---:|---|
| m1 | 3.029 / 2.598 | 4.410 / 3.369 | 1.068 / 1.199 | 1.995 / 2.127 | Yes |
| m05 | 3.029 / 2.808 | 4.674 / 4.071 | 1.246 / 1.428 | 2.174 / 2.352 | No |
| m025 | 3.040 / 2.908 | 4.355 / 4.093 | 1.352 / 1.510 | 2.276 / 2.441 | Yes |
| m025-angle | 3.228 / 3.256 | 4.415 / 4.559 | 1.424 / 1.433 | 2.846 / 2.852 | Yes |
| m025-dusk | 2.955 / 2.880 | 3.796 / 3.831 | 1.799 / 1.686 | 2.749 / 2.630 | Yes |

The half-meter pair, with no competing compiler recorded at either launch, adds 0.182 ms to the terrain pass (1.246 to 1.428 ms, about 14.6%) and 0.177 ms to selected GPU passes. It preserves the larger material features that were absent before. This is a visual-quality improvement with a measured cost, not an overall performance speedup claim. Attempts to repeat the 1 m and dusk baselines also encountered another task's compiler; their manifests remain in `repeat-before-*`. Do not use the apparent CPU frame improvement as evidence of faster simulation.

All five static pairs use identical benchmark arguments and terrain draw/triangle counts. All ten report zero near and visible missing chunks during measurement. Final shader provenance matches its build in every run.

| Capture | Before | Integrated candidate |
|---|---|---|
| 1 m, noon | `before-m1/bench-001.png` | `optimized-m1/bench-001.png` |
| 0.5 m, noon | `before-m05/bench-001.png` | `optimized-m05/bench-001.png` |
| 0.25 m, noon | `before-m025/bench-001.png` | `optimized-m025/bench-001.png` |
| 0.25 m, grazing farther view | `before-m025-angle/bench-001.png` | `optimized-m025-angle/bench-001.png` |
| 0.25 m, dusk | `before-m025-dusk/bench-001.png` | `optimized-m025-dusk/bench-001.png` |

Normal camera: `0,-32,2,0,-0.28`; grazing camera: `-2,-24,2,0.18,-0.14`. The fixture aligns the eye relative to the floor across scales. Normal review time is 0.5, dusk is 0.72. Walls have one-voxel thickness, so wall thickness differs physically across scales; material wavelengths and floor camera distances do not.

The separate clay/normal diagnostic uses a generated hills world at 0.25 m and camera `464,-720,-710,0,-0.27`. See `diagnostic-m025-clay/bench-001.png` and `diagnostic-m025-none/bench-001.png`. Both have zero measured near/visible missing chunks and identical 570,311 terrain triangles. This diagnoses debug appearance; it does not claim to reproduce the reference's unknown world/pose.

The extra 24 s quarter-meter motion sweep (`optimized-m025-motion`) preserves visible course detail in captured poses, but records a transient peak of 56 visible missing chunks and 2 near missing chunks while changing view. It also ran with a competing compiler. Its mean frame time is 4.217 ms, p95 7.464 ms, terrain GPU 2.380 ms. Treat this as a streaming follow-up and a failed coverage-clean motion qualification, not a verified geometry or motion-performance improvement. No geometry was hidden to reduce these counters. Static images alone do not establish temporal shimmer quality.

## Validation and integration

- Release-fast offline locked native build passed; all 27 composed WGSL programs validated with Naga. Final world shader FNV-1a fingerprint: `2063b2ebf439c9b4`.
- Existing shader contract tests ran directly with `rustc --test`: 4 passed. This covers shader inclusion, file existence, the single pixel-grid rule and retired helpers; it is not the full application test suite.
- Periodic mortar integral checked against 16,384-sample quadrature in 208 cases, including negative coordinates and spans beyond one period. Maximum error: 0.0000813802. This is a numerical algorithm check, separate from native WGSL validation.
- PowerShell runner/suite parse checks and Python compilation passed. The portable suite executed the final five native cases, and its summarizer flagged cost increases and coverage/competition issues.
- `just session-report` completed. Its aggregate repository counts and old benchmark results include other work and are not measurements of this change.
- The scoped shader and fixture patch applied to the shared tree without conflict. Normalized source hashes exactly match the native-tested snapshot. Existing staged/unstaged renderer and worldgen edits were preserved. No shared commit or production executable replacement was made because those files contain other tasks' changes. Rebuild the normal launcher from the integrated source; do not replace it with this isolated snapshot binary.

Shared source hashes (UTF-8 text after newline normalization):

```text
world_shader.wgsl 4cf6ea027c0ec54224161eaec108a42bca1ae63bc8a9bf61e179777b1f1c478e
terrain_review.rs ceb5df3f0b6721895bc00a301d879682b207f7e11af45d461efc0c046333564c
```

## Remaining work

The material change covers cut stone only. It does not repair terrain terraces, heightfield/resident transitions, moving-view residency gaps, or builder throughput. The shared mountain and streaming tasks were read before integration; their edits were left intact. Their benchmark results must be evaluated independently. The original screenshots lack seed, camera, build and debug manifests, so claims about their precise loading state would be speculative.

Library copies for review:

- [Quarter-meter floor before](https://chatgpt.com/api/library/files/libfile_68a4ef2899888191949ea57b7b793896/download)
- [Quarter-meter floor after](https://chatgpt.com/api/library/files/libfile_a13f1475e71881919f1224e53554087e/download)
- [Clay diagnostic](https://chatgpt.com/api/library/files/libfile_847161ae88bc81918aa7135a0b4fbde2/download)
- [Normal diagnostic](https://chatgpt.com/api/library/files/libfile_447b050cf1c48191bfe85fe90e0ee76a/download)
