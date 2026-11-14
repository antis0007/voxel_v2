# physical

## Summary
Read-only trace of the files on disk; nothing was built or run, so every claim below is a static reading.

Units: `farming.clock` is a u64 that advances once per fixed step (1/120 s, `engine_core::time::PLAYER_STEP_SECONDS`). `PULSE` is 30 ticks (0.25 s). `REST_TICKS` = 16 PULSE, `SOCIAL_TICKS` = 12 PULSE, the idle community wake is 600 ticks, and actor attach runs every 120 ticks. All positions, reach tests and body state are f32 metres.

A worker is a `crew::Worker` (or the leader fields inlined in `jobs::State`) holding `worker_pos: [f32;3]`, an optional `vehicle::ResidentBody` (health, velocity, yaw, status timers, all f32) and `motion_at`. The visible body is a `crate::world::adventure::Enemy` with `site.z == 8`.

**Attachment.** Driven by player feet only:
- Attach: `vehicle::service` spawns an actor at the saved pose when it is within 75 m of the player, its ±2 m neighbourhood is resident and the body box is free.
- Detach: the adventure retain drops actors 90 m or more from the player, calling `remember_worker` first.
- Checkpoint (detach or save) rewrites `worker_pos`, `body` and `motion_at`, so saved state differs with attachment and with the save tick.

**Unembodied workers do move, fall and take damage.** `service_worker` uses `actor.pos` if an actor exists, else the saved pose, and then calls `vehicle::advance_worker`. That builds a temporary `Enemy` and runs up to 30 substeps of water contact, status damage-over-time, gravity and the same `wildlife::tick` collision mover. Movement is capped at `min(clock - motion_at, PULSE)` per service, so a late agenda wake or terrain wait loses movement rather than catching up. Death is `health <= 0` or `y <= -256`. I found no fall damage in the gravity code.

**Unloaded terrain.** `residency::prepare` must return true before a worker moves or works. Otherwise the status is "Awaiting terrain" and the worker waits, while up to 2 chunks per call are requested from an asynchronous thread pool that uses wall-clock `Instant` and installs chunks into `world.chunks`.
- `known()` returns None only for a non-resident chunk that has a stored edit record; a non-resident unedited chunk is sampled procedurally.
- `World::get` never defaults to air: it reads the stored chunk from disk synchronously (Stone on failure) or samples procedurally.
- Unknown stockpile means wait; unknown mine or build cells are kept; `effects::replace` on a non-resident chunk returns `Deferred`.

**Outcomes decided by float physics.**
- Work reach: 1.65 m from feet + 0.7 m (2.25 m overhead), plus a line-of-sight trace.
- Routine arrival: 0.23 m horizontal.
- Waypoint pop: 0.1 m.
- Stall: under 0.05 m of movement for 8 services releases claims.
- Path costs and task ordering use float distances.
- Haul pickup matches a drop by exact f32 position equality.

**Construction.** Placement requires all of: claim owned, cargo matches, the worker in reach with clear line of sight, `unoccupied` (player box and every live attached actor), `workers_clear` (saved poses of the crew), and `Order::ready`. The voxel edit commits before the stack is decremented.

**Attachment-dependent outcomes.**
- `supply::execute` and `market::buy` require a live actor.
- `crew::physical` returns None for an alarmed actor but the saved pose for an unembodied worker.
- `unoccupied` only sees attached creatures.

**Death.** An embodied worker dies in the adventure death pass, which reaches `jobs::worker_fell` (drops cargo, mission pack and batch at the feet) and `communities::mark_dead`. The agenda also sets `died_at` when `crew::view` is None after a service.

`quarter/walk.rs` and `quarter/frontier/movement.rs` are check harnesses, not worker runtime.

## Summary corrections (verifier)
Static read only; nothing was built or run. 93 of 100 findings hold as written; 7 are imprecise (A17, A21, A53, A70, A80, A87, A93) and none is wrong. The summary needs these corrections:

1. **Death drops.** "Drops cargo, mission pack and batch at the feet" is wrong for the batch. recover_batch drops batch recovery at the batch station cell + (0.5, 1.0, 0.5) (jobs.rs:470); only carry and mission pack go to the feet.

