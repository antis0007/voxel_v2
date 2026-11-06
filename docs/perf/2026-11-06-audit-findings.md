# Code audit findings (2026-11-06)

Six read-only reviews of the shared tree: live water, rigid bodies, streaming
and meshing, render frame cost, determinism and netcode, worldgen and loading.
"Checked" marks findings re-read against the code after the review. Line
numbers are from the shared tree on 2026-11-06 and move as peers edit.

## Ranked across areas

| # | Severity | Area | Finding | Where | Checked |
|---|---|---|---|---|---|
| 1 | High | Streaming | Exact-LOD mesh planning (`MeshPlan::for_chunk`) runs under one global permit, so up to 8 mesh workers plan one chunk at a time; each job also reserves 24 MiB against a 256 MiB budget while real scratch is about 3 MiB. Likely cause of "300 pending, 0 to 16 generating". | `render/streaming.rs` 1522, `streaming/mesh_budget.rs` 42-58, `mesh/memory.rs` 4 | yes |
| 2 | High | Worldgen | Workers sharing a footprint wait on one mutex while one worker samples all 1,156 columns; a cancelled builder discards its rows. | `worldgen/src/generation_cache.rs` 136-151 | yes |
| 3 | High | Worldgen | Horizon `map_blocks` spawns fresh scoped threads per stage, so every thread-local plan cache (sites, homes, drainage, columns) starts cold on every stage and rebuild. | `render/horizon/parallel.rs` 86 | yes |
| 4 | High | Water | A cell whose cell below is "not full" wakes itself and returns even when the move can never happen (rounds to 0, fraction table full), so it spins awake forever. | `simulation/cellular/drainage.rs` 197-205 | yes |
| 5 | High | Water | Pressure outlet triggers at a 2/8 head difference but always moves `volume / 2`, overshooting and flowing back every 8 ticks. | `simulation/cellular/drainage.rs` 284-295 | yes |
| 6 | High | Render | Any wind marks shadow cascade 0 urgent every frame and cascade 1 every other frame, re-culling and sorting every published chunk, only to animate tuft shadows. | `render/shadow.rs` 703-714 | yes |
| 7 | High | Render | A horizon shadow generation change forces all 5 cascades in one frame (bypassing far-layer staging); generation bumps on every horizon publish and collar flip. | `render/shadow.rs` 736-798, `render/horizon.rs` 397, 873 | no |
| 8 | High | Render | `models::build` rebuilds orbital braid struts and re-uploads the scene every frame, including while the loader shows or the game is paused. | `mesh/models/orbital.rs` 272, `main.rs` 6207 | no |
| 9 | High | Rigid | Motion is blocked by any non-air cell but support counts only `is_solid` cells: a body on water or grass is never grounded, never sleeps and sweeps every tick. | `simulation/rigid.rs` 121, 156, 444 | yes |
| 10 | High | Rigid | Wall-clock `deadline.has_time()` decides which bodies run and where a sweep stops; leftover time feeds `debt`, so results depend on CPU speed. | `simulation/rigid.rs` 256, 365 | no |
| 11 | High | Sim | Near-list rebuild uses `Instant::now()` every 500 ms; the list orders the water pass, so headless runs depend on machine speed. | `simulation/world.rs` 472, 483 | yes |
| 12 | High | Streaming | Uploads capped at 8 meshes per frame and 16 in flight (empty meshes count), so convergence speed scales with frame rate. | `render/streaming.rs` 2735, 2784, `streaming/pending_gpu.rs` 31 | no |
| 13 | High | Streaming | Any change in urgent coords clears the priority heap and re-ranks every pending key, bypassing the 100 ms refresh. | `render/streaming.rs` 2446-2450 | no |
| 14 | Medium | Water | Resting dead-band (10,000 uL) is about 3,000x below one visible surface step, so settled pools keep levelling; each move wakes 22 cells and invalidates snapshots. | `cellular/drainage.rs` 244-245 | no |
| 15 | Medium | Worldgen | Valley and grove plans have no in-flight dedupe: every worker misses at once and runs the full survey at world open. | `worldgen/src/amberroot.rs` 64-77, `valley.rs` 318-334 | no |
| 16 | Medium | Worldgen | No disk cache for solved planets or horizon samples; every launch repeats them. | `geo/planet.rs` 333, `planetary/mod.rs` 119 | no |
| 17 | Medium | Rigid | Per awake body per tick: an owner map of every body's cells is rebuilt, and mass properties are recomputed per substep. | `simulation/rigid.rs` 356, 281, 388, `rigid/contact.rs` 111 | no |
| 18 | Medium | Rigid | Move into protected cargo passes the cell loop, fails `admit_motion`, retries every tick: body hangs and never sleeps. | `simulation/rigid.rs` 29, 421 | no |
| 19 | Medium | Render | Cloud pass copies the full cloud image every frame with temporal reuse compiled out; render-scale cap pins clouds at 360 rows. | `render.rs` 800-862, 5464 | no |
| 20 | Medium | Render | Postprocess runs two full-resolution Rgba16Float passes at render scale 1; indirect-light target is cleared and stored even when bounce light is off. | `render/postprocess.rs` 354-391, `render.rs` 5221 | no |
| 21 | Medium | Lockstep | Desync digest hashes cells only; free list, empty bricks and dirty masks affect stall decisions but are not hashed. | `crates/sim/src/digest.rs`, `kernel.rs` stall check | no |
| 22 | Medium | Lockstep | No session, model hash or config in lockstep messages; build id ignores cargo profile and line endings. | `netcode/src/lockstep/wire.rs` 40-58, `app/network_build.rs` | no |
| 23 | Medium | Lockstep | Checkpoint decode trusts remote capacity, chunk coordinates and cell values. | `crates/sim/src/store/checkpoint.rs` 50-81 | no |
| 24 | Medium | Streaming | Fading LOD mesh is trimmed immediately near the GPU budget, so LOD swaps hard-cut (pop-in). | `streaming/residency.rs` 183, `streaming.rs` 1297, 1381 | no |
| 25 | Medium | Streaming | One failed mesh job pauses all dispatch for 1 s; a full arena drops the finished mesh and regenerates it. | `render/streaming.rs` 2172, 2816 | no |
| 26 | Low | Sim | A later edit to the kernel added `.expect("scheduled brick slot must exist")` outside tests; it would panic if a brick got two groups. | `crates/sim/src/kernel.rs` 147 | no |

## Notes

- `MAX_MESH_JOBS` (64) and terrain admission are not the binding limits;
  `ready_chunks` (128) and the planning permit are.
- `pending_schedule` includes parked off-screen keys; log `parked_offscreen`,
  `meshing`, `ready_chunks`, `ready_meshes` and `queued_mesh_bytes` together
  before reading a pending count as starvation.
- The horizon now holds the loader for at most 2 s (`quality.rs` 33), so the
  older 21 to 41 s figure is stale; the build now competes with spawn
  streaming for cores instead.
- No NaN path, cell duplication or hash-order leak was found in the rigid
  solver; no integer overflow in water uL maths; content floats convert to
  integers once.
