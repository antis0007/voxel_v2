# Unified engine pass: evidence and continuation

Controlling specification: `docs/specs/AXIOM_Unified_Engine_Implementation_Prompt.txt`.
User reproduction description: freezing while exploring; the world falls behind
after many blocks are broken. No particular save or seed was supplied.

Stage 0 is still open. One independently reproduced CPU admission defect is
fixed, but it has not been established as the cause of the exploration stalls.
Stages 1–7 have not passed their gates. This is an evidence record, not another
implementation sequence.

## Actual paths inspected

| Contract | Native source | Observed boundary |
|---|---|---|
| Input and edits | `crates/app/src/main.rs`, `world/mining.rs`, `world.rs::set_many_with` | Player edit markers feed `ChunkStreamer::rebuild_dirty`; renderer publication remains separate from authoritative edits. |
| Meshing | `render/streaming.rs::run_mesh_job`, `render/streaming/mesh_budget.rs` | A shared planner and byte leases cover construction and retained CPU output. Impossible reservations previously waited indefinitely. |
| GPU upload | `render/streaming/{pending_gpu,gpu_upload,arena,residency,gpu_budget}.rs`, `render/upload_staging.rs` | Pending publication, terrain budget, arena backing and staging have distinct accounting. This inspection does not establish the specification's whole-process lifetime contract. |
| Authority and installation | `multiplayer/{authority,actions,replica,replication}.rs` | Host actions reuse survival rules; replica cells have bounded drain work. Replication explicitly uses the last **sent** image as its baseline; that is not evidence of recipient-confirmed installation. |
| Persistence and exact simulation | `world/storage/session.rs`, `simulation/cpu_region.rs` | Coherent isolated cellular checkpoints exist. They do not demonstrate complete rigid/fluid/contact-domain continuation. |
| Runtime material work | `simulation/world.rs` | Separate cellular, water, factory and rigid work budgets exist. Shared conserved coupling remains unverified. |
| Planet generation | `crates/worldgen/src/planet.rs` | Cube-sphere projection samples a continuous noise height source. This alone does not satisfy persisted planetary drainage/erosion or inhabited integration. |
| Frame reporting | `main.rs`, `perf/route.rs` | Successful `total_ms` measures presentation cadence, including time outside the current redraw. The previous route summary described this incorrectly as CPU work. |

The workspace has extensive pre-existing changes and concurrent writers. HEAD
changed during this pass. Source manifests describe files observed at capture
launch, not a verified compiler-input snapshot. Existing work was not reset,
stashed, cleaned or committed by this pass.

## Changes made here

- `mesh_budget.rs`: reject a reservation whose **total retained plus additional**
  bytes exceed the current cap; check again after every timed wait so budget
  contraction cannot leave an impossible request at the FIFO head. Existing
  leases remain charged until their owners drop them.
- `streaming.rs`: send non-cancelled admission failures to the existing failed-job
  collector. It clears request markers and throttles retries; silently dropping
  a rejected job would leave that coordinate marked as meshing indefinitely.
  This does not yet provide whole-job downstream GPU reservations or a supported
  minimal representation for every oversized edit.
- `main.rs` and `perf/route.rs`: append `redraw_ms` and `outside_redraw_ms` to the
  route CSV and correct its timing description. The latter includes event-loop
  gaps, earlier failed render attempts and recorder work, not solely GPU wait.
- `tools/Review-EngineEnvelope.ps1`: freeze the selected executable, use isolated
  saves, record CPU/GPU identifiers and hashes, asynchronously drain game logs,
  sample process memory, bound runtime and require fixture activation markers.
  Synthetic edits exercise `World::set_many`, not player/authority admission.

## Fault evidence

Two tests failed before the fix: oversized initial reservation and oversized
growth whose delta alone fits. Both waited until explicit cancellation. After
the fix those pass, as does contraction from 256 MiB to 160 MiB while growth is
waiting. The tests verify FIFO cleanup and retention/release of charged bytes.

Run the actual dependency-free production module without rebuilding unrelated
game systems:

```powershell
rustc --edition=2024 --test -A dead_code target/engine-envelope/budget-fault/harness.rs -o target/engine-envelope/budget-fault/after.exe
target/engine-envelope/budget-fault/after.exe --test-threads=1
```

