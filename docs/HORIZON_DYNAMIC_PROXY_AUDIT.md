# Dynamic source coverage audit — 21 October 2026

Status: production proxy implementation handed back to the build owner; combined
compile, contracts and native verification pending. Source parsing passed. This
document does not establish runtime acceptance or authorize weaker freshness checks.

The rejection diagnostic found real free-water volume changes outside resident
ownership (including volume 5 to 7 and 5 to 6), while long monolithic horizon
jobs were running. Retaining published columns prevents loading transitions;
it does not establish that subsequent horizon jobs publish under active physics.

## Prefer bounded resident proxies before a second horizon pipeline

Reuse ordinary edited-source generation, meshing, upload and published-column
ownership for a supplemental set of columns containing existing activity, with
a fixed 3 by 3 XZ collar. Keep the ordinary view radius and simulation scheduling
unchanged. Source-only requests must not discover new simulation activity.

The required changes are narrow but adding keys to `visible` alone is unsafe:

| Area | Required contract |
| --- | --- |
| `streaming.rs` cache state | Separate requested proxy columns/member coordinates from admitted GPU ownership. Include pending members in source interest and scheduling, while preserving the previous complete published bundle. Missing source must never acquire an owner bit. |
| `set_horizon_pins` | Existing function retains already published GPU allocations; it cannot request missing columns. Count retained pins and supplemental admission together against 512 columns and 1 GiB. Defer excess explicitly. |
| Desired LOD | Force source proxy LOD 0 consistently in desired keys, ready/result checks and upload repair. `lod_for_chunk` normally selects coarse LOD beyond the view radius. |
| `columns.rs` / `visibility.rs` | Add explicit supplemental interest to `ColumnPlans`; its radial interest currently discards outside plans. Require the exact completed plan before admitting ownership. Include edited Y coordinates, topology halo, roof and crown support. The distance-thinned ordinary Y spans are insufficient for arbitrary underground edits. |
| `terrain.rs` / seed admission | Carry source-only provenance. Current worker calls `simulation::discover`; `streaming.rs` queues the mask, then `main.rs` merges it into `world.activity`. Suppress that admission for proxies. Later ordinary-interest promotion needs explicit one-time discovery/admission even if its source is already cached. |
| Region loading | `WorldSnapshot::edits_ready` requires persisted records in the source chunk's 3 by 3 by 3 neighborhood to be hot. Both scheduling and terrain collection wait on this. Normal region read planning and `snapshot_near` use radial interest, so supplemental columns need explicit bounded source-load/retention interest for that halo. Requeueing alone cannot make a cold remote edit ready. |
| Queues | Prioritize missing supplemental bundles and invalidated edits, retaining existing bounded worker/upload budgets. Current ready priority is missing mesh, finer replacement, then everything else; dirty existing meshes have no separate priority. |
| Publication | Publish coverage only for a complete GPU-ready vertical member set. Keep old coarse/published cover while any replacement member is pending. Validate the parent against current source outside actual admitted coverage. |

Candidate activity must come from already active or dirty simulation coordinates,
not discovery of generated proxy sources. The simulation has a global fair lane,
so its domain is not guaranteed to fit the cap. If the activity plus collar is
too large, retain valid cover and report deferred columns; do not silently widen
physics or claim starvation is solved.

`World::stream_regions` and `install_disk_chunk` load edits without merging
activity. No `drain_ready_sources` or `seed_activity` symbol exists in the audited
tree. The concrete seed path is the terrain worker, cache seed queue and the two
main-loop merge sites.

Required runtime evidence is continued useful publication during active physics,
complete ownership at column/corner boundaries, no proxy-induced activity growth,
bounded memory and queues, and unchanged source checks outside admitted cover.
Strict resident mesh revision checks can still reject genuinely changing local
results; proxy coverage must not be advertised as a proof of universal progress.

### Implemented handoff

`streaming/edit_proxies.rs` implements the requested set separately from normal
visible coordinates. New candidates are restricted to columns outside the ordinary
XZ radius; existing proxies remain valid when the player approaches. The collar
uses 10 metres rounded up to chunk columns, with at least one column. Exact worker
column plans include hot and cold edited heights, vertical halo and 26-metre crown
support. At most eight incomplete bundles enter source scheduling concurrently;
each update returns at most 128 request keys. Pending membership never grants an
owner bit. Previously admitted complete bundles remain drawable during expansion.

Retained pins and proxies share the 512-column / 1-GiB pool. GPU eviction protects
both admitted cover and partial active bundles. Existing worker counts, CPU source
reservations, mesh budgets and upload limits remain the admission mechanism.
Dirty repair and proxy requests receive priority over speculative LOD renewal.

Terrain requests carry an activity-discovery role. Render-only generation skips
discovery and seed merging. Already cached sources promoted to ordinary interest
use bounded background discovery jobs, with one successful admission per coordinate
until source-interest eviction or world reset. No procedural generation or region
I/O was added to the frame thread.