2. **Terrain requests.** "Up to 2 chunks per call are requested" should read "installed". request() is called for every missing chunk; `admitted < 2` only limits installs (jobs/residency.rs:102-106). The pool is also polled only once per PULSE interval and is shared across all communities (jobs/residency.rs:93, communities.rs:229).

3. **Attachment.** "Driven by player feet only" and the single detach path understate it.
   - Attach also needs cave::loaded and a free body box (so it depends on chunk streaming), is skipped while travel.away holds today's day, and does not run on steps where express::step short-circuits travel::step (fixed.rs:68-69).
   - Detach also happens with no checkpoint through the 120-tick home-mismatch / not-present retain (vehicle.rs:393-398) and through sync_campaigns for mobilized villagers (vehicle.rs:142).
   - Community homes live in jobs.homes, not travel.homes (founding.rs:17), so the home-mismatch drop appears to apply to community workers. The reader listed this as an open question; the static reading supports it but it is not runtime-verified.

4. **Checkpoint.** "Checkpoint (detach or save) rewrites worker_pos, body and motion_at" omits that every embodied service does the same (jobs.rs:783, 789-790), as do communities::enlist (communities.rs:307) and founding::plan (founding.rs:28).

5. **Arrival thresholds.** The 0.23 m routine arrival and 0.1 m waypoint pop do not apply to mission travellers. They use 0.3 m path pops, a 2 m waypoint advance and a 2.5 m destination test (transit.rs:108-138), and they eat from their pack on a 24,000-tick timer with no arrival test (transit.rs:84-91).

6. **Order pruning.** "Unknown mine or build cells are kept" describes the phase-0 prune in jobs::service (jobs.rs:734-741), which runs only on the local crew state. Community states are serviced through crew::service_person and skip it; they have a separate mine-only prune at communities/development.rs:391.

7. **Attachment-dependent outcomes.** The list omits raids: defender contact reads only resident blocks (transit.rs:495), so an unloaded chunk turns a scuffle into a theft.

8. **Unembodied damage.** The open-question statement that status damage-over-time is the only damage for unembodied workers is incomplete. Raid scuffles write health to the saved body through set_health (transit.rs:504), and water contact can change status in resident chunks.

9. **Order within the step.** The open question is answerable from source: travel::step (attach) runs first, then jobs::service, then adventure.update (actor movement, death pass, 90 m detach) — fixed.rs:68-69, adventure.rs:3562, 3607.

## Checks: 93 confirmed, 7 not

- [imprecise] A17 (crates/app/src/world/adventure/wildlife/vehicle.rs:270) claim: The unembodied mover calls the same wildlife::tick as embodied actors, but with a fake player position 1000 m away and time 0.0, so player proximity cannot affect it.
  correction: The fake player point is feet + Vec3::splat(1000.0), about 1732 m away, not 1000 m (crates/app/src/world/adventure/wildlife/vehicle.rs:270). The conclusion holds for villagers: wildlife.rs:190-192 excludes them from the 4 m player test, and goal is passed as guard_goal so daily_goal is never called. The tick still reads world.travel.guard_posts and jobs::route_speed_multiplier.
- [imprecise] A21 (crates/app/src/world/farming/jobs/residency.rs:24) claim: The residency footprint is +/-0.5 m horizontally and -1.1..2.0 m vertically around the feet (plus feet-4 m, next waypoint, stockpile, home, beds, task cell).
  correction: The ±0.5 m, -1.1/+2.0 m footprint is right (jobs/residency.rs:24). For a worker on a mission, prepare returns early at jobs/residency.rs:51: it prepares only feet, feet-4 m, the path front and a corridor of up to 17 samples (±1 m, -2..+3 m) toward the mission waypoint. Stockpile, home, beds and task cell are skipped.
- [imprecise] A53 (crates/app/src/world/adventure/wildlife/vehicle.rs:397) claim: Every 120 ticks vehicle::service drops villager actors whose actor.home differs from civilization::home unless travel.homes or guard_posts holds the id; restored workers get e.home = saved pose (line 444).
  correction: The retain at crates/app/src/world/adventure/wildlife/vehicle.rs:393-398 also drops any villager actor for which civilization::present is false (dead, mobilized, or civilization disabled; line 395). It calls no remember_worker or checkpoint, so pose and body changes since the worker's last service are discarded.