The wrapper contains only a `#[path]` import of the production module. Three
tests passed; output is `target/engine-envelope/budget-fault/after.log`.

## Native diagnostic captures

Hardware: Intel Core i7-13700K, NVIDIA RTX 4090, Vulkan; Windows driver
`32.0.16.1742`. Hills, seed 74, 1280×720, view distance 8 chunks, LOD start 4,
3 LOD levels, physics on, clear weather, fresh isolated preferences/save.
Benchmark mode disables vsync and automatic quality, so these are not exact
normal-preset acceptance runs. Other builds were active throughout.

Frozen existing executable SHA-256:
`B826A0E247CAD8F312B870D47D9E0A30C73AE24C6278B2B4033825479BDA6DD4`.
It predates this pass and does not validate the patch or new timing columns.

| Capture directory under `target/engine-envelope` | Warm p50 / p95 / p99 | Worst warm frame | Peak sampled private bytes | Outcome |
|---|---|---|---|---|
| `existing-explore-20261029` | 10.117 / 26.913 / 46.744 ms | 13,152.957 ms | 2,895,069,184 | Exited normally; final pending streaming count 597 while moving. |
| `existing-edits-20261029` | 5.100 / 7.291 / 9.109 ms | 2,463.332 ms | 2,477,068,288 | **Invalid edit fixture:** static control only; no edit markers. Do not count as edit acceptance. |
| `existing-edit-fixture-20261029` | 9.823 / 18.625 / 33.502 ms | 10,261.079 ms | 2,812,186,624 | Corrected fixture: edit stages 1–12 recorded; 404 pending streaming requests at exit. |

The largest exploration frame has only 66.055 ms in its horizon timer, 10.985 ms
in streaming and 5.000 ms in rendering. The old capture cannot separate work
outside these timers from presentation/event-loop gaps. Logs also contain surface
timeout retries. Neither observation establishes that a GPU memory leak or the
CPU reservation defect caused the stall. Process private bytes are not VRAM.

Every capture preserves `run.json`, `source-hashes.csv`, `tree-status.txt`,
`process-memory.csv`, stdout/stderr, warmed `frames.csv`, and full route CSV/JSON.
The first edit attempt exposed a missing camera activation flag in the launcher;
it was corrected and a required edit marker was added before rerunning.

Build verification: the first release attempt encountered a concurrently changed
multiplayer signature. A subsequent optimized release compile failed at archive
renaming inside the filesystem sandbox. Debug checks reported an unresolved
`engine_core::modifiers` import even though its declaration exists in current
source; the cause of that debug-build discrepancy was not established.
The outside-sandbox `cargo build --offline --profile release-fast -p
minecraft_style_voxel_prototype --message-format=short` succeeded (6m 22s), with
existing warnings. This is the repository's optimized iteration profile, not
the thin-LTO release gate.

Rebuilt candidate SHA-256:
`14D85CB7D7BA88AD85B4CB7E6DCED96C5FBBA87C0FE525FD482FB48E25854A70`.
The `patched-explore-20261029` run completed with p50/p95/p99
9.601/14.984/24.268 ms, worst warmed frame 179.641 ms, peak sampled private
bytes 3,028,000,768 and final pending count 638 while still moving. The new CSV
columns are present. One 179.641 ms frame comprised 17.856 ms of redraw work
and 161.785 ms outside that redraw. A separate 84.784 ms frame spent 76.623 ms
in horizon work. A 314.098 ms pre-measurement frame spent 268.111 ms rendering.
These are different costs; a single CPU or GPU optimization is not established
as the remedy for all of them.

`patched-edit-fixture-20261029` also completed and recorded edit stages through
11. Warm p50/p95/p99 were 7.879/14.930/18.068 ms, but the worst warmed frame
was **7,140.607 ms**. Its redraw was 103.967 ms and time outside the redraw was
**7,036.641 ms**. The freeze therefore remains reproducible after the budget
fix. Peak sampled private bytes were 2,860,838,912; the final streaming queue
had 508 pending requests. At an earlier stationary point the source queue
reached zero, but this is not a declared full recovery deadline across all
simulation/rendering work. An optional `-TraceStages` launcher switch exposes
the existing native event-stage probes for locating the remaining gap.

