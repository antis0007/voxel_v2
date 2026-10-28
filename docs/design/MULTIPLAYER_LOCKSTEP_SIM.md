# Multiplayer world simulation in deterministic lockstep

Date: 2026-10-27. Status: foundation landed (netcode scheduler, sim digest, proof test); live play not wired.

Owner goal: friends share a world over the internet and every computer runs its own physics. Today the
host runs the legacy cellular pass under a wall-clock budget and streams the results (snapshots, cell
deltas). This document splits the work so world matter runs in deterministic lockstep and only inputs
cross the network.

## Authority split

| State | Model | Why |
|---|---|---|
| World matter: `engine_sim` bricks (material, volume, temperature, pressure, charge) | Deterministic lockstep on every peer | Integer-only, positional-hash randomness, canonical order (ADR 0008). Bandwidth no longer scales with how much water moves. |
| Players, enemies, projectiles, rigid bodies, items, inventories | Owner/host authoritative, as today (`engine_netcode` snapshots, movement validator) | Float physics (ADR 0011) and cheat surface. |
| Anything non-deterministic that writes matter (rigid body settling into voxels, legacy machines, wand blasts computed in floats) | The host turns its result into a stamped matter input | Matter stays a pure function of seed and the input log. |

The simulated domain (which chunks are awake and stepped) must be identical on every peer. It cannot be
"around the local player". Domain changes (activate or retire a chunk) are lockstep inputs the host stamps,
and domain edges use the sealed boundaries of ADR 0018.

## Tick and input model

The lockstep tick is the CA tick (60 Hz; two player steps per CA tick, ADR 0018).

`engine_netcode::lockstep` (`crates/netcode/src/lockstep.rs`, `lockstep/host.rs`, `lockstep/client.rs`):

- Every input that changes matter (an edit, a repair, a domain change, a host-computed blast) goes to the
  host. `LockstepHost::receive` accepts a client's inputs in its own sequence order and drops duplicates,
  stale inputs and gaps; `submit` takes the host's own inputs.
- The host stamps each input with tick `next_tick + input_delay` (default 4 ticks, 67 ms) and a
  session-wide `order`. `advance()` seals that tick into a `Frame { tick, inputs }` and returns the frame the
  host steps now. Every tick gets a frame, empty or not, so "complete" is well defined.
- Frames travel redundantly: each `HostMsg` repeats every frame from the client's acknowledged tick, up to
  32 frames. Client inputs go the same way (`ClientMsg` repeats every unacknowledged input). Loss,
  duplication and reordering cost latency, never correctness, and neither side needs a retransmit timer.
- `LockstepClient::release()` returns tick T's frame only when the client holds it. The caller applies its
  inputs in frame order, then calls `step`. A client paces itself to the host's tick (`clock::TickClock`)
  and catches up at most a few ticks per frame.
- Bounds: the host holds at most `max_pending_inputs` unsealed inputs; a frame takes at most
  `inputs_per_frame` inputs and the rest roll over to the next tick. The host retains frames only back to the
  slowest peer's acknowledgement. A peer that trails `resync_lag` ticks (300 = 5 s) is cut off: the host stops
  feeding it frames and flags `resync`, and `ClientStatus::NeedsResync` tells the client to load a
  checkpoint. Host memory is at most `resync_lag + input_delay` frames.
- Edit prediction stays a render overlay. The client draws its predicted edit (`edits::EditPredictor`) over
  the world but never writes it into its simulation, which would desync it. The overlay clears when the
  stamped edit's frame applies.
- The host validates every input (reach, inventory, flood limits) before stamping it. Clients apply frames
  without judging them. Item accounting stays host authoritative and replicates as it does today.

## Desync detection

`engine_sim::RegionDigest` (`crates/sim/src/digest.rs`) has three levels: a 64-bit hash per stored brick,
keyed by its chunk and brick index; a wrapping sum per chunk; and a wrapping sum over the region. Sums
commute, so map order and pool layout cannot change a value. An all-empty brick contributes zero, matching
`BrickStore::state_hash`.