- [imprecise] A70 (crates/app/src/world/farming/jobs/routine.rs:16) claim: Routine arrival (rest, social point, mission waypoint) is physical: horizontal distance under 0.23 m and height difference under 0.25*max(v,0.25) (previous line).
  correction: The 0.23 m and height thresholds are right for rest and social points (jobs/routine.rs:15-16). Mission waypoints do not use them: transit::walk returns first at jobs.rs:829, path waypoints pop at under 0.3 m in 3D (jobs/communities/transit.rs:115) and the mission waypoint advances at under 2 m (squared 4.0, transit.rs:138).
- [imprecise] A80 (crates/app/src/world/farming/jobs.rs:1166) claim: Task candidates are ordered by priority, group preference, then f32 squared distance from the worker's feet.
  correction: The third sort key is not always distance from the worker's feet. For Fetch and Fund it is the squared distance from the source chest to the build order (crates/app/src/world/farming/jobs.rs:1158); only other tasks use feet.distance_squared(task.point(v)) (jobs.rs:1159).
- [imprecise] A87 (crates/app/src/world/farming/jobs.rs:489) claim: clear_worker drops carried cargo, mission pack and batch recovery as dropped stacks at the feet (overflow when 1024 drops exist), then clears worker_pos, body and claims.
  correction: Carried cargo and the mission pack drop at the feet (jobs.rs:482, 494). Batch recovery does not: recover_batch drops it at the batch station, station cell + (0.5, 1.0, 0.5) in metres (crates/app/src/world/farming/jobs.rs:470).
- [imprecise] A93 (crates/app/src/world/farming/jobs/crew.rs:265) claim: An idle community worker wakes at retry_at or now + 600 ticks (PULSE with no homes); a worker with a path, task, cargo or a body needing ticks wakes at now + PULSE.
  correction: The idle wake is max(retry_at, now + 600 ticks), or now + PULSE with no homes (jobs/crew.rs:265), then lowered to routine.due and to the next meal time, each floored at now + PULSE (crew.rs:266-268). The now + PULSE cases also include an active batch, a mission, and any routine phase other than Work (crew.rs:263-264).

