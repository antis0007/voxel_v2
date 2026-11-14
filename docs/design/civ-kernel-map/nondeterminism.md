# nondeterminism

## Summary
Read-only pass over the files on disk; nothing was built or run.

Time units. `farming.clock` counts fixed steps at 120 Hz. PULSE is 30 steps (0.25 s). Travel "day" is clock / 57,600 steps (480 s). The worker code never reads DayNightCycle.

Driver. Farming and `jobs::service` run once per fixed step inside `adventure::tick`, only when `ca_view` is none. Both drivers pass `Budget::UNBOUNDED` to `farming::tick`; `jobs::service` takes no Budget at all. The window differs from headless in step count only: at most 16 steps per frame, elapsed time above that is dropped, an 8 ms wall deadline defers remaining steps, and frame time is multiplied by the production "speed" (1/2/4/8). In network mode the window loop runs zero steps. Steps are whole, so the tick sequence is the same; only wall-to-tick mapping varies.

What is not replayable today:
- **Async terrain.** Worker readiness polls a threaded source pool using `Instant`, `try_recv` and a 500 µs cutoff. The tick at which "Awaiting terrain" ends depends on thread timing and on a memory-pressure byte limit. Checks avoid this by pre-admitting chunks.
- **Async settlement survey.** `prepare_settlements_near` returns false until a background thread finishes; it gates town transit policy and the once-per-second regional service.
- **Player position.** It decides actor spawn (75 m) and despawn (90 m), blocks placement in an occupied cell, and decides which settlement ledgers exist and when their history starts.
- **Actor attachment.** An attached worker moves every tick under full actor AI; a detached one moves at most 30 substeps per service through `advance_worker`. All positions and reach tests are f32.
- **Save/load.** Path, stall counter, last walk, last placement and status are `serde(skip)`, so a reload replans routes and restarts stall counting.
- **Crop growth.** Rolls are keyed by interval index but use conditions sampled at evaluation time, and plots are serviced only while their chunk is resident.

Not hazards: the std HashMaps in the path search, supply frame and recipe table are only used for keyed lookups, never iterated. The navigation thread-local cache is revalidated by chunk version stamps. `sort_unstable` keys are total except one chunk-keyed sort whose result is only used per chunk. No `rand` appears in farming, travel or vehicle.rs.

Randomness. Resident draws are stateless `hash_u32` of immutable keys: personality (seed, id), artisan pick (id, no seed), traveller pick (id ^ target, no seed), ledger weather and war roll (seed ^ site ^ day, same input for both). Staggers use `id % n`. Sequence-dependent choices are `max_by_key` ties (roster order), `next_candidate`, scan cursors, circle serial, the survey counter (advances only when async terrain succeeds) and the gourd side roll (keyed by clock at evaluation).

Agenda. Order is (due, site, person); at most 8 events per tick; the rest stay due and `oldest_delay` records lateness. It is rebuilt on load from saved `wakes` and `next_plan`. The local crew runs before it by phase slice.

Reusable infrastructure:
- Sidecar `farming.vxf`: magic `VXFRM001`, FNV-1a checksum, RON text for jobs and communities, BTreeMap only.
- The `--headless` scenario runner and insta snapshots, but its hash omits farming, chests and actors, and Step has no job commands.
- Native checks behind `--city-livelihood-check=`, `--city-quarter-check=` (env-switched variants), `--city-group-check=`, `--city-service-check=`. They prove conservation, sidecar and full-checkpoint round trips, obstruction recovery and slice scheduling.
- `supply/check/snapshot` already compares cached against direct queries.
- No check or test exercises the community agenda.

First shadow target: `--city-livelihood-check=<root>` (Superflat, seed 424242, 3 residents, crops, meals, a shelter, sidecar reloads). It uses the local crew and has no home routine. The smallest fixture with housing is the 12-resident colony.

## Summary corrections (verifier)
Nothing was built or run for this verification either; all of the below is from reading the files on disk. No finding is wrong outright; 9 of 115 are imprecise.

1. "Checks avoid this by pre-admitting chunks" is over-general. Only the checkpoint-based routine checks call materialize::admit (routine/check.rs:31, neighborhood.rs:47, replay.rs:8). The livelihood check, which the reader recommends as the first shadow target, has no admission step, so it may itself contain wall-clock-dependent "Awaiting terrain" ticks.

2. The checks "prove" nothing until they are run; they assert. There is also an unresolved tension with N28. The reload asserts (production/check.rs:41, routine/check.rs:112, colony/check.rs:187, frontier/check.rs:137) compare RON of the live jobs with a decoded checkpoint whose worker_pos, body and motion_at were overwritten from live actors (crew.rs:398-410). By reading, they can hold only when every attached actor's pose equals its last serviced pose. Whether they pass today was not established.

