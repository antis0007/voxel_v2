# Spatial world simulation scheduler

Implemented 2026-10-18. Source and compilation evidence only; no tests, CI,
benchmark, game launch, remote-factory session or save round trip was run for
this change.

## What changed

The old local fluid queue discarded cells beyond 24 metres and discovered
natural physics through render residency and synchronous height sampling.
Water and falling-material rules could call disk loading or full chunk
generation on the event thread. World-facing pumps, drains and drills had a
separate 64-metre cutoff.

`simulation::WorldSimulation` now owns scheduling and asynchronous source
residency without a graphics-device dependency. Cellular rules run over
prepared authoritative chunks. Activity survives movement, render eviction,
source-loading delays and checkpoint reloads. It is activated by edits,
neighbour changes, terrain-worker discovery of potentially unstable material,
and migration/recovery scans. Untouched, stable wilderness sleeps; the engine
does not continuously enumerate an infinite procedural world.

Each active chunk has an exact cell bit mask. Wakeups deduplicate by bit;
inactive chunks retire their masks. A rotating bit cursor prevents repeatedly
awakened low-index cells from starving the rest. Chunk service combines a
nearby lane with a distance-independent fair lane. The first quantum alternates
between ready distant work and new admissions; a busy nearby waterfall cannot
consume the entire step before distant work gets considered. Source-completion
waiters promptly return blocked chunks to service, rather than relying on a
long queue rotation that could evict their data before reuse.

World-facing machines use the same source pool, with a separate rotating
cursor and retained time credit. Their distance cutoff is removed. Pending
reads do not consume water, inventory or excavation progress. Existing player
collision and assembly guards remain. Machine networks retain their existing
bounded-node model; the cellular-physics toggle pauses water/gravity, while
powered machines continue their normal independent operation.

## Work and memory policy

| Owner | Bound |
| --- | --- |
| Cellular activity | 4,096 chunk masks, 4 KiB each, plus bounded queues/metadata |
| Deferred activity | 8,192 compact region masks (512 chunk flags each), then an emergency bounded-volume sweep descriptor |
| Checkpoint activity capture | Pins immutable masks; first subsequent mutation copies a mask, at most another 16 MiB while the single save is active |
| Simulation sources | 32 MiB for cached sources plus outstanding job reservations |
| Source jobs | Eight outstanding, including queued, running and completed results; 4 MiB reserved per job |
| Source workers | One or two, with epoch cancellation and restart backoff |
| Machine source admission | Leaves two outstanding-job slots for cellular fronts |
| Cellular service | At most 1,024 cells / 128 chunk visits per update, with an approximately 3 ms total scheduler cutoff checked between operations |
| World-facing machine service | Approximately 1 ms within that scheduler update |
| Source integration | At most eight results / approximately 0.5 ms between integrations |
| Recovery enumeration | One on-demand worker and a two-coordinate result channel; one chunk admitted per update |

These are owned-resource and admission limits. They exclude OS thread stacks,
allocator bookkeeping, the shared procedural caches, edited-world residency,
renderer sources, mesh workers and graphics resources. Existing edited-cache,
CPU voxel, mesh and GPU budgets remain separate. Source reservations stay
charged through result delivery and cancellation, including world resets.

Cell rules commit synchronously after both endpoints and their neighbourhood
are available. Missing saved chunks are never interpreted as air or replaced
with procedural data. Water transfers conserve their eight-unit volume and
falling sediment returns displaced water to the vacated cell. Preparation
checks source versions and both assembly/bounds guards before mutation.

Background simulation stops admitting mutations when edited-world residency
needs writeback. Dirty data and activity remain intact on I/O failure. Activity
capacity also gates propagation before changes occur. External wakeups at full
capacity coalesce into compact deferred region masks, including procedural
chunks that have never been stored. At extreme metadata capacity they coarsen
to an emergency volume sweep; that may revisit many sleeping chunks but keeps
memory bounded. Deferred chunks promote as detailed work drains. Source workers
filter sleeping solids and balanced water before handing broad rescans back to
the cell scheduler. Migration recovery flushes unrepresented edits before
enumerating stored chunks. This is backpressure, not silent queue truncation.