The targeted `patched-edit-trace-20261029` run reproduced a 3,603.437 ms worst
warmed frame (p99 21.087 ms). Consecutive stage probes show a 3.564 s gap from
`render-done` at t=71.476 to `event-dispatch` at t=75.040, followed immediately
by `input`, `event-dispatch`, and `frame-start`. This bounds the gap to the
post-render/event-pump interval, not the next mesh build or gamepad polling
interval. It does not distinguish post-render cleanup, pre-dispatch window/UI
handling, OS scheduling or other event-loop work. The trace also observed a
615 ms interval between two event-dispatch probes. Further profiling belongs
there; deleting geometry or raising a mesh cap is not justified by this trace.
This diagnostic run used the same frozen rebuilt executable, verified edit
markers, exited successfully and recorded build contention.

This is native execution evidence for the rebuilt candidate, **not a controlled
before/after attribution**: the checkout/content changed concurrently and build
contention was observed in both captures. The specification's relative targets
are therefore still unverified.

## Remaining gate

Repeat exploration with matched content and no competing builds; exercise
player/authority burst edits rather than only synthetic setters. Diagnose the
dominant measured gap before selecting the next engine
optimization. Demonstrate drain time after movement/editing stops; the moving
route's terminal queue length alone is not a failed drain test.

No 30% p99 improvement, 50% warm-load improvement, stable prolonged memory,
multiplayer correctness, device recovery, integrated material scene, planetary
district or low-memory-hardware acceptance is claimed.

## Continuation: recent documents and Stage 1 cleanup

The 29 October continuation inventoried 61 recently modified text/JSON documents
under `docs/`; hashes and dates are in
`target/engine-envelope/doc-review-20261029/recent-documents.csv`.
Reviewed the consolidated package's execution/foundation/delivery chapters,
R1's authority and engineering corrections, decision 0039 and its reconciliation,
current master/tracking notes, and recent implementation/evidence notes for maps,
Manyfold, generators, mining/drainage, rivers, UI, multiplayer and tools.
Large older source briefs and the rough conversation are provenance; this was
not a fresh line-by-line re-audit of every historical proposal or an art review.

The package's P0/P1 gates and R1 T00/T01 both put correctness before expansion.
The actual source-cache eviction repair is present; the older reconciliation's
"HEAD still has the bug" is not a current-source conclusion. Source inspection
found save 46 and generator 82, ahead of several status documents. Map observations,
counterweight, tube traversal, seals and portal work have other active writers;
this increment takes no save, generator, gameplay-authority or protocol change.
Later stages remain targets, not completed work or permission to replace those
systems. Existing body-authority/transport decisions remain unchanged.

Implemented here:

- `gameplay_trace.rs` and `main.rs`: callback entry/exit scopes include early
  returns and local drops; UI, recorder and performance-summary boundaries now
  distinguish stalls inside a callback from gaps between callbacks.
- `streaming/arena.rs`: drain at most 128 returned ranges per pass, stopping
  after a 250 microsecond soft budget once at least one range has progressed.
  Retire at most one empty trailing page (two GPU buffers). Unprocessed ranges
  remain allocated; stable page indices and the five-second reuse delay remain.
  A driver operation itself is not preemptible, so this is a work bound, not a
  guaranteed wall-clock limit. Whole-cache destruction is outside this change.
  The existing three-lease GPU check now permits three maintenance passes;
  it was adjusted for incremental cleanup but was not run.
- `streaming/retirement.rs`, `streaming.rs`, `main.rs`: hidden/zero-size normal
  windows service GPU callbacks with nonwaiting `Poll` and arena retirement every
  100 ms. No surface acquisition, gameplay tick or candidate publication occurs
  here. Pending products still go through normal revision checks on redraw.
  This does not establish complete device-loss recovery or all pending cleanup.
- `perf/route.rs`: CSV output uses a worker and a 256-row queue. The frame thread
  uses `try_send`; overload drops diagnostic rows and records
  `csv_dropped_frames`. Aggregate timings still count every attempted render.
  Final flush/join runs after measurement; the native launcher retains its overall
  deadline. Optional verbose stage logging is still synchronous and can perturb
  a traced run.
- `tools/Summarize-EngineEnvelope.ps1`: reproducible warmed cadence percentiles,
  worst frames and the worst 32 trace intervals, with callback classification,
  CSV completeness and build-contention flags.