Updates are incremental. `step_tracked` observes the store's dirty masks before and after the kernel runs,
which covers the edits applied for the tick and every brick the tick wrote. `refresh` rehashes only those
bricks, so its cost is bounded by the bricks that changed and does not grow with the region. Whole-chunk
replacement calls `mark_chunk`.

Exchange: every `K` ticks (the test uses 30; 60 = 1 s in play) each client sends `(tick, digest)` to the
host. `lockstep::desync::DesyncMonitor` compares the report with the host's own digest for that tick. A
report can arrive before the host reaches that tick, because a client can run up to `input_delay` ticks
ahead; the monitor holds such reports, a bounded number of them. A report older than the monitor's history
expires.

Measured cost, `digest_tick_196k_cells_all_awake` (64 x 48 x 64 cells = 384 bricks, all changed, observe +
refresh): **0.14 ms** per tick on a quiet run (0.7 ns per cell); the recorded baseline, a median of 3 runs taken
while other builds loaded the machine, is 0.25 ms. Either is under 1.5% of a 16.7 ms tick. The bench machine is the desktop that measures the 262k-cell CA tick at 16.3 ms. A
quiet region costs one visit per awake chunk.

The digest only sees changes made through the store's API. It catches divergent simulation, which is what
lockstep risks, but not memory corruption. A background scrub that calls `mark_chunk` on a few chunks per
tick, rotating through the region, would cover that.

## Repair

The host repairs a client in three steps:

1. **Narrow.** On a mismatch at checkpoint C, the host asks that client for its chunk sums at C. Both sides
   keep the last few checkpoints' `chunk_sums()`. Comparing them yields the differing chunks; brick hashes
   can narrow a chunk further.
2. **Schedule.** The host submits a `Repair { peer, chunks }` input for the differing chunks plus their face
   neighbours, because a desync spreads while the repair is in flight. The input is sealed into frame R like
   any other.
3. **Ship.** When the host steps R, it copies those chunks *before* applying R's inputs. That copy is the
   state at the start of R. The host sends it on the reliable stream: whole bricks, the cell-delta path's
   stale-brick resend (`cells::write_brick` form, extended to 8-byte cells). The target client stalls at R
   until the data arrives, replaces the chunks (`fill_chunk` + `mark_chunk`), then applies R's inputs. Other
   peers ignore the repair.

The stall lasts `input_delay` ticks plus one-way latency. If a later checkpoint still differs, the host
repeats the repair once, then falls back to a full checkpoint resync.

Extra waking is harmless: a repaired brick wakes on the target and may sleep on the host. Stepping a settled
brick changes nothing, which `scenario_tests::waking_only_what_changed_misses_nothing` proves, so dirty masks
are not world state.

## Late join and resync

The host takes a checkpoint at the start of `next_tick()`, calls `add_peer(id, that_tick)` (or `resynced`)
and sends the checkpoint by bulk transfer. Retained frames are the input log since that tick. The joiner
loads the checkpoint, creates `LockstepClient::new(cfg, id, that_tick)` and catches up by stepping frames
faster than real time. Only the simulated domain's bricks travel. Chunks outside it regenerate from the seed
plus saved edits, as today. If the checkpoint takes longer than `resync_lag` to deliver, the join needs a
larger `resync_lag` for that peer. The frames are small (below), so 30 s of retention costs kilobytes.

## Bandwidth

Proof test: 3 peers, ~0.5 inputs per tick across the session (2,655 in 5,600 ticks, 28 per second), 5% loss, 2% duplication, 40 ms
latency with 25 ms jitter. Measured lockstep traffic is **4.3 kB/s per client, both directions together**,
including 32-frame redundancy. That cost depends on the input rate, not on how much matter moves. A dam
break that changes 10k cells per tick costs the same as a still world.

