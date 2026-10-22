# 0012 Horizon representation: measured, redesign recommended for the near band

Date: 2026-10-22. Status: proposed (the decision is the user's).

The horizon beyond the resident ring is a heightfield built from `distant_surface`. Chunks are built from `sample_block`. Both read the same `sample_column`, so they agree on the surface at knots, but not between knots or where the voxel world adds or removes cells. `crates/worldgen/src/horizon_agreement.rs` measures both errors in the 320–1536 m band, where the horizon's 8 m fine grid (`crates/app/src/render/horizon/fine.rs`) meets resident terrain. It uses seed 424242, about 4,096 columns, all four quadrants, and the fine grid's own triangle diagonal.

| World, scale | Exact columns | Above / below heightfield | Divergence p95 (voxels) | Interpolation p95 at 8 m | 16 m | 32 m | 64 m |
|---|---|---|---|---|---|---|---|
| Hills, 1 m | 91.3% | 8.1% / 0.6% | 7 | 1.75 vx (1.75 m) | 4.00 m | 8.75 m | 16.8 m |
| Hills, 0.25 m | 93.6% | 5.9% / 0.5% | 16 | 6.25 vx (1.56 m) | 3.91 m | 8.69 m | 16.8 m |
| Ridged, 1 m | 91.3% | 8.1% / 0.6% | 8 | 1.75 vx (1.75 m) | 4.25 m | 9.62 m | 19.2 m |
| Ridged, 0.25 m | 93.5% | 6.0% / 0.5% | 18 | 7.00 vx (1.75 m) | 4.25 m | 9.66 m | 19.1 m |

Decision rule, fixed before measuring: keep the heightfield if the 8 m interpolation p95 is ≤ 1 voxel and at least 90% of the divergence is explained by features (trees, structures).

- **Divergence passes.** About 93% of mismatching columns are features above the surface. Only about 0.5% are carved below it.
- **Interpolation fails.** The p95 at the 8 m grid is 1.75 voxels at 1 m and 6–7 voxels at 0.25 m. It doubles with each coarser ring, reaching 17–19 m at 64 m.

Recommendation: the error is in the representation, not in the source data. Linear triangles over a stepped voxel surface miss by about half a cell times the slope, which is the shader comment's own diagnosis. A design should replace the near band (inside about 1.5 km) with coarse voxel-mesh tiles derived from `generate_padded`, the same data the chunks use. The heightfield should stay for the far rings. Beyond about 6 km a pixel covers roughly 6 m at 1440p, so the error there is a few pixels of silhouette, not a visible step. That design gets its own plan; nothing changes until the user decides. `horizon_agreement_stays_within_measured_bounds` pins today's numbers, so a regression or an improvement shows up in `just test`.