3. "An idle worker's wake is pulled earlier by retry_at" is backwards: retry_at can only delay it (crew.rs:265).

4. "The window differs from headless in step count only" understates the difference:
   - The window calls fixed::finish with settle=false, a wall-clock budget and advance_day=!freeze_time (main.rs:5776-5777); headless passes UNBOUNDED, true, true.
   - Async terrain and survey results are polled by wall time, so the same tick index can see different world facts. "The tick sequence is the same" holds for the clock only, not for outcomes.
   - Most city checks call gameplay with move_player=false; headless uses true.

5. "The smallest fixture with housing is the 12-resident colony" needs qualifying:
   - It requires both VOXEL_CITY_START_SITE=1 and VOXEL_CITY_COLONY=1 (quarter/check.rs:42, frontier/check.rs:7).
   - Households start empty (colony/scene.rs:64), so the rest/home routine only runs after all six projects complete, or in the daily-life check, which needs a saved completed-colony checkpoint.
   - The routine also applies to community residents through the homes map (routine.rs:34), which no fixture drives.

6. The reader's open question about Superflat is answerable: generated settlements need planetary::active (worldgen/civilization.rs:123). No existing check or fixture can reach the community agenda; shadowing agenda order needs a Planet world.

7. The sidecar description omits what is not saved: the crop scan cursor (sidecar.rs:138), the agenda counters, and the source-pool poll stamp. Reload therefore changes plot evaluation timing in addition to routes and stall counters.

8. The crop growth sentence is slightly off: irrigated crops deliberately discard overdue intervals (farming.rs:204-205) rather than replaying them under current conditions.

9. The multiplayer gates (jobs.rs:233, base.rs:111, adventure.rs:3554, 3240) are compiled only with the `multiplayer` feature. It is on by default (crates/app/Cargo.toml:9).

10. The reader's open question about the save directory is answerable: storage::save_directory reads VOXEL_SAVE_DIR once per process into a OnceLock (world/storage/paths.rs:5-11). One process can therefore run only one disposable root.

## Checks: 109 confirmed, 9 not

- [imprecise] A10 (crates/app/src/world/farming/jobs/crew.rs:265) claim: An idle community worker's next wake is 600 ticks out (PULSE for a homeless executor), pulled earlier by retry_at, routine.due or next meal; a busy worker wakes every PULSE.
  correction: retry_at can only delay the idle wake, never pull it earlier: crates/app/src/world/farming/jobs/crew.rs:265 takes retry.max(now + 600 or PULSE). Only routine.due (crew.rs:266) and next_meal (crew.rs:267-268) pull it earlier, each floored at now+PULSE. A body with needs_tick() also forces now+PULSE (crew.rs:262). Units: fixed steps at 120 Hz, so 600 = 5 s.
- [imprecise] N13 (crates/app/src/world/adventure.rs:3242) claim: Actors farther than 90 m from the player are removed (their pose is checkpointed into the job state first).
  correction: The 90 m rule is right, but the compiled line is crates/app/src/world/adventure.rs:3240, not 3242: `multiplayer` is a default feature (crates/app/Cargo.toml:9). That line uses the local feet when no session exists and any session player's feet otherwise. Actors with e.habitat.is_some() are kept regardless of distance (adventure.rs:3246).
- [imprecise] N18 (crates/app/src/world/farming/jobs.rs:647) claim: The path queue breaks equal-priority ties by cost then cell coordinates, a total order.
  correction: The order is total, but the direction matters for reproduction. crates/app/src/world/farming/jobs.rs:647 pushes Reverse((priority, Reverse(cost), cell)), so among equal priority the entry with the higher accumulated cost pops first, then the smaller [x,y,z] array.
- [imprecise] N30 (crates/app/src/world/farming/jobs.rs:571) claim: Standability treats an unknown cell as not solid, so in path search an unknown fact silently becomes 'blocked' rather than suspending the operation.
  correction: The behaviour is right but the cited function is not the one A* uses. path_in_range calls navigation::Query::stand, whose footing is computed at crates/app/src/world/farming/jobs/navigation.rs:81-82 (floor.is_some_and(B::is_solid) and every body cell == Some(Air)), plus the step-edge test at jobs.rs:709. jobs.rs:571 `stand` is used by reachable() for Routine tasks, routine::home, recruit and the obstruction test at jobs.rs:904.
- [imprecise] R2 (crates/app/src/world/farming.rs:207) claim: The light/water/soil conditions applied to all overdue growth rolls are sampled once at evaluation time, so late evaluation can differ from on-time evaluation if conditions changed.
  correction: Not every overdue roll uses the sampled conditions. Irrigated crops discard overdue intervals down to one before sampling (crates/app/src/world/farming.rs:204-205). A zero chance consumes all due intervals without rolling (crates/app/src/world/farming/stage.rs:62-64). At most ROLLS_PER_SERVICE = 512 rolls are applied per service (farming.rs:28, 208).
