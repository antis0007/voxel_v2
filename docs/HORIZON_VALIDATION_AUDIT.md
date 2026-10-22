# Main-thread horizon validation audit — 21 October 2026

Source audit requested by the horizon owner while the native motion route runs.
This identifies expensive paths; it does not establish measured attribution of
the 2,907.8 ms frame. The explicit horizon CPU bucket is now integrated.
The horizon owner retains implementation ownership of `horizon.rs` and its modules.

## Confirmed work outside the existing CPU buckets

`main.rs` calls `gpu.update_horizon` after its streaming timer ends and before
the render timer starts. Active upload checks, CPU-result acceptance and the
idle published/desired comparison can each validate dependencies in one frame.

`changes_visible_resident` scans all 32,768 block entries for every changed hot
Arc, then scans both complete partial-water maps. Only afterwards does it test
the accumulated bounds against current GPU coverage and occlusion.

`scope_covers_resident` visits every omitted region. Its `bounds_visible` fallback
calls `occluded_bounds`, which can try nine planes, each up to 4,096 cells.
Plane misses call canonical `world::sample_block` on the render thread. The eye
medium fallback can call it too. Region I/O is excluded, but generation is not.

The occlusion cache is thread-local. Proofs produced by a horizon worker are
not available to the render thread and disappear when that worker exits. The
render-thread cache also clears its entire plane map at 2,048 entries and its
generated-point map at 65,536 entries. Thousands of brick/plane/eye keys can
therefore repeatedly evict the working set. Moving the eye changes every key.

## Small exact fast paths available before a certificate rewrite

1. Before scanning a changed chunk, test its complete two-voxel halo against
   `ResidentCoverage`. If covered, none of its changes can affect this proxy.
   This is the same ownership condition already applied to each final bound.
2. Scan only the union of old/new cumulative `modified_bounds`. Both absent
   means generated content is unchanged; cold decoded chunks retain full bounds
   and therefore remain conservative. Keep additions/removals and record changes.
3. `WaterSnapshot.changed_chunks(old)` is now implemented in `world/water.rs`.
   It skips identical COW chunk maps and returns added/changed/removed old/new
   maps. Apply coverage to the chunk before comparing its cells. Compare both
   directions so removed partial entries still count.
4. Snapshot once and memoize validation within one frame per target/scope.
   Include current revision, region-record Arc identity, configuration, eye
   cell, coverage identity and omitted-scope identity. Region publication and
   replacement of an initially empty scope must invalidate the memoized result.

These fast paths and the water iterator are now integrated and compiled in the
pins-memo candidate. They preserve existing visibility semantics. They do not
justify stale TTLs, skipping publication checks or changing physics.

The retained-target route's CSV covers only its final stationary measurement:
1,411 samples, horizon p95 0.0526 ms and maximum 0.1249 ms. Earlier travel hitch
logs report at most 2.53 ms in that bucket. A 393.87 ms travel frame reports
327.02 ms render CPU and 0.04 ms horizon. The old 2,907.8 ms stall is therefore
not reproduced or causally attributed by this run. Full-route timing is still
needed before claiming a bound. Loading frame 036 independently fails motion
continuity, even though the recording mechanically completed.

`VOXEL_HORIZON_ROUTE=1` now enables a separate `route-frames.csv` from benchmark
world start through every render attempt, including loading, warmup and surface
errors. It records the monotonic route clock, camera, readiness, measurement and
physics flags, resolution and all CPU buckets. `route-timing.json` reports exact
all-frame means/maxima and peak timestamps, failed renders and ready-to-loading
transitions. Quantiles retain at most 262,144 samples and explicitly report
incompleteness for longer runs. This buffered diagnostic stream is separate from
`frames.csv`; capture-route diagnostics do not establish performance acceptance.
`measuring_at_render` describes state at render time, before warmup transitions,
and is false during loading. CPU timers exclude recorder I/O, loop idle and
asynchronous GPU work. Creation invalidates the old summary with
`recording_closed: false`; a closed recording alone does not indicate acceptance.
The combined structural candidate compiled successfully in 20.28 seconds
(`target/horizon-continuity/structural-build.log`). A native route with these
new files is still pending.

The first native structural component capture failed before rendering because
its frozen diagnostic shader referenced `debug_mode` outside scope. The run is
preserved as invalid, with no PNG. Build-script reuse failed to reject that copy;
the owner is forcing fresh validation under a distinct review package identity.
Compilation of the timing code above is separate from renderer readiness.

For the final stationary physics run, `VOXEL_RENDER_TIMING=1` enables the same
recorder without enabling the motion route. Adjacent CSV `seconds` values measure
render-attempt cadence, including recorder overhead and loop idle; the first
interval is unknown. They do not measure compositor/display cadence.
`VOXEL_BENCHMARK_NORMAL_PRESENT=1` uses the saved normal VSync preference and
the summary reports the actual requested auto presentation mode. The forest
review helper exposes these as `-FrameTiming` and `-NormalPresent`. These two
new options still await compilation.

## Bounded worker certificate

Have the worker return the successful occlusion proof with each omission,
instead of returning only its box. Capture the plane rectangle, a conservative
eye envelope, camera-medium assumption, source configuration and the chunk/
record dependencies actually used. Deduplicate shared chunk/plane footprints.

Main-thread validation should perform coverage-bit checks and dependency
comparisons only. It must never run procedural generation or cold I/O. Unchanged
hot Arcs and immutable cold record versions remain valid. A newly hot generated
chunk whose cumulative modified bounds miss the proof plane is also unchanged
there. For changed hot dependencies, inspect only the recorded plane cells;
partial-water volume is a separate dependency. Recheck the outside-water
assumption when validating a water occluder.

Camera movement may reuse a proof only while the newly projected footprint is
inside its certified rectangle and the eye remains on the certified side.
Otherwise request worker refresh. Coalesce refreshes and bound their total
footprints and main-thread work. An incomplete check is pending/invalid, never
implicitly valid. Retain only a cover whose own certificate is still valid;
revalidate published omissions before any busy-worker return can draw them.

Do not replace the TLS cache with a blocking global mutex: that can move the
same generation stall behind a lock. Do not solve this by raising cache limits
without a bounded footprint and measurements. Final acceptance needs the new
CPU bucket, a sustained physics/edit run and the camera/coverage transition.