The implementation compares one retained water snapshot against the current one
to detect volume-only changes. It requires actual value differences and hot source
membership in both snapshots; changing snapshot scope alone cannot create a proxy
edit. Missing incoming dirty events enter the existing dependency-halo revision
repair path before completed meshes are considered for publication.

Integration APIs are `update_edit_proxies(snapshot, dirty_chunks, world_revision,
parent_revision)`, `edit_proxy_source_coords()`, `edit_proxy_columns()` and
`retire_edit_proxies(baked_columns, parent_revision)`. Retirement requires an
explicit parent baked/not-omitted proof and a revision at least as recent as the
last local change. Update dirty revisions before evaluating retirement. Normal
future-parent desired coverage excludes supplemental proxy ownership.

The build owner added main-loop glue and explicit source-only region interest.
The source receipt is
`reviews/forest-repair-20261021/physics-window/edit-proxy-source-handoff.json`.
This chat ran source parsing only; it did not compile, launch, or add/run tests
during the proxy ownership window.

### First native proxy run: liveness still fails

The combined build passed the build owner's 44 renderer contracts and the real
cold-source loading/no-activity/release check, and completed an application smoke
capture. `D:\voxel-horizon-review-20261021\proxy-physics-120` then ran 120 seconds
of active physics at 2560 by 1440, generator 25, High/divisor 3, with no readback.
The executable SHA-256 is
`019CD80DF10ACAF0611BA22AE8ECCDCAFCAF88B44A4171280C5F6DC0FF7D4503`.

There were zero ready-to-loading transitions and failed renders. However, parent
publication stopped after nine publishes, last bracketed at stream seconds 24–25;
sixteen completed CPU builds were rejected, with no parent publication for at
least the final 113 seconds of the route. The proxy lane published 114 meshes,
but only four complete columns were admitted out of 118 requested. The oldest
pending column reached 118.5 seconds. Pool usage was 573,754,320 bytes, with zero
cap deferrals, so memory admission was not the observed bottleneck.

The existing ready priority puts ordinary dirty repairs ahead of new proxies.
Finishing the oldest pending proxy bundle before repeated ordinary repair is
the next narrow scheduling correction. Local freshness checks must remain intact.

The 8,046 measured work samples had p50/p95/p99 **14.583 / 16.531 / 18.747 ms**,
maximum 70.116 ms. Selected GPU passes averaged 13.972 ms, including clouds
10.605 ms. This is a numeric budget result from the liveness diagnostic, which
included bounded rejection tracing; independent process/timing acceptance and
the starvation fix remain open. The candidate's printed literal zero for
source-only seed admission describes the source policy, not an observed counter;
the next source version labels that policy `source_only_discovery=disabled`.

Readonly interpretation is retained in
`reviews/forest-repair-20261021/physics-window/proxy-physics-120-readonly-review.json`.

### Pending-bundle priority correction handed back

The next source version adds `EditProxies::pending_since` and prefixes the shared
ready ordering with coverage class and original request time. A missing ordinary
GPU member whose column has no existing published bundle keeps emergency priority.
Next comes the oldest scheduled, not-yet-admitted proxy column. Existing dirty
repair and LOD ordering follows. The same helper orders CPU mesh dispatch and GPU
upload, so steady ordinary repairs cannot monopolize either completion stage.
Adding a Y member above retained published cover is repair, not loss of coverage.

These two hunks change no source validity, queue capacity, memory limit, collar or
simulation role. Source parsing passed; build, contracts and another native run
belong to the build owner. The exact source receipt is
`reviews/forest-repair-20261021/physics-window/edit-proxy-priority-handoff.json`.

## Sparse ownership ABI audit

The dense owner window remains 129 by 129 chunks around the streaming center.
The ordinary radius clamps to 64; every eligible column outside that window must
therefore be an admitted horizon pin. Pins cap at 512; the sparse table has 1,024
slots and accepts at most 512 keys. Sorted insertion keeps byte equality stable.

Production sparse insertion checks failure and returns an enabled empty mask
instead of a partially populated mask. Dense insertion asserts success. Layout
assertions establish 10,320 bytes for `ResidentCoverage`, 10,640 for
`CameraUniform` (coverage offset 320), and 10,400 for `ShadowGenUniform` (offset
80). The shadow WGSL has the matching dense and sparse field order. The requested
wgpu 27 default uniform binding limit is 65,536 bytes; both uniforms fit.

## Regional fallback if finite supplemental coverage fails

Existing spatial draw batches classify triangles by centroid, and upload pages
are allocation units. Neither is an independent source ownership region. A true
regional fallback needs clipped water-only cores, local source certificates,
atomic shared-edge cohorts, and simultaneous parent exclusion plus replacement
publication in both main and shadow draws. It must share the total upload and
publication memory budgets. This is a larger change than supplemental source
coverage and should follow evidence that the smaller approach cannot close the
measured rejection loop. Never solve it by ignoring visible volume changes.