- [imprecise] I2 (crates/app/src/world/farming/jobs.rs:739) claim: Input: voxel edits enter by reading the world each PULSE; build orders whose cell already holds the block are dropped, and mine orders are dropped on mismatch (line 736-738).
  correction: The prune at crates/app/src/world/farming/jobs.rs:736-739 runs only on the local crew's lists (world.farming.jobs), only at clock % PULSE == 0, and not at all while the quarter is paused (early return at jobs.rs:730-733). Community executors are swapped in only inside communities::with_site and never pass through this retain.
- [imprecise] H15 (crates/app/src/world/farming/jobs/routine/check.rs:112) claim: The daily-life check (routine/check.rs) reloads the sidecar mid-route, mid-rest and mid-cargo and requires identical RON; it needs a saved completed colony checkpoint.
  correction: The mid-cargo reload is attempted and recorded but not required to pass. crates/app/src/world/farming/jobs/routine/check.rs:141-142 requires route_reload, rest_reload and full_reload only; cargo_reload appears only in the report at line 147.
- [imprecise] H21 (crates/app/src/world/farming/jobs/quarter/frontier/colony/check.rs:25) claim: The colony check proves each helper is visited exactly once per PULSE with at most 3 workers per tick.
  correction: The check exercises crew::service_phase (crates/app/src/world/farming/jobs/quarter/frontier/colony/check.rs:22), which the runtime never calls. The runtime uses crew::phase_indices (crates/app/src/world/farming/jobs.rs:746). The two agree arithmetically (index i is served at phase floor((i+1)*30/64)), but no check asserts that agreement or the live visit order.
- [imprecise] S3 (crates/app/src/world/farming/jobs/routine.rs:43) claim: The home/rest routine needs a colony ledger (non-empty project_layers); the start-site scene sets project_layers empty, so only the colony fixture exercises housing as behaviour.
  correction: routine::home first accepts an entry in the executor's homes map (crates/app/src/world/farming/jobs/routine.rs:34), which community executors populate (communities/transit.rs:396 and 450), so the rest/home routine also runs for community residents with no quarter ledger. The start-site scene does set project_layers empty (quarter/frontier/scene.rs:111). Among fixtures only the colony reaches the routine, and its households start empty (colony/scene.rs:64), so rest/social begins only after completion (housing::check, colony/check.rs:124) or in the daily-life check.