Today's cell path is different. The host sends every changed cell inside each client's interest set, within
a per-flush cell budget. Bricks that change faster than the budget turn stale and are resent whole (4 KiB of
cells per brick before palette compression). Under heavy flow that is bounded by the budget, so clients see
stale water.

Lockstep pays instead in determinism discipline and in repair traffic, which is rare.

## Nondeterminism to remove before live play

The current working tree was audited on 2026-10-27. Ranked by how surely each breaks lockstep.

| # | Source | Where | Severity |
|---|---|---|---|
| 1 | Host steps the world with a wall-clock budget each fixed step: `fixed::finish(..., WorldSimulation::live_budget(world), false, true)` | `crates/app/src/multiplayer.rs:1300-1301`; window: `main.rs:4276-4282` | blocker |
| 2 | `Budget::wall` / `child` / `has_time` read `Instant::now()`; every consumer cuts work where the deadline lands | `crates/core/src/time.rs:33-37`; consumers `simulation/world.rs:344, 352-354, 376-378, 415, 466`, `rigid.rs:98, 171`, `environment.rs:44` | blocker |
| 3 | Water tick counter advances only when the wall-gated loop runs; it seeds direction choice and thermal phase | `simulation/world.rs:378-381`; read at `cellular.rs:300, 385`, `thermal.rs:38, 149, 269` | blocker |
| 4 | Near-list rebuilt every 500 ms of wall time, reordering chunk processing even unbudgeted | `simulation/world.rs:385, 403` | blocker |
| 5 | Recovery rescan on a thread, retried on an `Instant` | `simulation/world.rs:250-296` | blocker |
| 6 | Source chunks load on worker threads; readiness depends on thread timing (`available_parallelism`, `try_recv`), and a cell next to an unready neighbour is postponed | `simulation/residency.rs:95, 174` | blocker |
| 7 | Activity seeded from each machine's own streaming radius around its local player | `simulation/world.rs:131` (`seed`); anchor `multiplayer.rs:1299` | blocker (domain must be a lockstep input) |
| 8 | Edit's CA tick gets +1 when the GPU is staging, so it depends on readback timing | `simulation/cpu_region.rs:130-134` | blocker for the CPU-region path |
| 9 | Climate ambient uses platform `cos`/`exp` (f32) feeding melt/boil thresholds | `simulation/climate.rs:78-79` | medium |
| 10 | Rigid bodies: f32 velocity, `Quat::from_scaled_axis` (sin/cos), `round()` decides occupied voxels | `simulation/rigid.rs:180-188` | medium (stays host authoritative; its voxel writes become inputs) |
| 11 | engine_sim pool-full stall depends on `free_bricks()`, so on capacity and on `trim` history | `crates/sim/src/kernel.rs:142` | medium (every peer must use equal capacity and identical trim calls, or reserve as `cpu_region/backend.rs:58-72` does) |
| 12 | Fine pass truncated by a wall budget and a 500 µs `WALL_SLICE` | `crates/sim/src/matter/step.rs:197`, `matter_jobs.rs:62, 112` | medium (not reached from the app yet) |
| 13 | GPU brick allocation is a CAS race when the pool runs out | `crates/sim_gpu/src/alloc.wgsl:12, 39` | medium (a GPU peer must treat overflow as a desync) |
| 14 | FastMap iteration picks work (`deferred.iter().next()`); deterministic only after identical insert histories | `crates/app/src/world/activity.rs:122` | minor |
| 15 | Plain f32 in water velocity and heat flow (bit-stable on one instruction set without FMA, fragile across targets) | `simulation/cellular.rs:103-109`, `thermal.rs:97-101` | minor |

Clean: the engine_sim kernel (integer only, seeded dice, canonical dispatch order `store/activity.rs:53`,
sorted `state_hash`). The digest folds its maps by commutative sums.

Consequence: the legacy passes (1-7, 9, 15) are not repairable piecemeal into lockstep. The plan is to
replace them with engine_sim for the shared domain, which the CPU-region work started.

