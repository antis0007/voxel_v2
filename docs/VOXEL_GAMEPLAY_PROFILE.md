# Cubic terrain and scale-aware gameplay review

User direction: restore Minecraft/Noita-style cubic terrain and make building/mining practical at 0.5 m and 0.25 m. No tests or CI were added or run.

## Changes

- All opaque terrain uses greedy cubic faces. Natural-material relaxation is removed from the active mesh path. Cave/overhang topology and sharp constructed geometry remain voxel-exact.
- Clean meshes with distance-independent geometry are retagged when crossing LOD bands, without source regeneration, remeshing or another GPU upload. Torch geometry still requests its distance-specific representation. Dirty chunks still rebuild.
- Removed the source array that distinguished authored voxels for smoothing; cubic geometry needs no authored-mask allocation.
- Outer distant-backdrop rings sample the deterministic base landscape without generating eroded tiles. The two inner rings retain exact surface sampling; approximate cached samples upgrade when needed nearby. Resident terrain generation and saved generator versions are unchanged.
- Default hand edits cover one physical metre: 1, 8 or 64 voxels at 1, 0.5 or 0.25 m. Grid-aligned widths are adjustable, with a separate single-voxel/detail mode. Models and torches remain single objects.
- Brush radii, area depth, wand face area and explosions support metre units. Area erasing cuts into the targeted face. Preview sampling no longer truncates committed edits.
- Edit strokes use an asynchronous source loader, then apply at most 256 cells / 2 ms per frame. Placement rechecks occupancy and the player's body. Opening/closing panels or changing worlds can cancel remaining work.
- Stationary previews are cached by world revision, target and settings, avoiding repeated connected-face searches.
- Upgraded bore mining shares the queued edit path and scales its candidate allowance with voxel volume.
- Terrain generation can use eight workers on larger CPUs, leaving four logical processors for other work. Running/completed source arrays remain covered by the existing admission reservation.
- Default storage roots are checked for write access once; unavailable locations fall back to another writable default. Browser discovery retains previous roots. An explicit `VOXEL_SAVE_DIR` remains authoritative and reports its own errors.

## Captured release measurements

NVIDIA RTX 4090, Vulkan, 1600 × 900, view distance 16 chunks, Hills seed 2738958700, partly cloudy noon. Single game process during each profile. Warmup 30 s, measured interval 15 s. Frozen snapshots prevent concurrent chats from changing code during compilation.

| Captured build/workload | Average FPS | Mean frame | P99 frame | Terrain GPU | Cloud GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Previous sculpted build, static, physics off | 194.62 | 5.138 ms | 7.084 ms | 1.639 ms | 2.172 ms |
| Cubic candidate, static, physics off | 271.54 | 3.683 ms | 4.891 ms | 0.899 ms | 2.280 ms |
| Cubic candidate, quarter-metre moving sweep, physics on, 32 MiB sources / 2 GiB GPU | 213.31 | 4.688 ms | 7.713 ms | 0.802 ms | 2.245 ms |
| Final release, same quarter sweep, physics and successful region writes | 176.52 | 5.665 ms | 10.431 ms | 0.803 ms | 2.276 ms |

Static throughput increased 39.5%; mean frame time fell 28.3%. These compare the captured old/new builds with the same camera/preset, not an isolated toggle: concurrent backdrop/UI changes are included, and visible terrain counts differ. Cloud rendering is now the largest steady GPU pass.

The previous ground-view backdrop first completed in 19,056 ms. The new static backdrop built in 112 ms (quarter-metre run: 89 ms). The latter includes the current backdrop geometry/proxies as well as the sampling change.

During the quarter-metre sweep, sources repeatedly evicted while GPU geometry stayed available: 10,310 source evictions, zero GPU evictions and zero missing visible chunks at second 42. At second 45 there was one incoming missing chunk during motion. Sources were about 28–31 MiB; GPU geometry was about 500 MiB. The 32 MiB setting limits source residency, not total process memory or mesh-worker scratch.

The first physics sweep reproduced a denied AppData region write in the managed execution environment. This prompted the storage fallback fix; its numbers above are not clean save-success measurements.

The final release uses eight terrain workers on this 24-thread machine. Its clean repeat first reached fully expanded / zero missing visible chunks at 20.01 s, versus 38.00 s for the initial four-worker quarter sweep. At 43/44 s it still had zero missing; at 45 s movement introduced two incoming chunks. There were no GPU evictions. This improves observed coverage latency; it does not establish an isolated FPS gain, since successful physics persistence and more completed geometry also change the workload.

Storage fallback was verified without `VOXEL_SAVE_DIR`: denied preferred directories led to `target/measured-review/final-stream/saves`, where 27 region files totaling 151,522,056 bytes were written. The subsequent clean repeat used an explicitly isolated save root, wrote 28 region files, and had an empty error log. Its final renderer captures were inspected without the save-warning overlay. The intermediate final-stream FPS was excluded from the headline comparison because other chats were compiling during that run.

The final native build was also launched and its world browser inspected. Interactive tool manipulation was interrupted by active human desktop input; the review instance was closed. Metre-mode behavior is implemented and compiled, but this pass does not claim a completed manual mining/building session at either fine scale.

## Evidence and limits

- Initial candidate: `target/measured-review/candidate-source`, `candidate-build.log`, `voxel-gameplay.exe`.
- Static profiles: `target/measured-review/baseline-static/run.log` and `candidate-static/run.log`; renderer captures in each directory.
- Quarter sweep: `target/measured-review/quarter-stream/run.log` and `captures`.
- Final release: `target/measured-review/voxel-gameplay-final.exe`; launch with `run-voxel-gameplay.cmd`. Frozen source, source hashes and executable SHA-256 are in `final-source`, `final-source-manifest.json` and `final-build-receipt.json`.
- Final clean run: `target/measured-review/final-clean-stream/run.log`, `error.log`, `captures` and `saves`. Save-fallback run: `target/measured-review/final-stream`.
- Source checks and release compilation validate Rust and composed WGSL. Actual renderer PNGs were inspected separately.
- Nearby terrain is cubic. The distant backdrop is still a heightfield approximation, and cannot represent remote caves/builds. Fine cloud patterning, bright distant slopes and horizon lighting remain visible in this snapshot; related chats own those art improvements.
- A 512 MiB GPU budget still defers outer exact meshes at this view distance; the backdrop supplies their distant surface. More source memory alone cannot resolve a GPU geometry budget limit.