## Missed (verifier)
- Council policy (survey, prospects, project planning) reads terrain only from chunks resident in RAM, not from versioned facts. Which cells a council 'knows' depends on what the camera or streaming has loaded when the council event fires. transit::contact_clear uses the same lookup (communities/transit.rs:495). [crates/app/src/world/farming/jobs/communities/development.rs:15] `w.resident_block(at)`
- Each agenda event has ordered side effects outside the borrowed executor: transit::policy after a council turn, transit::arrive after every person turn. If with_site returns None the already-cancelled wake is not rescheduled (unwrap_or_default at line 287). A person turn for an enlisted or dead resident returns no reschedule (line 265). A kernel reproducing agenda order must reproduce these. [crates/app/src/world/farming/jobs/communities.rs:289] `if person == COUNCIL { transit::policy(w, site); }`
- The crop scan cursor is not saved: sidecar::encode writes no cursor and decode starts from State::default(). After a load the 32-plots-per-step scan (farming.rs:27) restarts at the first key, changing the tick at which each plot is evaluated and so the clock-keyed gourd roll and the sampled conditions. [crates/app/src/world/farming/sidecar.rs:138] `let mut state = State { clock: r.u64()?, ..State::default() };`
- prepare_cells installs at most 2 chunks per call, in sorted chunk order; a third chunk that is already loaded in the pool still makes the call return false. The source pool is one instance swapped between all executors (communities.rs:229). Readiness therefore takes several wakes and depends on what other residents requested. [crates/app/src/world/farming/jobs/residency.rs:102] `if sources.inner.request(world, coord) && admitted < 2 {`
- Settlement-survey readiness can regress and requests can be dropped. The shared plan cache evicts its oldest entry at 256 (chunk generation threads fill the same cache), and the request uses try_send on a capacity-1 channel. For generators without landscape cycles, or without civilization, the function returns true synchronously (line 162). [crates/worldgen/src/civilization.rs:152] `if shared.len() >= 256 { shared.remove(0); }`
- Generated settlements require a planetary world. No Superflat fixture (livelihood, start site, colony, group) can register a community or put an event on the agenda, so agenda ordering can only be shadowed on a Planet world at or above the civilization generator version. This answers the reader's open question about Superflat. [crates/worldgen/src/civilization.rs:123] `cfg.civilization_enabled() && cfg.generator_version >= VERSION && planetary::active(cfg)`
- travel::step is short-circuited on any step where the express lift's step returns true. travel::step is the only caller of vehicle::service (travel.rs:314), which runs the 120-tick resident spawn, civilization::service, politics::service and communities::register. Lift state is therefore an input to whether those services run on a given tick. [crates/app/src/simulation/fixed.rs:68] `let riding = legacy && (world::orbital::express::step(world, player, input, move_player)`
- The recommended first target does not pre-admit terrain. production/check.rs has no admission call; only the checkpoint-based routine checks do (routine/check.rs:31, neighborhood.rs:47, replay.rs:8). Worker footprints in the livelihood check that fall outside the edited chunks go through the threaded source pool, so 'Awaiting terrain' ticks there may depend on wall time. Not run, so not measured. [crates/app/src/world/farming/jobs/routine/check.rs:31] `let admitted_chunks=materialize::admit(&mut w)?;`
- The scenario snapshot baseline is not clean in this tree: sand_falls has a pending insta .snap.new with hash 5c5e003c00af4e22, while the accepted .snap has 2a6d6444f0d05f49. The pending file may be stale; the test was not run. [crates/app/src/snapshots/minecraft_style_voxel_prototype__headless__tests__sand_falls.snap.new:6] `{"scenario":"sand_falls","ticks":510,"hash":"5c5e003c00af4e22","failed":0,"failures":[]}`
- The headless runner already has exact session save and continue (--ca-session-out / --ca-session-in through world::storage::session, plus --ca-checkpoint-in/out). It exists only on the CA-region path, where adventure::tick and farming are skipped (fixed.rs:59, 145), so it cannot replay workers as it stands. [crates/app/src/headless/options.rs:33] `session_in: value("--ca-session-in")?, session_out: value("--ca-session-out")?,`
- Further environment inputs reach live state: enabling housing growth takes its layout seed and profile from VOXEL_CITY_LAYOUT_SEED and VOXEL_CITY_LAYOUT (growth.rs:79, 89), overriding the world seed. VOXEL_CITY_COLONY, VOXEL_CITY_DISTRICTS and VOXEL_CITY_START_SITE select fixtures. [crates/app/src/world/farming/jobs/quarter/layout.rs:102] `std::env::var("VOXEL_CITY_LAYOUT_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(world_seed)`
- Resident draw sites not listed by the reader: the daily home/work shift of non-worker residents is hash(id) with no world seed, evaluated against farming.clock. Also the regional charter hash with no seed (travel/civilization.rs:87) and the quarter layout jitter keyed by seed, slot and a retry counter (jobs/quarter/layout.rs:124). [crates/app/src/world/adventure/wildlife/vehicle.rs:346] `let shift = u64::from(engine_core::hash::hash_u32(id)) % (day / 8);`

## Open questions
- No check, test or scenario was found that drives communities::service with a non-empty agenda (searched *test*/*check*/*review* files for communities, register, EVENTS_PER_TICK, oldest_delay). Which world type, seed and generator version produce the smallest generated settlement that reaches stage 2 was not determined; it is needed before the agenda ordering can be shadowed.
- Whether generated settlements exist on Superflat was not traced: civilization_enabled() is true without planet settings, but engine_worldgen::civilization::enabled(cfg) was not read.
- communities::with_site returns None when another executor is already borrowed, and the caller then schedules nothing after having cancelled the wake (communities.rs:224, 246, 287). Whether this re-entrant case is reachable from service, which would silently drop a resident from the agenda, was not established.
- Attached actors move through the general enemy update (adventure.rs, type Enemy) every tick. That update was not read, so it is unknown whether it uses HashMap iteration, dt-dependent f32 integration beyond STEP_SECONDS, or interactions with wild mobs that would make attached-worker motion differ run to run.
- Whether ron::to_string output for f32 fields (worker_pos, ResidentBody, Task::Haul positions) is bit-exact on round trip in every case was not verified; the checks compare RON text after one decode/encode, which shows text stability but not bit equality of the floats.
- The exact save directory contract for the checks: several require storage::save_directory() == <root>/disposable-saves. docs/WORK_CIRCLES.md says to set VOXEL_SAVE_DIR for the group check; storage.rs was not read to confirm this for the others.
- Who calls enable_housing_growth outside checks (UI or command) was not traced, so it is unknown whether the VOXEL_CITY_GROWTH environment read can occur during normal play.
- The multiplayer path (multiplayer.rs:2242 calls fixed::finish) and the meaning of shared_tick were not read; how farming.clock advances on host versus client is undetermined.
- The livelihood check calls fixed::gameplay without fixed::finish, while the colony and daily checks call both. Whether a shadow harness should include finish (rigid felled trees, drops settling) for the first target is a design choice the code does not settle.
- Founding (founding::plan, founding::advance), custody::exchange and transit::arrive were not read in full; their ordering and any further draw sites are not covered here.