## Missed (verifier)
- Community homes are stored in the community's jobs.homes, not world.travel.homes. So the 120-tick retain at wildlife/vehicle.rs:393-398 (keep only if travel.homes or guard_posts has the id, or actor.home == civilization::home) drops a restored community worker's actor whenever its saved pose is not exactly the home-destination point, with no checkpoint. The spawn loop in the same call then recreates it at the last serviced worker_pos with the last captured body. Static reading only; needs a runtime check. [crates/app/src/world/farming/jobs/communities/founding.rs:17] `jobs.homes.insert(id, home.clone());`
- A worker on a mission bypasses the whole work/routine path (transit::walk returns true first at jobs.rs:829). Its meal is taken from the carried pack on a 24,000-tick timer (200 s at 120 Hz) with no position or stockpile test. A missing meal turns the mission back. Fatigue over 24,000 ticks forces a 1,200-tick rest (transit.rs:101-102). [crates/app/src/world/farming/jobs/communities/transit.rs:91] `mission.next_meal = now.saturating_add(24_000);`
- A raid's outcome depends on chunk residency. A defender is found by saved poses within 3 m (squared 9.0, transit.rs:465) and a contact line that reads only resident blocks. If any sample cell is in a non-resident chunk, contact fails, no scuffle happens, and the raider takes up to 8 meals from the chest instead (transit.rs:481-486). [crates/app/src/world/farming/jobs/communities/transit.rs:495] `w.resident_block(p).is_some_and(|block| !block.is_solid())`
- Mission travel uses its own float thresholds: path waypoints pop at under 0.3 m in 3D (transit.rs:115), the mission waypoint advances when the saved or actor feet are within 2 m (squared 4.0), and 8 stalled services (under 0.05 m moved) pick a new waypoint or turn the mission back. [crates/app/src/world/farming/jobs/communities/transit.rs:138] `if super::super::metres(waypoint, scale).distance_squared(feet) < 4.0 {`
- Step order, answering the reader's open question: travel::step (which runs the attach pass vehicle::service) runs before adventure::tick in the same fixed step. It is short-circuited when orbital::express::step returns true, so no attach or home-mismatch pass runs on those steps. [crates/app/src/simulation/fixed.rs:68] `let riding = legacy && (world::orbital::express::step(world, player, input, move_player)`
- Inside adventure::tick, farming::tick and jobs::service (adventure.rs:3560-3562) run before adventure.update, which contains actor gravity and movement (1925, 1945), the death pass (3099) and the 90 m detach retain (3238). A worker is therefore serviced from the pose its actor had at the end of the previous step, and an actor detached this step is serviced unembodied next time. [crates/app/src/world/adventure.rs:3607] `let consumed = adventure.update(world, player, dt, cast, cycle, melee);`
- The worker terrain source pool is polled at most once per PULSE interval (clock / PULSE), and one pool is shared by the local crew and every community (with_site swaps sources at communities.rs:229). Finished chunks become installable only at the first prepare call of a later PULSE interval, so 'Awaiting terrain' is quantised to PULSE on top of wall-clock thread timing. [crates/app/src/world/farming/jobs/residency.rs:93] `if sources.polled != Some(interval) {`
- Installing a ready source chunk can be refused by global memory state: under edited-memory pressure, prepare returns false when the per-poll pressure allowance is used up or the region write backlog is full. A worker's terrain wait therefore depends on unrelated world edits and save-writeback progress. [crates/app/src/simulation/residency.rs:444] `&& (missing.len() > self.pressure_chunks_left || world.regions.backlog_full())`
- Status damage on an unembodied worker cancels its work. Damage sets flash to 0.06 (vehicle.rs:267), wildlife::tick turns flash into alert = 4.0 s, and the executor then interrupts the routine, releases claims and resets the task. advance_worker sets actor.home = feet (vehicle.rs:259), so the unembodied retreat target is its own position, while an embodied actor retreats to its e.home. [crates/app/src/world/farming/jobs.rs:810] `if body.seeking_safety() {`
- Mission candidate selection depends on physical position: only workers whose saved pose is within 3 m (squared 9.0) of the community stockpile point are eligible. The pick among them is by grievance for raids, otherwise by hash_u32(id ^ target) (transit.rs:178-183). [crates/app/src/world/farming/jobs/communities/transit.rs:171] `&& p.pos.is_some_and(|at| c.jobs.stockpile.is_some_and(|pile|`
- Player and attached-actor occupancy gates more than placement. construction::unoccupied also filters Fetch and Fund candidates at selection (here) and their continued validity (jobs.rs:1037), so the player standing in a build cell stops workers fetching material for it. [crates/app/src/world/farming/jobs.rs:1110] `|| crew::planned(j,Task::Fetch(from,o)) || !construction::unoccupied(world,o,player)`
- A third detach path exists: sync_campaigns, run at the end of vehicle::service every 120 ticks, removes the actor of any politically mobilized villager with no remember_worker or checkpoint. [crates/app/src/world/adventure/wildlife/vehicle.rs:142] `if villager_id(actor).is_some_and(|id| politics.mobilized(id)) { return false; }`

## Open questions
- Damage sources for embodied worker actors were not fully enumerated. I saw status damage-over-time, water contact and the volatile death blast (adventure.rs:3200); hits from hostile mobs, spells, settlement_guard and night code were not traced. For unembodied workers the only damage is status damage-over-time from the saved status.
- No fall damage or lava/fire block-contact damage for Enemy actors was found in navigation::gravity or wildlife::tick. I did not exhaustively search adventure.rs (3997 lines) to rule it out.
- vehicle.rs:393-398 appears to drop and re-spawn, every 120 ticks, any community worker actor whose actor.home (set to the saved pose at restore) differs from the community home and that has no travel.homes entry. That would reset transient actor state and rewind the pose to the last serviced worker_pos. Needs a runtime check.
- An embodied live worker that falls below y = -256 is removed by the retain at adventure.rs:3246 without remember_worker or worker_fell. Death then seems to depend on a later unembodied service continuing the fall from the last saved pose. Not verified.
- cave::loaded needs a +/-2 m resident box but residency::prepare only guarantees about +/-0.5 m. An unembodied worker near an unloaded chunk border may receive gravity but no walking, and accumulate stalls. Frequency unknown.
- The simulation Residency lane uses Instant::now, a 500 microsecond poll budget and thread completion order. Whether headless or test runs make 'Awaiting terrain' durations reproducible was not determined.
- supply::execute returns early without a live actor (transport.rs:57), after which work() resets the task. Whether an unembodied courier loops forever on Supply/Recover, or another rule prevents selection, was not confirmed.
- Order within the fixed step between travel::step (attach) and adventure::tick (jobs service, actor update, detach) was not traced; it decides whether a worker can be serviced embodied and unembodied in the same tick.
- In multiplayer, jobs::enabled is false, but vehicle::service still lists crew::saved_workers when civilization is enabled. Whether saved workers get actors that then idle on a host was not traced (multiplayer/authority.rs calls adventure::tick).
- Paths for followed, guard-post and politics-mobilized workers (campaign actors, politics::wound, crew::set_health callers) were only skimmed; how campaign damage and death map back to the worker body is not established.
- crew::Worker.path, last_walk, stalled, status and last_placement are #[serde(skip)], so a reload replans routes. Whether that changes outcomes relative to an uninterrupted run was not analysed.

