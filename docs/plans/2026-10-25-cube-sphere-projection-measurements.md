# Cube-sphere projection measurements

Date: 2026-10-25. Branch `space-phase1`. Code: `crates/core/src/cube_sphere/distortion.rs` (distortion) and `crates/core/benches/cube_sphere.rs` (cost).

Input to W01's SDD, which picks the face-to-sphere projection. ADR 0024 is unchanged.

Three candidates are measured: Gnomonic, Tangent and Spherified. The COBE quadrilateralized spherical cube, the fourth candidate in the space port design (`docs/plans/2026-10-24-soa-space-port-design.md`, section 4.3), was not measured.

## Result

Distortion is `distortion::measure(p, 513)`: 513 x 513 cell centres over one face (the six faces are rotations of each other; `every_face_measures_the_same` checks it to 1e-9). Cost is the best of 7 passes (5 `just bench cube_sphere` runs and the 2 median-of-3 passes inside `just verify`), divided by the 1,024 calls in one iteration.

| Projection | `area_ratio` | `max_angle_deviation_deg` | `max_edge_length_ratio` | `to_sphere`, ns per column | `from_sphere`, ns per call |
|---|---|---|---|---|---|
| Gnomonic | 5.176 | 29.94 | 2.118 | 3.8 | 4.7 |
| Tangent | 1.412 | 29.90 | 1.412 | 8.9 | 11.7 |
| Spherified | 1.327 | 29.71 | 1.727 | 7.8 | 132 |

Field meanings: `area_ratio` is the largest over the smallest solid angle per unit of `(s, t)` area. `max_angle_deviation_deg` is the largest distance of the angle between the two tangents from 90 degrees. `max_edge_length_ratio` is the largest over the smallest arc length per unit of `s`. A map with no distortion reads 1, 0 and 1.

Reading the table:
- Gnomonic reaches 5.176 at 513 samples. The corner value is 3 sqrt(3) = 5.196; cell centres stop 1/513 short of the corner, so the measure sits 0.4% under it. `finer_sampling_approaches_the_limit_from_inside` checks that finer grids rise toward the limit and never pass it.
- Tangent area ratio is sqrt(2) = 1.414 between the face centre (largest area per unit of `(s, t)`) and the midpoint of a face edge (smallest); 1.412 measured. The corner is not the extreme for this map.
- None of the three is equal-area (ratio 1). The best measured is Spherified at 1.327.
- The angle deviation is 29.7 to 29.9 degrees for all three and tends to 30 degrees. Three faces meet at a cube corner, so the tangents there are 60 or 120 degrees apart on any map. The column does not separate the projections; the other two columns do.

## ADR 0024 figure

ADR 0024 states "area ratio about 1.6 with a plain gnomonic map". The measurement does not show that. The gnomonic area ratio is 3 sqrt(3) = 5.196 analytically and 5.176 measured. The gnomonic length ratio is 2.118 (analytically 3 / sqrt(2) = 2.121), so the length ratio is not 1.6 either. None of the three measured quantities gives 1.6 for gnomonic. The smaller figures belong to the other maps: 1.412 (Tangent area and length) and 1.327 (Spherified area).

## Determinism

Tangent's `to_sphere` calls `f64::tan` and its `from_sphere` calls `f64::atan`. Both come from the platform libm and are not correctly rounded, so results can differ in the last bits between the MSVC CRT and glibc. Gnomonic and Spherified `to_sphere` use only `+ * / sqrt`, which IEEE 754 rounds exactly, so they are bit-reproducible across platforms. Spherified `from_sphere` is not covered by that claim: its Newton loop stops on a `hypot` test, which is libm too.

Worldgen calls `to_sphere` once per column and determinism is a project invariant. W01 must either accept the libm difference for Tangent or ship its own `tan`.

## Recommendation for W01's SDD

Use Tangent if W01 accepts the libm difference above or ships its own `tan`. Its area ratio of 1.412 is 6% above Spherified's 1.327, its edge-length ratio of 1.412 is better than Spherified's 1.727, and its inverse is closed-form at 12 ns per call against 132 ns for Spherified's Newton solve. Gnomonic is out at an area ratio of 5.2.

If bit-reproducibility wins, use Spherified: it has the best area ratio, a cheaper forward map (7.8 ns per column against 8.9), and an expensive inverse that runs per query rather than per column.

If the SDD needs an exactly equal-area map, none of the three measured maps qualifies, and the COBE quadrilateralized spherical cube was not measured.

## Commands

```
cargo test -p engine_core --lib cube_sphere::distortion -- --nocapture
just test cube_sphere
just bench cube_sphere
```

The first prints the three `Distortion` values at 513 samples. `just verify` ran `just bench-check` as well. `just bench-save cube_sphere` then wrote the six baselines to `crates/core/benches/baselines.json` (median of 3 pinned passes on a quiet machine: `to_sphere` 3.969, 9.319 and 8.116 microseconds, `from_sphere` 4.730, 12.625 and 133.161 microseconds for gnomonic, tangent and spherified), and a later `just bench-check` passed all six within 3%.

## Machine and bench noise

CPU: 13th Gen Intel(R) Core(TM) i7-13700K (16 cores, 24 logical processors), Windows 11. `scripts/bench.py` pinned the benches to logical processors `0x5555` (one thread per performance core). The machine was shared with peer builds and test runs.

Passes 1 to 5 are one `just bench cube_sphere` each. Passes 6 and 7 are the two median-of-3 measurements `just verify` took in `bench-check`, on a quieter machine. Values are criterion medians for one iteration (1,024 calls), in microseconds.

| Pass | `to_sphere` gnomonic | `to_sphere` tangent | `to_sphere` spherified | `from_sphere` gnomonic | `from_sphere` tangent | `from_sphere` spherified |
|---|---|---|---|---|---|---|
| 1 | 15.614 | 43.927 | 28.102 | 22.583 | 56.498 | 330 |
| 2 | 8.569 | 39.994 | 52.559 | 12.893 | 44.262 | 282 |
| 3 | 4.628 | 22.120 | 10.335 | 8.083 | 16.364 | 143 |
| 4 | 5.501 | 15.188 | 39.302 | 8.233 | 24.734 | 158 |
| 5 | 6.272 | 33.965 | 10.916 | 7.875 | 17.369 | 140 |
| 6 | 3.842 | 9.097 | 7.985 | 4.820 | 11.997 | 137 |
| 7 | 4.092 | 9.874 | 9.179 | 4.993 | 12.779 | 135 |
| Best | 3.842 | 9.097 | 7.985 | 4.820 | 11.997 | 135 |

The same code reads up to 6 times slower between passes, so the best of 7 is the number to use and the ordering is the only firm result. Best-of-7 ordering: `to_sphere` Gnomonic < Spherified < Tangent (the `tan` calls); `from_sphere` Gnomonic < Tangent < Spherified (the Newton iteration). One chunk of 1,024 columns costs 3.8 to 9.1 microseconds in `to_sphere` on any of them, small beside the 8,333 microsecond fixed tick (1/120 s).
