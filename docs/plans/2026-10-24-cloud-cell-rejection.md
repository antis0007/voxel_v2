# Cloud cell rejection optimization

This increment retains the existing cloud resolution, density functions, ray
steps, quadrature, lighting taps and shadow reuse cadence.

`broad_cloud_mass` now rejects an absent bank before computing its centre and
ellipse parameters. A second conservative bound uses the distance to the cell
rectangle: the bank centre lies within that rectangle, and the longest ellipse
axis is at most 0.68 cell widths. Dropping the nonpositive vertical and taper
terms produces an upper bound. A candidate at least the smooth-max radius
(0.30) below the accumulator cannot contribute. The rejection includes a 0.001
rounding margin and retains the original accumulation order.

The ray march also stops once its existing distance fade reaches zero. Distance
increases along the ray, so later samples cannot contribute radiance, opacity
or depth. Diagnostic march-step counts can decrease.

The measurement harness recognizes suffixed `before-*` and `after-*` renderer
names as peers. It waits for them and excludes overlapping runs without closing
another session's processes. An optional `WorkingDirectory` allows both runs
to load the same frozen assets rather than the changing shared checkout.

## Validation

Both isolated executables built with the local `release-fast` profile and
incremental compilation disabled, validating 19 composed WGSL programs. Their
world shader fingerprints match `0b51e2f278e385f4`. Hash manifests of all 518
frozen inputs confirm that only the two cloud shader files differ. The binaries
contain the expected baseline/candidate shader text. Source snapshots, build
logs, manifests and binaries are in `target/cloud-refine-20261024/`.
No tests or hosted CI were run.

The user requested finishing without timing claims while leaving the other
renderers running. No benchmark or visual comparison remains queued. Baseline runs were
stopped when unrelated renderers started; their `overlap.json` files exclude
them from performance claims. Early shared-checkout captures also have
`INVALID.txt` markers because world-generation inputs changed between builds.
The final source and build directory are isolated to avoid both source changes
and stale shared Cargo artifacts. Compilation alone does not establish a
speedup or visual equivalence.