The time cutoffs are soft checks between operations. A single chunk install,
copy-on-write allocation or mutation is not preemptible. Under sustained load,
world simulation slows rather than trying to replay an unbounded backlog.
There is no simulation while the application is closed or the world is paused.

## Checkpoints and compatibility

VXWSAV10 appends cell masks, traversal cursors, deferred regions, emergency sweep,
simulation tick and the recovery
flag to the existing session footer. The background save worker serializes the
pinned masks together with the captured voxel/water state. Source jobs never
mutate the world, so an unfinished read cannot disappear between capture and
commit. V1-V9 remain readable; V9 activity masks retain their original layout.
V1-V8 worlds request a background rescan of
stored chunks instead of synchronously loading their complete edited history.
Region-record format and reader-generation leases are unchanged.

## Streaming changes

- Partial water volumes are partitioned by chunk. Paging, eviction, writeback
  and checkpoint capture visit selected chunks instead of repeatedly filtering
  a global liquid-cell map.
- Render snapshots contain nearby edited chunks and their partial volumes.
  Remote edits do not invalidate an unchanged nearby snapshot. Configuration,
  view movement, relevant edits and relevant paging/publication still refresh it.
  Region metadata refreshes independently so a stationary renderer does not
  indefinitely pin obsolete distant segment generations.
- Region lookup uses an XZ column directory. Nearby discovery performs keyed
  spatial lookups instead of scanning every stored region.
- Region-index copy-on-write roots are split into 128 partitions. Publishing
  one record copies its affected directory bucket and region group, rather
  than cloning the complete region directory held by readers.
- Dirty material edits are indexed by the exact source halos they intersect;
  each dirty render chunk no longer scans every remote edit in the batch.
- Ordinary distant writeback uses a 750 ms dirty-age/coalescing window, also
  renewed when a write completes behind newer edits. Memory pressure, explicit
  checkpoint requests and recovery can request earlier writeback. Distance
  alone no longer forces hot remote chunks into repeated syncs.
- Cell batches and transfer admission use stack buffers. Visited sets retain
  their storage across steps instead of allocating in each hot-loop iteration.

## GPU upload failure handling

An existing long-flight log reported `Buffer::get_mapped_range` panicking on an
invalid `chunk vb`. This is diagnostic evidence from a separate review, not a
runtime session launched for this change. The exact allocation/driver cause
was not recorded in that log.

Chunk uploads now validate device buffer-size limits, create unmapped GPU-local
destinations and use fallible queue staging. Failed allocations/validation are
captured in scoped errors; scopes are popped immediately and their futures
polled without blocking the event thread. Failed staging retains the CPU mesh
under its existing heap lease and backs off before retry. Unsupported oversized
meshes defer without evicting valid terrain merely to attempt an invalid
allocation. The UI exposes the upload error. This removes mapping of chunk
destination buffers; it does not establish that device loss or every possible
GPU allocation failure has been eliminated. The long-flight case has not been
replayed for this change.

## Remaining scaling work

This is a scheduler for active cellular water/gravity and existing machines,
not a complete Noita-scale material engine. Fluid pressure, temperature,
combustion, broader reactions, persistent dynamic bodies, parallel conflict
resolution and simulation detail levels remain open. Factory graph operations
retain their existing node cap and are not fully incremental. Distant activity
is budgeted, not guaranteed to run at the same rate as nearby activity under
overload. Actual throughput, conservation during runtime pressure, long-flight
behaviour and save/recovery acceptance remain unmeasured.

Local verification: `cargo check --offline --target-dir target` and
`cargo build --release --offline --target-dir target`. Diagnostics are in
`target/world-simulation-check.log` and `target/world-simulation-final-release.log`.

Final optimized candidate: `target/world-simulation-review/voxel-workshop-v10.exe`.
Use the repository as the working directory. SHA256: `1DBB2D9BE4FAA3C27CE3ABCCA275FE0E4A543AE48DA2C9116AABE8FC3ADFA434`.

The newer save-path and streaming follow-up is documented in
[SAVE_AND_STREAMING_FIX.md](SAVE_AND_STREAMING_FIX.md). Its candidate supersedes
the launch-folder-dependent save paths of the V10 candidate above.
