# Axiom native integration baseline

2026-10-26. Two manual native runs completed with one frozen integrated executable.
These are a baseline and observed limits, not graphics or simulation acceptance.
No automated tests or CA command scenarios were run.

## Provenance

- Frozen executable: `target/axiom-20261026/native-baseline-build/axiom-baseline.exe`.
- SHA-256: `4C61AAF01C2323B9FA3D77B82ECAC7D22955AEE4006E3E14AE7A6DDB0D2E1610`.
- Profile: debug/dev. The checkout contains concurrent work; these runs do not
  isolate the performance or appearance of this chat's changes.
- RTX 4090, Vulkan, 1280 x 720, seed 2738958700, generator 58, 1 m cells,
  four resident view chunks, partly cloudy noon, static eye at (0,127,-120) m.
- Requested cloud divisor 3; both measured reports used divisor 2 and reported
  zero configuration crossovers during measurement. Cloud history was disabled.
- Isolated generated worlds under each evidence directory; no player save opened.
- The native review script recorded arguments, environment, executable hash,
  process identity/exit, frame data and benchmark receipts.

The last transient-readback admission guard was compiled after this executable
was frozen. It affects the opt-in headless GPU adapter, which these runs do not
activate. The captures remain evidence for their exact frozen executable.

## Runs

| Evidence directory under `target/reviews/horizon-continuity/` | Static capture | Physics timing |
|---|---|---|
| Label | `axiom-g58-integration-still-001` | `axiom-g58-integration-physics-perf-001` |
| Requested measurement duration | 10 s | 15 s |
| Legacy physics | Disabled | Enabled |
| Screenshot readback | Enabled; 3 PNGs | Disabled |
| Exit | 0 | 0 |
| Measured frame samples | 1,019 | 1,324 |
| Average frame | 9.088 ms | 11.067 ms |
| Frame p95 / p99 | 9.784 / 10.203 ms | 16.358 / 21.640 ms |
| Maximum frame | 13.999 ms | 47.229 ms |
| Selected GPU passes, average | 8.905 ms | 9.901 ms |
| Cloud raymarch, average | 5.437 ms | 5.336 ms |
| Terrain GPU pass, average | 3.383 ms | 3.631 ms |
| Latest visible chunks / GPU meshes | 247 / 379 | 253 / 465 |
| Latest terrain triangles | 1,570,488 | 2,515,909 |

These are different workloads and final scenes. Do not subtract their times as
an isolated physics cost, claim a speedup, or treat screenshot-readback timing
as a controlled performance gate. The physics timing process audit recorded
61 samples with no other matching project executable, compiler or analyzer;
it does not establish that the whole computer was otherwise idle.

## Observations

The inspected image was
`target/reviews/horizon-continuity/axiom-g58-integration-still-001/bench-001.png`.
It shows the landscape, resident forest, distant terrain and held wand rendered
at the recorded aerial camera. No visible missing ground was identified in that
single image. This is not ordinary player-height or motion review.

The final static stream sample reported missing=0 with no pending work. The
physics run still had one missing resident mesh in late stream samples, a
nonempty streaming queue and 100,338 pending simulation entries at one sample.
A missing-resident counter alone does not establish a visible hole because
fallback coverage may exist. The simulation's last-step zero update counter
does not prove no earlier steps ran. These need route and scheduler evidence.

The physics run also logged a rejected stale horizon result at build/upload.
That shows the guard executed once; it does not prove all stale publications or
coverage transitions are correct. The Vulkan validation layer was unavailable.

## Gates still open

The proposed 8.33 ms normal-play target is not established by either run. The
physics run's tail remains above a stable 60 FPS frame budget at p99/max. There
is no matched before/after, release-profile, Intel/Iris Xe or low-preset evidence.

Gate A still needs motion, excavation and teleport coverage, command-to-visible
latency, bounded long-session queues and the appropriate hardware/preset matrix.
The new headless GPU adapter, exact checkpoint continuation and full-session
persistence have no runtime acceptance from these ordinary-renderer captures.
