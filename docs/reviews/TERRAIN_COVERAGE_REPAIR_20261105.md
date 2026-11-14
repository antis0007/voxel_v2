# Terrain coverage repair — 2026-11-05

Continues the pasted LOD/streaming handoff in `C:\dev\voxel_v2_backup`.

## Source changes

- `render/streaming/visibility.rs`: retain the inherited actual-crown height bound. Match the authoritative voxel generator's tree-cell search (two cells either side of every voxel in the column), including the previously omitted positive outer row. Use `TreeVisual.radius` directly: it already contains the generator's leaf envelope. Keep the existing terrain/water/landmark bounds and world-height clamp.
- `render/streaming/terrain.rs`: restore the handoff's missing shallow FIFO admission policy. Count jobs waiting to start, admit at most two waiting jobs per worker, and retain the existing byte reservation and pending-result limit. Both terrain generation and activity-seed jobs count against admission. Starting a job releases a queue slot; collecting its result releases the memory reservation.
- Update the existing crown coverage check to inspect the generator's complete cell range and actual leaf envelope. Add two admission checks for worker backlog and uncollected-result pressure.

No render-distance defaults, shaders, generation rules, saved-world formats, edits, or multiplayer protocols changed in this pass. Other sessions' working-tree changes remain present.

## Focused validation

The app test target cannot compile because unrelated test fixtures lack `GeneratedChunk::water_motion` and bed tests borrow their clock mutably and immutably in the same call. No test modules were disabled or stubbed.

A standalone Rust harness extracts the current column/crown/admission functions and their focused checks, links the built world-generator dependencies, and constructs the same seed-7 configuration. Artifacts are under `target/coverage_repair_*` and logs under `target/coverage-repair-*`.

- Fixed harness: **3 passed**. Hills and Ridged, each at 1 m and 0.25 m, cover all sampled crowns: 2,765 overlaps per 1 m case and 542 per 0.25 m case, zero exceeding the loaded height range.
- Negative control: removing only the actual-crown contribution reproduces **12 / 2,765** clipped Hills/1 m overlaps, worst **8 voxels** beyond the resident range. This establishes the need for the inherited crown-height fix; it does not isolate the outer-row correction's visual effect.
- Changed-source whitespace check passes.

These checks prove sampled bounds and admission arithmetic, not universal visible coverage.

## Native validation

`cargo build --offline --locked --profile release-fast -p minecraft_style_voxel_prototype --bin minecraft_style_voxel_prototype` passed (112 warnings). Executable SHA-256: `4CA0289D8240782D050332C9821A685F9AAA3EE6250353753F6556997D83A089`.

Two sequential native runs used seed 7, Hills, orbit motion, 960×540, five-second warmup, twelve-second measurement, and PNG captures every two seconds. Logs: `target/coverage-repair-native-1m.log` and `target/coverage-repair-native-quarter.log`. Each corresponding directory contains six `bench-*.png` frames and `frames.csv`. The mid/final 1 m and final quarter-metre images were visually inspected; no obvious open terrain gaps were visible in those sampled views. This is not an exhaustive motion review.

| Measurement | 1 m | 0.25 m |
| --- | ---: | ---: |
| Measured frames | 1,611 | 1,972 |
| Frame p95 / p99, ms | 9.296 / 14.159 | 7.896 / 11.490 |
| GPU pass average, ms | 6.718 | 5.175 |
| Peak missing near chunks | 39 | 47 |
| Frames with missing near chunks | 380 | 1,953 |
| Peak queued/running requests | 140 | 734 |
| Final queued/running requests | 7 | 584 |
| Peak resident GPU mesh MiB | 963.80 | 229.65 |

The queued/running request count includes the scheduler backlog and meshing; it is not the terrain FIFO length. Frame pacing logged surface timeouts during startup. There is no matched native baseline, so these are after-change measurements, not a claimed performance gain. The two scales also cover different physical distances under the current settings, so their throughput numbers are not directly comparable.

## Remaining scope

The crown clipping defect is repaired and shallow terrain admission is restored. The quarter-metre run still has substantial streaming backlog. Missing-chunk counters do not classify geometry/air/underground or measure uncovered visible area. High-speed straight/reversal/descent trajectories, large coordinates, edit/multiplayer replay, matched native before/after evidence, and LOD transition dithering remain unverified. Do not label the overall terrain pop-in problem solved from this pass.

Changes are left uncommitted alongside the inherited work; no unrelated changes were staged or reverted.