## Confirmed findings
- A1 PULSE is 30 ticks: 0.25 s at the 120 Hz player step. All job timing constants are multiples of PULSE or raw ticks of farming.clock. [crates/app/src/world/farming/jobs.rs:31]
- A2 The tick unit is defined as 1/120 s in engine_core. [crates/core/src/time.rs:5]
- A3 farming.clock is a u64 incremented once per farming::tick; every job 'now' reads it. [crates/app/src/world/farming.rs:62]
- A4 adventure::tick (which runs farming::tick and jobs::service) only runs on the legacy path, when ca_view is none. [crates/app/src/simulation/fixed.rs:59]
- A5 jobs::service receives the player's feet in metres as its only spatial input besides the world. [crates/app/src/world/adventure.rs:3562]
- A6 Workers are disabled when a multiplayer context exists. [crates/app/src/world/farming/jobs.rs:233]
- A7 Workers also require civilization enabled and no CA-region view. [crates/app/src/world/farming/jobs.rs:236]
- A8 The saved worker position is f32 metres, not a cell. [crates/app/src/world/farming/jobs/crew.rs:26]
- A9 ResidentBody is the persistent physical state for a worker whose actor may be absent; its fields are f32 (health, velocity, yaw, slow, stagger, alert, flash, shield) plus status and affixes. [crates/app/src/world/adventure/wildlife/vehicle.rs:32]
- A10 A worker's actor is an Enemy identified by villager kind and site.z == VILLAGER_Z (8), with the resident id in site.x. [crates/app/src/world/adventure/wildlife/vehicle.rs:193]
- A11 The executor takes the worker's feet from the live actor if one exists, otherwise from the saved pose. Attachment changes which float position every reach test sees. [crates/app/src/world/farming/jobs.rs:784]
- A12 Movement time credited per service is capped at one PULSE, so a late wake or terrain wait loses movement instead of catching up. [crates/app/src/world/farming/jobs.rs:788]
- A13 An unembodied worker (no actor) still moves: service_worker calls the collision mover toward the first path waypoint. [crates/app/src/world/farming/jobs.rs:800]
- A14 advance_worker runs at most 30 fixed substeps on a temporary Enemy built from the saved body. [crates/app/src/world/adventure/wildlife/vehicle.rs:261]
- A15 An unembodied worker takes status damage-over-time (burn, poison) from its saved status each substep, in f32. [crates/app/src/world/adventure/wildlife/vehicle.rs:265]
- A16 An unembodied worker falls: gravity is applied each substep before the walk step. [crates/app/src/world/adventure/wildlife/vehicle.rs:269]
- A18 Gravity is float: 14 m/s^2 with a 14 m/s terminal speed; on collision velocity is zeroed. No fall damage is applied in this function. [crates/app/src/world/adventure/navigation.rs:204]
- A19 An unembodied worker dies when body health reaches 0 or feet fall to y <= -256 m; the executor then marks dead, clears the worker and records it away for the day. [crates/app/src/world/farming/jobs.rs:802]
- A20 Before moving or working, the worker's terrain footprint must be resident; otherwise it waits with status 'Awaiting terrain' and keeps its task. [crates/app/src/world/farming/jobs.rs:791]
- A22 Missing chunks are requested from the asynchronous source lane and at most 2 are installed per prepare call; the worker is not ready until all are resident. [crates/app/src/world/farming/jobs/residency.rs:102]
- A23 The source lane generates or loads chunks on background threads, so completion order is not tick-deterministic. [crates/app/src/simulation/residency.rs:132]
- A24 The source lane poll is bounded by wall-clock time (500 microseconds), so how many results land per tick depends on machine speed. [crates/app/src/simulation/residency.rs:250]
- A25 Worker terrain readiness makes chunks resident (forces generation and installation), which changes world.chunks for every other system. [crates/app/src/simulation/residency.rs:456]
- A26 known() returns None only for a non-resident chunk that has a stored edit record; a non-resident never-edited chunk is answered by world.get (procedural sample). [crates/app/src/world/farming.rs:102]
- A27 World::get on a non-resident edited chunk does a synchronous stored-chunk read and falls back to Stone if the load fails; it never returns air by default. [crates/app/src/world.rs:1829]
- A28 If the stockpile cell is unknown the worker waits ('Stockpile unloaded') without resetting its task. [crates/app/src/world/farming/jobs.rs:843]
- A29 The per-PULSE mine-order prune keeps orders whose cell is unknown; unknown is not treated as air or as mismatch. [crates/app/src/world/farming/jobs.rs:737]
- A30 Build orders are dropped as complete only when the cell is known to hold the target block; unknown cells keep the order. [crates/app/src/world/farming/jobs.rs:739]
- A31 Standability (used by pathfinding, routine goals, mission waypoints) needs a known solid floor and known air for ceil(1.8/v) cells; an unknown cell is not standable. [crates/app/src/world/farming/jobs.rs:571]
- A32 The body collision test (free/collides_world) reads world.get directly, not known(), so the mover samples non-resident cells procedurally or from disk. [crates/app/src/world/collision.rs:27]
- A33 wildlife::tick does no horizontal movement when the actor's neighbourhood is not resident; gravity has already run by then (both for embodied actors and inside advance_worker). [crates/app/src/world/adventure/wildlife.rs:185]
- A34 cave::loaded requires resident chunks at the corners of a +/-2 m by -2..+3 m box, which is wider than the worker residency footprint (A21). [crates/app/src/world/adventure/wildlife/cave.rs:8]
- A35 Water contact (wet status, which also clears burn) is only evaluated in resident chunks. [crates/app/src/world/adventure/water_contact.rs:26]
- A36 Embodied actors get gravity every adventure tick with float dt. [crates/app/src/world/adventure.rs:1925]
- A37 Embodied worker actors are moved by wildlife::tick each tick, with the real player feet passed in. [crates/app/src/world/adventure.rs:1945]
- A38 The embodied actor's walk target is the first waypoint of the executor's path, or its own position when the path is empty. [crates/app/src/world/farming/jobs.rs:514]
- A39 Walk speed is the species speed times 2.0 for Build, Fetch, Fund and Buy routes (1.0 otherwise), in f32. [crates/app/src/world/farming/jobs.rs:523]
- A40 The walk step is float: distance = speed * dt, subdivided into steps of at most 0.12 m, each requiring a free body box and resident footing within 1.1 m. [crates/app/src/world/adventure/wildlife.rs:232]
- A41 A villager actor becomes alarmed (alert = 4.0 s) only from its own hit flash; player proximity is excluded for villagers on the preceding lines. [crates/app/src/world/adventure/wildlife.rs:193]
- A42 An alarmed embodied worker (flash or alert > 0) has its routine interrupted, claims released and task reset. [crates/app/src/world/farming/jobs.rs:769]
- A43 Attachment is evaluated once every 120 ticks (1 s) from vehicle::service. [crates/app/src/world/adventure/wildlife/vehicle.rs:385]
- A44 vehicle::service is driven by the player's feet (travel::step). [crates/app/src/world/travel.rs:314]
- A45 An actor is attached when the saved pose is within 75 m of the player and the neighbourhood is resident (the next line also requires a free body box). [crates/app/src/world/adventure/wildlife/vehicle.rs:438]
- A46 On attach the saved ResidentBody is applied to the new actor (health, velocity, yaw, status and so on). [crates/app/src/world/adventure/wildlife/vehicle.rs:441]
- A47 Applying a saved body clamps health to the actor's max health, so attachment can change health. [crates/app/src/world/adventure/wildlife/vehicle.rs:79]
- A48 Detach is by distance from the player: single-player 'near' is under 90 m. [crates/app/src/world/adventure.rs:3242]
- A49 A live actor being detached is checkpointed into the jobs state first. [crates/app/src/world/adventure.rs:3244]
- A50 The retain predicate removes any actor at y <= -256 or with health <= 0; a live actor below -256 is removed with no remember and no death call on this line. [crates/app/src/world/adventure.rs:3246]
- A51 Checkpoint writes three saved fields from the actor: worker_pos, body (next line) and motion_at (line after). Attachment therefore changes more than pose. [crates/app/src/world/farming/jobs/crew.rs:402]
- A52 The save path clones the jobs state and checkpoints every live actor at save time, so saved bytes depend on which workers are embodied and on the save tick. [crates/app/src/world/farming/sidecar.rs:102]
- A54 Generic work reach is a float distance from feet + 0.7 m to the target centre: 1.65 m, or 2.25 m for overhead builds. [crates/app/src/world/farming/jobs.rs:584]
- A55 A build target level with the body must be horizontally outside the body: max(|dx|,|dz|) + 0.001 >= v/2 + 0.4 m. [crates/app/src/world/farming/jobs.rs:592]
- A56 Reach also needs line of sight from feet + 0.7 m to the target via a float voxel trace that calls world.solid/world.get. [crates/app/src/world/farming/jobs.rs:599]
- A57 For Build, Haul, Recover and Routine any solid hit on that trace fails the reach test. [crates/app/src/world/farming/jobs.rs:604]
- A58 Placement is refused while the cell is occupied by the player's body box (0.8 x 1.8 x 0.8 m at the player's feet). [crates/app/src/world/farming/jobs/construction.rs:170]
- A59 Placement is also refused while any live attached actor (any kind, including the builder's own actor) overlaps the cell; unattached creatures are not seen. [crates/app/src/world/farming/jobs/construction.rs:175]
- A60 workers_clear tests crew members by their saved worker_pos (as of each member's last service), not by live actor positions. [crates/app/src/world/farming/jobs/construction.rs:185]
- A61 place() gates on both occupancy tests and reports 'Build site occupied' without consuming cargo. [crates/app/src/world/farming/jobs/construction.rs:200]
- A62 A build cell is ready only if it is known to hold the expected block (air, or the paved-over block) and has a known solid neighbour. [crates/app/src/world/farming/jobs/construction.rs:65]
- A63 The voxel edit is attempted first; on failure place() returns with cargo intact ('Waiting to place block'). The stack is decremented only after the commit (line 225). [crates/app/src/world/farming/jobs/construction.rs:213]
- A64 effects::replace returns Deferred when the target chunk is not resident. [crates/app/src/world/effects.rs:82]
- A65 The player position also decides whether a Build task stays valid, and so affects task and route choice. [crates/app/src/world/farming/jobs.rs:1049]
- A66 Mining takes ceil(3 + 4*hardness) services clamped to 3..60 (f32 hardness); the commit also requires the chunk resident (line 967). [crates/app/src/world/farming/jobs.rs:963]
- A67 Haul pickup finds the dropped stack by exact f32 position equality. [crates/app/src/world/farming/jobs.rs:948]
- A68 Supply and Recover cargo pickup requires a live actor; with no body the action returns without transferring anything. [crates/app/src/world/farming/jobs/supply/transport.rs:57]
- A69 A market purchase requires the merchant's live actor within 2.5 m of the seller chest; an unembodied merchant counts as away. [crates/app/src/world/farming/jobs/market.rs:69]
- A71 waypoint_reached takes f32: horizontal under 0.1 m (squared 0.01) and height under 0.25*max(scale,0.25). [crates/app/src/world/farming/jobs/routine/policy.rs:121]
- A72 Rest completes after 16 PULSE (480 ticks, 4 s) of dwell accumulated only while physically arrived at the home cell. [crates/app/src/world/farming/jobs/routine.rs:9]
- A73 Rest is blocked ('Home Occupied') when another household member's physical position is within 1.8 m of the home destination. [crates/app/src/world/farming/jobs/routine.rs:109]
- A74 crew::physical returns no position for a live actor that is alarmed, but an unembodied worker's saved pose is returned without that test (line 395). [crates/app/src/world/farming/jobs/crew.rs:392]
- A75 A social meeting counts only while both members are physically arrived at their points with a clear line of sight (next lines); completion needs 12 PULSE (360 ticks, 3 s) of dwell. [crates/app/src/world/farming/jobs/routine/social.rs:27]
- A76 A meal is a Produce(Eat(pile)) task executed under the generic 1.65 m reach at the stockpile chest; it debits one meal item, and only then advances the routine. [crates/app/src/world/farming/jobs/production/actions.rs:85]
- A77 Non-batch production actions (seed, sow, harvest, eat) silently do nothing unless the task cell's chunk is resident. [crates/app/src/world/farming/jobs/production/actions.rs:36]
- A78 Route stall detection is float: under 0.05 m moved, or under 0.025 m progress toward the waypoint, per service; 8 stalls release claims and set a retry of 2 PULSE. [crates/app/src/world/farming/jobs.rs:887]
- A79 Path search cost and heuristic are integers derived from f32 distances (900/1000 per cell). [crates/app/src/world/farming/jobs.rs:645]
- A81 Job range is float: RANGE is 24 m from stockpile or feet; colonies with homes use 48 m. [crates/app/src/world/farming/jobs.rs:1006]
- A82 Mission arrival is physical: within 2.5 m of the destination (squared 6.25); survey outbound uses horizontal distance only. [crates/app/src/world/farming/jobs/communities/transit.rs:108]
- A83 Mission cargo handover requires the saved pose within 3 m of the stockpile chest (squared 9.0). [crates/app/src/world/farming/jobs/communities/transit.rs:333]
- A84 Raid scuffles write f32 damage to the saved body and to the live actor; health is floored at min(health, 2.0), so they are nonlethal. [crates/app/src/world/farming/jobs/communities/transit.rs:503]
- A85 An embodied worker's death is handled in the adventure death pass for passive kinds. [crates/app/src/world/adventure.rs:3099]
- A86 That pass calls jobs::worker_fell with the actor's position, then records the villager away for the day (line 477). [crates/app/src/world/adventure/wildlife/vehicle.rs:471]
- A88 mark_dead sets died_at to the current clock, notifies the regional ledger and cancels the agenda wake. [crates/app/src/world/farming/jobs/communities.rs:342]
- A89 The agenda also learns of a death when the worker is no longer viewable after its service: it sets died_at = now and schedules no further wake. [crates/app/src/world/farming/jobs/communities.rs:281]
- A90 The community agenda is a BTreeSet of (due, site, person), rebuilt from saved wakes and not itself serialized. [crates/app/src/world/farming/jobs/communities.rs:65]
- A91 At most 8 agenda events are processed per tick. [crates/app/src/world/farming/jobs/communities.rs:17]
- A92 Crew capacity is 64. [crates/app/src/world/farming/jobs/crew.rs:4]
- A94 The local player crew runs outside the agenda: the leader at phase 0 of each PULSE, helpers on a phase-indexed roster slice. [crates/app/src/world/farming/jobs.rs:746]
- A95 Adding a resident to the local crew requires a live actor (the feet come from the actor). [crates/app/src/world/farming/jobs/crew.rs:135]
- A96 Constructor recruitment is gated by player distance to the stockpile (24 m). [crates/app/src/world/farming/jobs/crew.rs:162]
- A97 The walkability cache key includes whether the chunk is resident; the file comment says cache identity never affects path tie-breaking. [crates/app/src/world/farming/jobs/navigation.rs:13]
- A98 quarter/walk.rs is a check harness that drives the Player through update_player; it is not worker movement. [crates/app/src/world/farming/jobs/quarter/walk.rs:56]
- A99 quarter/frontier/movement.rs is a check of diagonal_clear and path_with_limit on a Superflat fixture; it is not runtime. [crates/app/src/world/farming/jobs/quarter/frontier/movement.rs:20]
- A100 Drop discovery for hauling is a rotating window of at most 256 dropped stacks per selection. [crates/app/src/world/farming/jobs/drop_scan.rs:2]