## Confirmed findings
- U1 PULSE is 30 fixed steps (0.25 s at 120 Hz); every job timer written as n*PULSE is in fixed steps. [crates/app/src/world/farming/jobs.rs:31]
- U2 The fixed step is 1/120 s, defined in engine_core::time. [crates/core/src/time.rs:5]
- U3 farming.clock is a count of fixed steps, incremented once per farming::tick; it is the time base for every worker, agenda and travel timer. [crates/app/src/world/farming.rs:62]
- U4 The travel/civilization day is 57,600 fixed steps (480 s) and is derived from farming.clock, not from DayNightCycle. [crates/app/src/world/travel.rs:23]
- D1 The window driver runs at most 16 fixed steps per frame. [crates/app/src/simulation.rs:34]
- D2 Elapsed frame time above 16 steps is clamped before accumulation, so wall time is dropped rather than caught up. [crates/app/src/simulation.rs:52]
- D3 The window loop stops stepping after an 8 ms wall deadline (set at line 4924) and defers the remaining steps to the next frame; steps per frame are wall-clock dependent but each step is whole. [crates/app/src/main.rs:4928]
- D4 The window multiplies frame time by the livelihood scene speed (1/2/4/8) before computing steps; this is a UI-controlled input to tick rate, saved in production::State.speed. [crates/app/src/main.rs:3354]
- D5 In network mode the window loop runs zero local fixed steps; multiplayer drives its own path. [crates/app/src/main.rs:4926]
- D6 farming::tick is called from adventure::tick with an unbounded budget in every driver; the worker path never receives a wall-clock Budget. [crates/app/src/world/adventure.rs:3560]
- D7 jobs::service runs every fixed step right after farming::tick and takes only the player's feet position, no Budget. [crates/app/src/world/adventure.rs:3562]
- D8 adventure::tick (and so farming) runs only on the legacy path; with a CA region view it is skipped. [crates/app/src/simulation/fixed.rs:59]
- D9 A habitat review pause returns before farming runs, so the clock does not advance on that step. [crates/app/src/world/adventure.rs:3550]
- D10 Under multiplayer the farming block runs only when the session's shared_tick flag is set. [crates/app/src/world/adventure.rs:3554]
- D11 Headless passes Budget::UNBOUNDED to fixed::finish and settles every step. [crates/app/src/headless.rs:170]
- D12 The window passes one wall-clock budget per frame to fixed::finish (cellular, storms, industry); this does not reach farming or jobs. [crates/app/src/main.rs:5776]
- D13 The crop plot loop honours a Budget, but its only non-test caller passes UNBOUNDED, so the break never fires today. [crates/app/src/world/farming.rs:84]
- A1 The community agenda is a BTreeSet of (due tick, site id, person id); it is not serialized. [crates/app/src/world/farming/jobs/communities.rs:65]
- A2 At most 8 agenda events are processed per fixed step. [crates/app/src/world/farming/jobs/communities.rs:17]
- A3 Each iteration takes the smallest (due, site, person) entry that is due; equal-time ties break by site id then person id, and COUNCIL (u32::MAX-1) and FOUNDING (u32::MAX) sort last within a site. [crates/app/src/world/farming/jobs/communities.rs:245]
- A4 Lateness of a processed event is tracked as a running maximum in ticks; it is a session-only counter (serde skip). [crates/app/src/world/farming/jobs/communities.rs:248]
- A5 On load the agenda is rebuilt from each community's saved wakes map (plus next_plan for the council), so pending wake-ups survive save/load. [crates/app/src/world/farming/jobs/communities.rs:83]
- A6 Crew capacity is 64 per executor (one leader plus up to 63 helpers). [crates/app/src/world/farming/jobs/crew.rs:4]
- A7 Workers are disabled whenever a multiplayer session exists. [crates/app/src/world/farming/jobs.rs:233]
- A8 Base claims are likewise disabled under multiplayer. [crates/app/src/world/farming/base.rs:111]
- A9 The local crew runs outside the agenda: the leader at phase 0 of each PULSE, helpers by a contiguous roster slice per phase (so service order follows helper Vec index), then the community agenda. [crates/app/src/world/farming/jobs.rs:746]
- A11 After founding, residents are staggered one tick apart in roster order and the council by 120 + site%480 ticks; no hash is used. [crates/app/src/world/farming/jobs/communities.rs:254]
- N1 Worker terrain readiness polls an asynchronous source pool once per PULSE interval; results arrive from background threads. [crates/app/src/world/farming/jobs/residency.rs:94]
- N2 The source pool drains completed chunks with try_recv, so which tick a chunk becomes available depends on thread timing. [crates/app/src/simulation/residency.rs:183]
- N3 The poll loop stops after 500 microseconds of wall time, a second wall-clock dependency on how many chunks install per poll. [crates/app/src/simulation/residency.rs:250]
- N4 Chunk requests are refused while a 2-second wall-clock retry window is open. [crates/app/src/simulation/residency.rs:271]
- N5 The source pool thread count depends on the machine's core count (1 or 2). [crates/app/src/simulation/residency.rs:100]
- N6 Chunk installation is also gated by an edited-bytes memory limit, so readiness depends on what else is resident (camera, settings). [crates/app/src/simulation/residency.rs:436]
- N7 When terrain is not ready the worker's whole turn is suspended with status 'Awaiting terrain'; the duration in ticks therefore inherits N1 to N6. [crates/app/src/world/farming/jobs.rs:791]
- N8 Settlement plan readiness is produced by a background 'settlement-survey' thread; the caller gets false until it finishes. [crates/worldgen/src/civilization.rs:170]
- N9 Town transit policy (migration, exchange, trade, raids) is skipped on any council turn where that async survey is not ready. [crates/app/src/world/farming/jobs/communities/transit.rs:220]
- N10 The once-per-second regional civilization service is gated on the same async survey, centred on the player's feet. [crates/app/src/world/travel/civilization.rs:845]
- N11 Settlement ledgers are created when the player comes within range, stamped with the current day; regional history therefore starts when and where the player walks. [crates/app/src/world/travel/civilization.rs:860]
- N12 Resident actors are spawned only within 75 m of the player's feet (checked every 120 ticks) and only where terrain is loaded. [crates/app/src/world/adventure/wildlife/vehicle.rs:438]
- N14 A worker with no actor is moved by a separate call during its own service turn, using elapsed ticks capped at PULSE; an attached actor instead moves every tick in the enemy update. [crates/app/src/world/farming/jobs.rs:800]
- N15 The detached mover runs at most 30 substeps per call and grants no catch-up. [crates/app/src/world/adventure/wildlife/vehicle.rs:261]
- N16 Construction placement is refused while the player's body (f32 position) overlaps the target cell; any live actor overlapping it blocks too (line 175). [crates/app/src/world/farming/jobs/construction.rs:170]
- N17 The A* 'prev' map is a randomly seeded std HashMap but is only used for keyed get/insert/len; it is never iterated, so its order cannot reach a route. [crates/app/src/world/farming/jobs.rs:648]
- N19 The supply frame's HashMaps are accessed only by key (entry/get); no iteration. The frame is serde-skipped. [crates/app/src/world/farming/jobs/supply/frame.rs:11]
- N20 The static recipe HashMap is read only by key; candidate order inside each entry is authored recipe order. [crates/app/src/world/farming/jobs/communities/development.rs:111]
- N21 The walkability cache is thread-local and revalidated per cell by chunk version stamps; the code states it does not affect tie breaking. Its hit/miss counters do appear in diagnostics output. [crates/app/src/world/farming/jobs/navigation.rs:28]
- N22 Task selection sorts by (priority, group preference, f32 squared distance); the sort is stable and total, but the key is float geometry. [crates/app/src/world/farming/jobs.rs:1166]
- N23 The path heuristic converts an f32 distance to an integer cost. [crates/app/src/world/farming/jobs.rs:645]
- N24 Mining duration in pulses is computed from f32 material hardness. [crates/app/src/world/farming/jobs.rs:963]
- N25 One sort_unstable uses a non-unique key (chunk coordinate) followed by dedup by the same key; which cell survives is unspecified, but only its chunk coordinate is used afterwards. [crates/app/src/world/farming/jobs/residency.rs:89]
- N26 Route state is not saved (same for the leader in jobs.rs:82-83); after a reload the worker replans from its saved pose. [crates/app/src/world/farming/jobs/crew.rs:40]
- N27 The stall counter is not saved, so the 8-stall 'blocked route' threshold restarts after a reload. [crates/app/src/world/farming/jobs/crew.rs:47]
- N28 Saving writes a clone whose worker pose, body and motion_at are overwritten from the live actors at save time, so the saved state is not byte-equal to the live job state. [crates/app/src/world/farming/sidecar.rs:102]
- N29 'known' is None only for a chunk that is edited on disk but not resident; unedited unloaded cells are answered by world.get. [crates/app/src/world/farming.rs:102]
- N31 Crop plots are serviced only while their chunk is resident; growth is deferred and caught up on return. [crates/app/src/world/farming.rs:175]
- R1 Crop growth rolls are keyed by world seed, cell and interval index (plot.rolled), independent of evaluation time. [crates/app/src/world/farming/stage.rs:70]
- R3 The gourd side choice is keyed by farming.clock at the moment the plot is serviced; a retry on a later tick or a different service order rerolls it. [crates/app/src/world/farming/forage.rs:121]
- R4 Harvest yield is keyed by seed, cell and the plot's planting tick; a retry does not reroll it. [crates/app/src/world/farming.rs:464]
- R5 Resident personality (craft, belonging, autonomy) is one hash of world seed and resident id, drawn once when the person record is created. [crates/app/src/world/farming/jobs/groups/model.rs:27]
- R6 The town artisan is chosen by max of (experience, hash(id)%8) with no world seed; ties fall to the later entry in production.workers, i.e. roster order. [crates/app/src/world/farming/jobs/communities/development.rs:335]
- R7 The traveller for a non-raid mission is chosen by max of hash(id ^ target site), with no world seed, day or draw index; raids pick by grievance. [crates/app/src/world/farming/jobs/communities/transit.rs:181]
- R8 Abstract-ledger daily weather is hash(seed ^ site ^ day) % 100. [crates/app/src/world/travel/civilization.rs:471]
- R9 The daily war roll uses the same hash input as the weather roll (seed ^ site ^ day) with a different modulus; the two draws are not domain-separated. [crates/app/src/world/travel/civilization/politics.rs:560]
- R10 The first rest/meal deadline is staggered by id modulo 24, not by a hash. [crates/app/src/world/farming/jobs/routine.rs:79]
- R11 The wide path-search budget rotates on (clock/PULSE + worker id) mod 7. [crates/app/src/world/farming/jobs.rs:836]
- R12 Candidate rotation uses a saved per-worker counter; which task is tried first depends on prior failed attempts. [crates/app/src/world/farming/jobs.rs:1169]
- R13 Work-circle ids come from a saved serial counter, so id assignment depends on acceptance order. [crates/app/src/world/farming/jobs/groups/decision.rs:80]
- R14 The survey observer is picked by a counter that advances only when the async terrain request succeeds (line 163-164), so the observer sequence inherits terrain timing. [crates/app/src/world/farming/jobs/communities/development.rs:140]
- R15 Player fishing waits are keyed by the clock at cast time (player activity, not residents). [crates/app/src/world/travel/fishing.rs:69]
- I1 Input: player position enters as feet in metres computed in adventure::tick and is passed to jobs::service. [crates/app/src/world/adventure.rs:3552]
- I3 Input: assign/release a resident from the trade page UI. [crates/app/src/egui_ui/trade_page.rs:138]
- I4 Input: recruit constructors from the trade page UI. [crates/app/src/egui_ui/trade_page.rs:144]
- I5 Input: per-worker priorities from the trade page UI (clear_builds is at line 163, guard post toggle at 133, invite_to_base at 179). [crates/app/src/egui_ui/trade_page.rs:160]
- I6 Input: stockpile and mining designations from the window's interaction code. [crates/app/src/main.rs:5479]
- I7 Input: build orders queued from the creative blueprint tool. [crates/app/src/creative/blueprint.rs:193]
- I8 Input: travel and politics state (following, guard posts, mobilized) interrupts a worker's turn. [crates/app/src/world/farming/jobs.rs:755]
- I9 Input: actor death in the adventure simulation calls back into jobs. [crates/app/src/world/adventure/wildlife/vehicle.rs:471]
- I10 Input: campaign resolution in politics can also kill a worker. [crates/app/src/world/travel/civilization/politics.rs:432]
- I11 Input: player trades against a community's physical stock. [crates/app/src/world/travel.rs:213]
- I12 Input: the shared dropped-item list (player and world drops) is scanned for haul jobs by index window. [crates/app/src/world/farming/jobs.rs:1130]
- I13 Input: load replaces the whole farming state from the sidecar; communities.restore() rebuilds the agenda (line 262). [crates/app/src/world/farming/sidecar.rs:275]
- I14 Input: the housing-growth mode is read from an environment variable when growth is enabled. [crates/app/src/world/farming/jobs/quarter/frontier/colony/growth.rs:76]
- I15 Input: production speed is editable from the settlement panel and read by the window clock. [crates/app/src/world/farming/jobs/production/scene.rs:88]
- H1 The headless world hash covers resident chunk blocks, player pose, factory node coordinates and inventory only; farming, chests, actors and travel are not hashed. [crates/app/src/headless.rs:199]
- H2 Every scenario in assets/scenarios is run and compared to an insta snapshot by a normal test (just test scenarios_pass). The three scenarios (smoke, water_falls, sand_falls) contain no residents. [crates/app/src/headless.rs:521]
- H3 A test asserts that running the first scenario twice gives the same hash. [crates/app/src/headless.rs:870]
- H4 The farming sidecar is a binary file with magic VXFRM001 and tagged optional sections; it has no separate version field. [crates/app/src/world/farming/sidecar.rs:9]
- H5 The sidecar ends with an FNV-1a 64 checksum over all preceding bytes; this is the only existing digest of farming state. [crates/app/src/world/farming/sidecar.rs:13]
- H6 Community state is written as RON text under tag COM1 (jobs under JOB1); the serialized structs use BTreeMap and Vec only, so field and key order are fixed. [crates/app/src/world/farming/sidecar.rs:121]
- H7 --city-livelihood-check=<root> runs a fixed Superflat world, seed 424242, through fixed::gameplay only (no fixed::finish). [crates/app/src/world/farming/jobs/production/check.rs:19]
- H8 The livelihood check is dispatched from main by CLI flag before the window starts. [crates/app/src/main.rs:1814]
- H9 The livelihood check proves sidecar round trip with carried seed and carried harvest (RON text equality of jobs before and after). [crates/app/src/world/farming/jobs/production/check.rs:41]
- H10 The livelihood check proves meal conservation across chest, cargo and eaten (plus crop, timber and earthwork balances on lines 71-84). [crates/app/src/world/farming/jobs/production/check.rs:68]
- H11 The livelihood scene has exactly three residents. [crates/app/src/world/farming/jobs/production/scene.rs:28]
- H12 Those residents are placed in the local crew (assign / crew::add), so the scene never puts an event on the community agenda. [crates/app/src/world/farming/jobs/production/scene.rs:32]
- H13 A normal unit test replays work-circle policy: state is RON round-tripped, the same decisions are applied to both copies and compared. [crates/app/src/world/farming/jobs/groups/check/policy.rs:28]
- H14 That replay runs as test voluntary_group_policy_replays; the full group check is --city-group-check=<root> with VOXEL_SAVE_DIR=<root>/disposable-saves. [crates/app/src/world/farming/jobs/groups/check.rs:125]
- H16 The daily-life check is selected by env var inside the colony check, reached through --city-quarter-check with the start-site and colony env switches. [crates/app/src/world/farming/jobs/quarter/frontier/colony/check.rs:16]
- H17 routine/check/reload.rs does a full world save and load and compares jobs RON, travel sidecar bytes, roster poses, chest stock, geometry and food/timber balances. [crates/app/src/world/farming/jobs/routine/check/reload.rs:37]
- H18 neighborhood/replay.rs is not a decision replay: it loads a saved 64-resident checkpoint, steps up to 182 ticks and passes once actors rematerialize at their saved pose with valid commitments. [crates/app/src/world/farming/jobs/routine/check/neighborhood/replay.rs:33]
- H19 That replay is selected by VOXEL_INFILL_RELOAD_ONLY=1 inside the neighborhood check. [crates/app/src/world/farming/jobs/routine/check/neighborhood.rs:39]
- H20 supply/check/snapshot already implements a shadow comparison: a cached per-turn frame is compared with direct queries for membership, sources, stock, reservations and candidate lists. [crates/app/src/world/farming/jobs/supply/check/snapshot.rs:39]
- H22 The colony check proves timber conservation across logs, planks, chests, cargo and drops (food on lines 144-146), plus a player-obstruction interruption and cargo sidecar reload. [crates/app/src/world/farming/jobs/quarter/frontier/colony/check.rs:142]
- H23 The quarter check asserts payment conservation every step and does an interrupted-cargo sidecar round trip with a paused project. [crates/app/src/world/farming/jobs/quarter/check.rs:104]
- H24 The communities report exposes agenda counters (scheduled, due, processed, per-tick cap, oldest delay) as JSON through jobs::diagnostics. [crates/app/src/world/farming/jobs/communities.rs:439]
- H25 jobs/review.rs is a window-only save/resume exercise driven by VOXEL_JOBS_REVIEW (exercise, build-exercise, resume, build-resume); it requires benchmark mode and VOXEL_SAVE_DIR and uses a wall-clock timeout. [crates/app/src/world/farming/jobs/review.rs:100]
- S1 The start-site fixture (three projects, same three residents, obstruction and carried-cargo reloads, conservation every 120 steps) is the next step up; it is reached by --city-quarter-check with VOXEL_CITY_START_SITE=1. [crates/app/src/world/farming/jobs/quarter/frontier/check.rs:16]
- S2 The start-site switch is an environment variable. [crates/app/src/world/farming/jobs/quarter/frontier.rs:97]
- S4 The colony fixture's smallest roster is 12 residents (VOXEL_CITY_RESIDENTS), default 32. [crates/app/src/world/farming/jobs/quarter/frontier/colony/scene.rs:10]