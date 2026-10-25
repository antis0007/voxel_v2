# 0018 Shared fixed steps and an isolated CPU cellular region

Date: 2026-10-24. Status: implemented increment; compilation evidence in the execution ledger.

The app adds a direct workspace dependency on `engine_sim` to call the existing
`BrickStore` and `step` reference. No external package, allocator, kernel, cell
layout, save format, generator gate or movement binding is changed.

## Clock and input

`TickId(0)` means initial session state. Each completed player step increments
the ID once and lasts 1/120 second. The existing frame accumulator admits at most
16 steps of wall time per frame, discarding excess wall time for responsiveness;
it does not skip tick IDs or drop accepted commands. Pause/loading clears the
fractional accumulator. Resume starts from the same completed ID. IDs restart
when a world/session starts; they are not yet a persisted replay timeline.

Both drivers execute `simulation::fixed::gameplay`, then their interaction ingress,
then `fixed::finish` for **each** step: expedition, factory timers, player,
adventure, tools, simulation (including factory extraction), day. The window's
review/input callbacks are explicit seams; the headless driver settles legacy
source jobs. A zero-step frame executes none of these writers and retains input
edges and the last tool outline. Held movement/cast input is sampled every step;
cast-pressed and spell-cycle edges are consumed once on the first executed step.
Short mouse clicks are latched independently of held place/break state, so a
press/release on zero-step frames survives until interaction ingress. Each action
retains one pending edge; repeated held use follows its existing repeat interval.
Pick/interact are likewise consumed in the window's first tool ingress. Opening
UI/focus reset still cancels gameplay input. UI selection/scroll and camera look
remain presentation/frame actions. Tool repetition advances by one fixed dt.
Legacy headless `Place`/`PlaceAt` retain their immediate fixture-edit semantics
between steps; they are not a replay of window tool ingress. With the CPU option,
those same operations become acknowledged commands for the next eligible CA tick.

Legacy budgets remain wall-clock based, shared across the frame's fixed steps
(2.5 ms, or the existing 6 ms water-backlog allowance). The deadline starts at
the first simulation pass, and child water passes cannot extend it;
the legacy edit queue also retains its time-based yield. Therefore this change
establishes common stage execution, **not** deterministic legacy scheduling or
headless/window world-hash equivalence. This debt is not inherited by the new
CPU region, whose complete bounded work is independent of wall time and camera.

## Development integration and ownership

`--headless=<path> --ca-cpu-region=x,y,z` imports one 8³ interior at an 8-cell
aligned origin into the actual scenario runner. Its player collision queries
read the tick-labelled mirror through `World::get`. The region is deliberately
headless-only: ordinary window play/saves retain legacy authority. Within this
isolated session, factory, adventure, legacy cellular/rigid/thermal and source
residency writers are suspended. Scenario edits use the explicit command ingress;
outside/halo edits fail before acceptance. There is no live mid-world cutover.

A one-cell, immutable bedrock halo encloses the region, with no boundary flux.
It is a development boundary, not sampled terrain or a promise of cross-region
coupling. The interior imports only ambient air/water/sand/stone/bedrock;
unsupported cells reject activation before attaching a mirror. Import scans
exactly 1,000 cells. All 27 intersecting bricks are reserved first (108 KiB base
cells plus store metadata). Capacity failure rejects activation, leaving the
source world intact. Existing dense awake indexing and canonical order stay intact.

Each snapshot retains 512 exact eight-byte cells (4 KiB) plus metadata, with at
most three retained snapshots after a tick and one transient result during
publication. There are no optional planes, staging buffers, meshes or GPU targets
for this headless path. State hashing also scans the fixed reserved pool. These
are capacity bounds, not measured throughput/memory acceptance.

The queue holds at most 128 commands. Admission returns sequence, submitted
player tick and earliest CA tick. Full/outside/unsupported requests return an
error without acceptance. Each CA tick consumes up to 32 commands, FIFO, before
the kernel. Overflow past that quota remains queued, preserving order even for
repeated edits to one cell. No-op writes still acknowledge the sequence. Results
report the last applied sequence separately from the last visible sequence;
an accepted receipt does not mean applied or visible. The runner fails visibly
on an admission error, and never reports that rejected edit as accepted.

## 60 Hz divider and publication

The 60 Hz proposal is adopted **only for this opt-in backend**: player steps 1
and 2 read S[0]; after step 2, command ingestion and CA tick 1 produce S[1].
After step 4, CA tick 2 produces S[2], still publishing S[0]. After step 6,
CA tick 3 produces S[3] and publishes S[1], consumed by subsequent player steps.
In general, completed CA n publishes S[max(0,n-2)], with the snapshot's real
tick retained throughout startup. Publication swaps an immutable Arc; exact
cell state is never converted through `engine_voxel::bricks::read_chunk`, which
rounds water to eighths. Legacy water queries expose a rounded **view** only;
`Water64` and `InspectCell` expose the exact development state.

`prepare_step` checks the result deadline before any gameplay writer, and the
region checks it again before advancing the backend or consuming the queue.
Missing due results fail/stall authority; they cannot be relabelled or
replaced by an older result. The current backend is synchronous, so delayed GPU
results cannot yet occur. An asynchronous integration must preflight that deadline
before the next gameplay step, retain the last published render view while
stalled, and resume the same tick/input. No prediction overlay is implemented.
The pinned u32 kernel tick is checked for exhaustion instead of silently wrapping.

## Remaining cutover gates

This is a bounded application path, not the Flooded Observatory or a migration
of factory/spell/save/renderer writers. CPU sessions do not load or save ordinary
world files; their seed and command document are the development input. Pending
commands are reported on exit, not implicitly drained by invented ticks. Exact
checkpoint/resume, world revisions/meshes, shared boundary exchange, migrated
writers and the GPU-resident adapter are next gates. GPU work must reuse
`sim_gpu::resident` and `resident_readback`; legacy rollback stays available.

No tests, scenarios under the automatically tested assets directory, benchmarks,
CI or visual changes are added or executed for this increment.