## Changes proposed to existing APIs (not made)

- `engine_core::Budget`: a count budget (`Budget::work(n)`, in blocks or bricks) so a bounded pass stays
  deterministic. A wall budget can only decide *when* the next tick starts, never how much of a tick runs.
- `engine_sim::BrickStore`: a public `brick_cells(chunk, brick) -> Option<&[Cell]>` for the repair encoder,
  and a `scrub` hook for the digest.
- `engine_sim::MatterStore`: fold fine sub-bricks into the digest (`MatterStore::state_hash` already folds
  them for the full hash).
- `engine_netcode::cells`: an 8-byte-cell form of `write_brick`/`read_brick` for repairs; `reliable`'s
  1,000-byte message cap means repair data goes over bulk transfer.

## Next: wiring into live play

Build on the in-flight CPU region (`crates/app/src/simulation/cpu_region.rs` and `cpu_region/`, last touched
2026-10-26; its owner is idle). It already has most of what lockstep needs:

- `Region::advance(tick)` (`cpu_region.rs:175`) applies the commands due for a tick, then
  `engine_sim::step(store, rules, seed, tick)` with no wall budget.
- `Region::import` (`:88`) builds a reserved, fixed-capacity store, so pool stalls cannot differ.
- `Snapshot.hash` is a state hash.
- `encode_checkpoint`/`decode_checkpoint` (`cpu_region/checkpoint.rs:148, 189`) serialise the region for a
  join.

Missing pieces, in order:

1. **Stamped ingress.** Add `Region::apply_frame(&Frame<RegionOp>)`, where each input carries
   `(peer, sequence, ca_tick)` from the host. Today `submit` (`cpu_region.rs:109`) computes
   `earliest_ca_tick` locally (and adds GPU staging slack, item 8 above), and sequences are per process.
2. **Digest.** Replace the per-snapshot `state_hash` with a `RegionDigest` driven by `step_tracked` in
   `cpu_region/backend.rs` (around `:179-187`), and send digests every 60 CA ticks.
3. **Multiplayer driver.** In `multiplayer.rs` the host holds a `LockstepHost<RegionOp>` and joiners hold a
   `LockstepClient`. Their frames ride the unreliable channel; digests, chunk sums and repairs ride the
   reliable stream and bulk transfer. Stop calling legacy `fixed::finish` with a wall budget for any domain
   the region owns. (That file belongs to the parallel hosting agent, so coordinate.)
4. **Escrow.** Escrow (`cpu_region/ownership.rs:31, 53`) is tied to the local inventory. In lockstep the
   host debits the inventory when it stamps the input, and clients never escrow.
5. **Scale.** The region is a single sealed 8³ volume with 5 materials. Lockstep needs a host-stamped domain
   of chunks (activate and retire as inputs) and the full material table. That is the M1 slice's
   whole-region work.
6. **Resync.** An authority error currently stops the simulation. Under lockstep it triggers repair, then
   checkpoint resync.

## Proof

`crates/sim/tests/lockstep_world/` (engine_netcode is a dev-dependency of engine_sim only; the runtime
crates stay independent). A host and two clients each own an engine_sim world built from the same seed,
starting as a 64 x 32 x 64 stone tank holding sand and water. Every peer makes seeded random edits, all
routed through `LockstepHost`, over `link::Link` with 5% loss, 2% duplication and 40 ms latency with 25 ms
jitter, so datagrams reorder.

| Measure | Result |
|---|---|
| Ticks run | 5,600 |
| Inputs | 2,655 |
| Checkpoints compared, every 30 ticks | 186, all three peers identical in digest and in full `state_hash` |
| Datagrams lost | about 600 per client |
| Corruption | at tick 5,010, one client gets a hot sand grain that no other peer has |
| Detection | the first exchange after the corruption (tick 5,010) |
| Repair | 3 chunks, scheduled for tick 5,023; the client stalled 3 ticks |
| Convergence | digests equal from tick 5,040 on |