The pinned wgpu 27.0.1 source and Context7's upstream documentation were checked:
`Device::poll(PollType::Poll)` services completion callbacks without waiting for
a requested submission. Callback/driver CPU work can still take time; no hard
latency guarantee follows from the word "Poll".

### Native continuation evidence

`event-scope-edits-20261029` completed the same 90-second synthetic edit fixture.
Executable SHA-256:
`368C5A496E6CC92CE588E7A7E5A64916D4EED242146578ED57CA1FBB5C6A5DF5`.
This build contains the expanded event scopes, before the bounded-retirement
and asynchronous-recorder changes. Its optimized build passed (83 warnings,
13m 01s including the shared artifact-lock wait).

7930 warmed successful frames: p50 **8.3756 ms**, p95 **18.4247 ms**, p99
**21.0437 ms**, maximum **5006.4136 ms**; 12 frames exceeded 50 ms. The trace
isolates **4995 ms between upload and upload-done**. It also found **11921 ms
between render-done and timing-done on the final frame**; with no later rendered
frame, that recorder delay is absent from the frame maximum. There was also a
7828 ms gap between event callbacks before measurement. These are distinct
intervals, not evidence of one common cause.

The recorder interval motivated moving CSV I/O off the render thread. Upload
sub-stages now separate residency, polling, retirement, ready-result collection,
allocation, writes and submission for the next run. Native sources/content and
competing builds changed, so none of these numbers supports a before/after gain.
REN-01 is recorded as FAIL/open in the consolidated acceptance catalog; lifecycle
and mixed-workload cases remain NOT_RUN rather than being inferred from source.

No tests or hosted CI were added or run in this continuation. Formatting and
scoped whitespace checks passed. A subsequent full build first stopped on the
concurrent `hell_gate.rs` reference to nonexistent `BlockId::Obsidian`; that
reference changed independently before the final build attempt. Final build and
native results are recorded below when available.

The final `cargo build --offline --profile release-fast
-p minecraft_style_voxel_prototype --message-format=short` passed: 12m 16s
including shared-lock waiting, 87 existing warnings, 22 composed WGSL programs
validated. Receipts: `target/engine-envelope/continuation-build.log` and
`continuation-build.exit`. The session-report command initially hit the Windows
Store Python alias; the bundled Python successfully ran the same script.
Its historical benchmark entries are not measurements from this continuation.

### Final native run: bounded cleanup and asynchronous recording

`bounded-cleanup-edits-20261029` completed the 90-second fixture without verbose
stage tracing. Executable SHA-256:
`ED34BD9F1C2D9A81FE6263660ADA2307D0EB8F3C682D8AA6136E0385A26D9991`;
launch HEAD `90832e92ea5c3536dd18001e2c9e128e73bfd4d3` with shared changes.

The complete `frames.csv` contains **11714 warmed samples**: p50 **6.1725 ms**,
p95 **12.7219 ms**, p99 **17.5236 ms**, maximum **2123.2048 ms**, and **22 frames
over 50 ms**. Peak sampled private memory was 2,804,383,744 bytes. Competing
builds were observed. These are current-run measurements, not attributable
improvements over the earlier builds.

The bounded writer reported **5398 dropped route rows**, no I/O failure, and a
successful final flush. The diagnostic CSV is incomplete; only 7872 warmed rows
remain there. The summarizer now uses the separate complete benchmark sample
export for cadence percentiles and labels retained route spans separately. The
aggregate recorder also retained all 16247 attempts (including startup/warmup).
This verifies overload reporting in the native run; it does not certify disk
performance or a complete per-frame route trace.

The retained worst frame has 28.7222 ms of redraw work and 2094.4827 ms outside
that redraw. Another frame has 1439.6499 ms inside its redraw. Large pauses
therefore remain without verbose tracing. GPU-upload, event-loop, scheduling and
other synchronous diagnostic costs are not yet fully isolated. REN-01 stays
failing/open; neither quieter diagnostics nor lower reported p99 closes it.

The normal-play minimized maintenance branch was compiled but not exercised by
this benchmark, which deliberately continues rendering when occluded. Its
native minimize/restore, multi-page retirement and device-fault acceptance remain
open. No later implementation stage is marked complete. The next dependency is
a matched, uncontended capture and diagnosis of the remaining in/out-of-redraw
pauses, followed by actual player-edit backlog drain and lifecycle review.
