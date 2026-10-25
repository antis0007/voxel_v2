# Cloud performance work, 2026-10-24

Target: at least 2x cloud rendering speed and game frame throughput. **Not achieved
at unchanged visual quality.** The final implementation improves the measured
partly-cloudy scene by 1.58x in FPS and 1.78x in cloud GPU time; storm gains are
1.42x and 1.50x respectively.

## Changes

- Include the existing compact-support fade in the cumulus rejection bounds.
  Cells that cannot improve the density envelope skip profile/lobe evaluation.
- Visit the containing cell and closest neighboring sides first, raising the
  bound sooner. Cumulus uses a commutative maximum; the order-dependent smooth
  maximum in the broad cloud field retains its original traversal.
- Reject pure cumulus below its existing condensation floor or above its cap;
  evaluate the broad bank first to tighten the thermal rejection floor.
- Bound the maximum possible cell presence before searching the neighborhood,
  and defer cell hashes until the corresponding geometry can contribute.
- Share the procedural density evaluator across view and light samples, and
  share the march across cloud layers. Preserve positions, weather lookups,
  weights, resolution, step budgets, lighting and temporal behavior.
  Explicit scalar accumulators are deliberate: the dynamically indexed vector
  variant produced a visible lighting discontinuity in native captures.
- Disable the overall automatic-quality governor in benchmarks, alongside the
  existing cloud governor, so a run cannot improve by changing its own quality.
- Add `tools/Measure-CloudPerformance.ps1` for fixed-scene native measurements,
  isolated saves, executable hashes, frame CSVs and separate capture runs.
  It yields to known peer renderer processes and aborts its own run if a peer
  starts. It never terminates a peer process.

## Build and measurement status

The isolated optimized build passed release-fast compilation and validation of
all 16 composed WGSL programs. Its native startup confirmed the build/runtime
world shader fingerprint matched (`d81875e65a1da1af`). No tests or CI were run.

The final shared checkout also built successfully in
`target/perf-cloud-2x/integrated`, validating all 18 current WGSL programs. Its
native capture run completed with a matching world fingerprint
(`f697bf6429cc0859`). That run includes peers' newer terrain and simulation
changes (1,888,706 terrain triangles instead of 1,081,510), so its capture-run
timing is not part of the controlled speedup comparison.

Evidence lives in `target/perf-cloud-2x/`. It includes the original dirty patch,
original shader files, frozen executables, a separate source snapshot, build
logs, run manifests, GPU timing logs and frame samples. The snapshot preserves
the starting work while other sessions implement simulation changes. Incremental
compilation was disabled after detecting stale include-string artifacts from
reusing the shared target directory. Later builds use their own target at
`target/perf-cloud-2x/build`, avoiding the executable used by peer sessions.
Frozen binary contents were also checked against their shader source text.

Initial profiling identified clouds as the largest GPU pass. However, another
session was running historical cloud comparisons, including binaries in its
own worktree. Those overlapping measurements are excluded from speedup claims.
One run also ended with a driver device loss and is excluded. The user approved
closing the regular game for measurement. A later peer review stopped producing
frames during loading for over nine minutes; only that stalled renderer was
closed, with its logs preserved in `peer-stall-54004`. Peer scripts and edits
were retained. Other live peer renderers cause the harness to wait or abort
its own run.

## Final native measurements

RTX 4090, Vulkan, 1280x720, release-fast profile; 10 seconds of warmup followed
by 12 measured seconds, with captures in separate runs:

| Weather | Before FPS | After FPS | FPS gain | Before cloud ms | After cloud ms | Cloud speedup | Before / after p95 frame ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Partly cloudy | 54.39 | 86.13 | 1.58x | 15.846 | 8.891 | 1.78x | 20.947 / 13.116 |
| Storm | 69.33 | 98.12 | 1.42x | 11.760 | 7.841 | 1.50x | 16.577 / 11.482 |

Runs: `solo-before-partly-2`, `final-after-partly-1`, `solo-before-storm-2`,
`final-after-storm-1`. `final-summary.json` derives the values from frame CSVs,
matching delayed GPU samples to measured frame IDs. An earlier clean partly
baseline was slower (47.97 FPS, 17.221 ms cloud time); the table uses the faster
repeat baseline to avoid overstating gains. Ordinary desktop run variability
remains; these are fixed-scene results, not a universal gameplay guarantee.

Both sides used the same 1,081,510 terrain triangles, 328 draws, 16-chunk view,
fixed camera/seed/time, automatic quality disabled, and temporal history off.
The requested cloud divisor of 3 is clamped to an effective divisor of 2 at
720p by the existing 640x360 minimum cloud target; both sides retain that.
Separate final native captures show the top 300 sky rows match within 1/255 RGB
for partly cloudy weather and exactly for storm. The lower image includes animated torch particles,
which also differ between repeat captures of the unchanged baseline.

The final candidates retain the original light-sample cadence and trigonometric
cloud shape. Longer lighting reuse, alternative harmonic math, scalar layer
storage, and removal of debug bookkeeping did not show a useful additional
gain and were excluded. Resolution, detail, view steps and lighting budgets
were not reduced to manufacture a 2x result.

Frozen binary SHA-256:

- `before.exe`: `BF9D6654B10C7C4792142647FC8508A08D2D5DB8DCEEE2C4DBCC15EC7EDCE778`
- `after.exe`: `DE4E22BB2A8243698F4872A999B64A13ED163316DE2E1192BFF952271A18D1A7`

The isolated source starts from `178db23a^` plus the initial dirty patch and
the existing untracked quality/activity modules. Both sides disable the
benchmark quality governor. Only the cloud shaders differ between the timed
binaries. The shared checkout additionally contains other agents' changes;
their game/terrain improvements are not attributed to this comparison.

## Reproduce

Run one renderer at a time. Use the same executable build profile, weather,
camera, resolution, seed and effective cloud divisor on both sides. Repeat
both sides in alternating order; compare cloud GPU time, total frame time and
tail latency separately. These fixed-camera benchmarks do not establish a
universal gameplay speedup or simulation performance.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/Measure-CloudPerformance.ps1 `
  -Exe target/perf-cloud-2x/before.exe -OutputDirectory target/perf-cloud-2x/before-clean-1
powershell -NoProfile -ExecutionPolicy Bypass -File tools/Measure-CloudPerformance.ps1 `
  -Exe target/perf-cloud-2x/after.exe -OutputDirectory target/perf-cloud-2x/after-clean-1
```

Repeat with `-Weather storm`. For visual comparison, use fresh output directories
and `-Capture`; do not mix screenshot readback runs into the timing comparison.
