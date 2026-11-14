# local_crew

## Summary
Read-only pass over the files on disk; nothing was built or run.

ENTRY AND UNITS. One tick is one fixed player step of 1/120 s. simulation::fixed::gameplay calls world::adventure::tick only when ca_view is none. That function, when shared_tick holds, calls farming::tick (the only place farming.clock is incremented, +1 per step) and then farming::jobs::service(world, feet), where feet = player.pos * voxel metres. PULSE = 30 ticks (0.25 s).

jobs::service order:
1. phase = clock % PULSE; return if !enabled (multiplayer present, civilization off, or ca_view set).
2. At phase 0, quarter::service.
3. If the quarter is paused, run only communities::service and return.
4. At phase 0, purge stale mine/build/claims on the local Jobs, service the lead worker, and promote a helper if the lead vanished.
5. crew::service for each helper index in phase_indices(phase).
6. communities::service (agenda, at most 8 events per tick).

LOCAL SCHEDULING. The lead is slot 0 and helper i is slot i+1; a slot runs at phase floor(slot*30/64). Every local worker is therefore serviced exactly once per 30 ticks at a phase fixed by its index in the helpers Vec, with at most 3 services per tick, whatever its state. retry_at only short-circuits inside work(). Agenda workers instead run when their (due, site, person) entry is popped, then get crew::next_wake:
- now+30 if the body needs ticking, or the worker has a task, path, carry, batch or mission, or is not in Work phase;
- otherwise max(retry_at, now + (30 if j.homes is empty, else 600 ticks));
- then lowered to routine.due and to the next meal when those are in the future, each floored at now+30.

SHARED EXECUTOR. crew::service swaps a helper's fields into the top-level Jobs fields (with its delivery chest and priorities overriding stockpile and priorities), runs service_worker, and swaps back. service_worker:
1. Suspends the worker if following, guarding or mobilized.
2. Looks up a live actor. If the actor has flash or alert it resets the task ("Seeking safety").
3. Takes feet from the actor, else from saved worker_pos.
4. Sets elapsed = min(clock - motion_at, 30).
5. Calls residency::prepare; false means "Awaiting terrain" and return.
6. With no actor, integrates the body through vehicle::advance_worker for at most 30 substeps toward path.front(); death or y <= -256 calls mark_dead and clear_worker.
7. Calls work(): mission walk, stockpile checks, claim reconcile, routine::service, groups::service, the retry gate, task_still_valid or select_task, then either walk/replan/stall handling or execute plus reset_task.

Work counters (mining, sow 4, harvest 6) count service calls, not ticks.

IDENTITY. Ids are u32, carried by the actor as site.x with site.z == 8. Constructors are 0x6000_0000..+10, spawned by crew::recruit near the stockpile with priorities [0,0,1]. assign() makes the first worker the lead; crew::add pushes helpers (at most 63). Removal is by assign() release, worker_fell (actor death), clear_worker (unembodied death), or take_worker (enlist, return home, transit). crew::view returning None means only "not in this Jobs". In communities::service, right after service_person, it is treated as death (died_at = now).

PLAYER INFLUENCE. Direct:
- construction::unoccupied tests the player's AABB against a build cell. It filters Build and Fetch candidates, invalidates held Fetch/Fund/Build tasks and blocks placement. The same value is passed to community workers.
- crew::recruit uses it for a 24 m range check and a 1.5 m spawn clearance.

Transitive:
- Actors exist only near the player (spawn within 75 m once per 120 ticks, despawn at 90 m). That switches the position source and integrator, and changes which live enemies block build cells.
- Chunks resident around the player skip the asynchronous source lane, so "Awaiting terrain" and known() results depend on streaming.

Presentation only: presentation()/WorkPose, status strings, markers, last_placement.

TASKS. Task has 11 variants: Routine, Produce(Seed/Sow/Harvest/Fell/Craft/Smelt/Eat), Store, Mine, Haul, Fetch, Supply, Recover, Fund, Buy, Build. Transitions happen in work()/select_task, routine::service, transit::walk and the execute arms. Selection sorts by (priority, group preference, f32 distance) and tries 4 candidates when helpers is empty, else 2.

## Summary corrections (verifier)
I read the cited code for all 96 findings; 90 hold as written and 6 are imprecise (A56, A63, A76, A81, A86, A96 is fine; the sixth is listed below), none wrong. Nothing was built or run.

The imprecise ones are A56, A63, A76, A81 and A86, plus the summary-level points below. (That is five findings, not six: 91 confirmed, 5 imprecise.)

What the summary gets wrong or overstates:

1. **"Serviced exactly once per 30 ticks ... whatever its state" is not an invariant.**
   - A paused quarter skips the whole local crew (jobs.rs:730-733).
   - The index range is computed once per tick (jobs.rs:746), but crew::service can Vec::remove a helper mid-loop (crew.rs:310). The next index then skips a helper for that pulse.
   - promote, remove and take_worker also shift indices and therefore phases. The reader left this as an open question while the summary states it as fact.

2. **"Actors exist only near the player (spawn ... once per 120 ticks)" has exceptions.**
   - crew::recruit spawns actors immediately (crew.rs:178).
   - vehicle::service is not reached on steps where orbital::express::step returns true, because of the `||` at fixed.rs:68-69.
   - Its cadence test reads the pre-increment clock and the pre-movement player feet (travel.rs:312).

3. **The tick-dependent path budget applies only to colony Jobs.** The every-7th-pulse "broad" flag changes path_limit only when homes is non-empty or frontier project layers exist (jobs.rs:832, 838). A plain player crew always gets 1100 nodes, or 4096 with a quarter.

4. **The player-influence list is incomplete.** It omits:
   - assign() and crew::add need a live embodied actor (jobs.rs:324, crew.rs:135);
   - the game mode's visible_to_mobs zeroes actor alert (adventure.rs:1885-1887);
   - the unembodied mover is gated by cave::loaded on world.chunks (wildlife.rs:185);
   - mining and production commits are gated by resident() (jobs.rs:967);
   - Residency::poll has a 500 µs wall-clock cut-off (simulation/residency.rs:250), which is not player-derived but is non-deterministic.

5. **Nothing is said about save/load.** path, stalled, last_walk and the agenda are `#[serde(skip)]` (jobs.rs:82-103, communities.rs:65). A reload replans every route and resets stall counters, which matters directly for the planned replay versus save/load comparison.

6. **The order of decisions and movement within a step is missing.** jobs::service decides first (adventure.rs:3562); embodied actors move afterwards in adventure.update (adventure.rs:3607), every step. Unembodied workers move only inside their own service.

7. **"retry_at only short-circuits inside work()" understates it.** It also gates routine travel (routine.rs:148) and mission walking (transit.rs:136), both called from work(), and it feeds next_wake (crew.rs:265).

8. **The work counters are incomplete.** Woodlot felling needs 12 services (production/woodlot.rs:83). j.work is zeroed on any service where the target is not reachable (jobs.rs:917).

9. **A Build refusal is not the end of the story.** After place() refuses, work() still calls reset_task (jobs.rs:995), so the worker drops the Build task and reselects while keeping its cargo. The occupancy test includes the builder's own embodied actor (construction.rs:175).

10. **The agenda description omits two things.** FOUNDING and COUNCIL pseudo-persons consume the same 8-event budget, and transit::policy/arrive runs after every event (communities.rs:289-290) and can move a worker between sites.

## Checks: 91 confirmed, 5 not

- [imprecise] A56 (crates/app/src/world/farming/jobs.rs:836) claim: The path search budget depends on the tick of service and the worker id: a 'broad' search every 7th pulse index.
  correction: The 'broad' flag (crates/app/src/world/farming/jobs.rs:836) changes the search budget only for colony Jobs: jobs.rs:838 `let path_limit = if colony&&!broad {768} else if j.quarter.is_some() { 4096 } else { 1100 };`. A Jobs with no homes and no frontier project layers always gets 1100 nodes (4096 with a quarter), whatever the tick or worker id. `colony` is defined at jobs.rs:832.
- [imprecise] A63 (crates/app/src/world/farming/jobs/construction.rs:200) claim: Placement is refused ('Build site occupied') when the player or a worker's saved position overlaps the cell: outcome-changing.
  correction: Refusal also fires when any live actor in world.adventure.enemies overlaps the cell (crates/app/src/world/farming/jobs/construction.rs:175), which includes the builder's own actor. place() returns normally after setting 'Build site occupied', so work() still reaches `j.reset_task();` at crates/app/src/world/farming/jobs.rs:995: the Build task is dropped, the carried stack is kept, and the worker reselects on its next service.
- [imprecise] A76 (crates/app/src/world/farming/jobs/crew.rs:131) claim: crew::add refuses when disabled, already assigned, 63 helpers exist, or (line 132) the villager is a guard.
  correction: The list of refusals is incomplete. crew::add also returns false when the villager has no live embodied actor (crates/app/src/world/farming/jobs/crew.rs:135 `let Some(pos) = vehicle::villager_feet(world, id) else { return false };`) and when communities::enlist returns Err (crew.rs:141). When enlist returns Ok(true) (crew.rs:137-139) it returns true without pushing here, because enlist already moved the resident with put_worker (communities.rs:310).
- [imprecise] A81 (crates/app/src/world/adventure/wildlife/vehicle.rs:385) claim: Resident actors are discovered and spawned only once per 120 ticks (1 s), in vehicle::service called with the player's feet.
  correction: Not the only spawn site. crew::recruit spawns a constructor actor immediately at crates/app/src/world/farming/jobs/crew.rs:178 `vehicle::spawn_resident(world, id, pos);`, outside the 120-tick cadence. The cadence test at vehicle.rs:385 also reads farming.clock before that step's increment, because travel::step (fixed.rs:69) runs before adventure::tick, where farming::tick increments the clock (adventure.rs:3560).
- [imprecise] A86 (crates/app/src/world/travel.rs:314) claim: vehicle::service is reached from travel::step, called from fixed::gameplay before adventure::tick (fixed.rs:69).
  correction: travel::step is not reached every step. crates/app/src/simulation/fixed.rs:68-69 reads `let riding = legacy && (world::orbital::express::step(world, player, input, move_player)` `|| world::travel::step(...))`. The `||` short-circuits, so vehicle::service (resident discovery and spawn) is skipped on any step where express::step returns true, and on every step when ca_view is set. The feet it receives are also the pre-movement player position (travel.rs:312), unlike the post-movement feet passed to jobs::service.

## Missed (verifier)
- Save/load cannot reproduce a continuous run of the executor. The worker's route is not saved (the `#[serde(skip)]` on line 82 covers `path`), and neither are last_walk, stalled, status, last_placement, hub_access, supply_frame and sources (jobs.rs:86-103; the same fields on crew::Worker, crew.rs:40-47). After a load every walking worker replans from an empty path with stall counters at 0. The community agenda is also skipped (communities.rs:65) and rebuilt from Community.wakes in restore() (communities.rs:78-87). [crates/app/src/world/farming/jobs.rs:83] `path: VecDeque<Vec3>,`
- Residency::poll, which residency::prepare_cells calls once per PULSE interval, stops draining completed source pages after 500 microseconds of wall-clock time. Which pages are accepted in a given pulse, and so which services return 'Awaiting terrain', depends on wall time. This is separate from the retry gate the reader cited at line 271; `retry` is also set from Instant::now() on decode errors (lines 231, 236), not only on disconnect. [crates/app/src/simulation/residency.rs:250] `if started.elapsed() >= Duration::from_micros(500) {`
- Within one step, jobs::service (adventure.rs:3562) runs before adventure.update, which moves embodied actors through wildlife::tick (adventure.rs:1945) and then despawns those beyond 90 m. An embodied worker therefore moves every 1/120 s step between services, steered by jobs::goal = path.front() (vehicle.rs:334). An unembodied worker moves only inside its own service, in at most 30 substeps. The two modes yield different positions at each decision point. [crates/app/src/world/adventure.rs:3607] `let consumed = adventure.update(world, player, dt, cast, cycle, melee);`
- The shared mover returns without moving whenever cave::loaded fails. cave::loaded requires world.chunks to hold the chunks at +/-2 m in x and z and -2/+3 m in y around the body (cave.rs:5-8). residency::prepare only requests a +/-0.5 m footprint (residency.rs:24). advance_worker goes through this same function, so an unembodied worker's movement depends on which chunks are in world.chunks. Not moving feeds the stall counter (jobs.rs:886-894). [crates/app/src/world/adventure/wildlife.rs:185] `if !cave::loaded(world,e.pos) { e.gait=0.0; return; }`
- Candidate ties are broken by insertion order, because sort_by is stable. Insertion order is: Build/Fetch/Fund in j.build order, Mine in j.mine order, Haul in drop-window order, then supply::candidates, then production::candidates (jobs.rs:1066-1139). The distance key for Fetch/Fund is source-chest-to-order distance, not worker distance (jobs.rs:1158). The priority key is production::work_priority, which returns 0 or 1 or normal+2 (production.rs:246-253), or u8::MAX for Store when prefer_delivery is set. It is not the raw priorities value. [crates/app/src/world/farming/jobs.rs:1163] `tasks.sort_by(|&a, &b| {`
- Assigning a worker needs a live embodied actor: assign() returns silently when villager_feet is None, and crew::add does the same (crew.rs:135). Actors exist only within the player-centred spawn/despawn radii, so crew membership itself depends on player proximity. The reader's player-influence list omits this. [crates/app/src/world/farming/jobs.rs:324] `let Some(feet) = vehicle::villager_feet(world, id) else {`
- The agenda holds more than person wakes. FOUNDING (u32::MAX) and COUNCIL (u32::MAX-1) pseudo-persons share the 8-events-per-tick budget (communities.rs:18-19, 250-264). After every event, outside the borrowed executor, transit::policy or transit::arrive runs and can move a worker between sites with take_worker/put_worker (transit.rs:386-400, 437-451) and reschedule it at clock+1. Founding seeds first wakes at now + index + 1 ticks (communities.rs:252-253). [crates/app/src/world/farming/jobs/communities.rs:290] `else if person < COUNCIL { transit::arrive(w, site, person); }`
- A worker's decisions read the other workers' live state in the same Jobs. crew::planned excludes tasks any other worker already holds; the lead's state is parked in helpers[index] during a helper's service (crew.rs:304), so it is visible too. Shared claims, workers_clear, the routine 'Home Occupied' test (routine.rs:104-110) and groups peers within 4 m (groups.rs:27) are read the same way. Service order within a pulse (lead, then helpers by Vec index; agenda by (due, site, person)) therefore decides who gets a contested task. [crates/app/src/world/farming/jobs/crew.rs:316] `j.helpers.iter().filter_map(|worker| worker.task).any(|other| match (task, other) {`
- motion_at is advanced before residency::prepare is tested, so ticks spent 'Awaiting terrain' are discarded, not carried forward. The following/guarding/mobilized early return (jobs.rs:755-764) leaves motion_at untouched, and the 30-tick cap then applies on resume. [crates/app/src/world/farming/jobs.rs:789] `world.farming.jobs.motion_at = world.farming.clock;`
- Mining commits only if the target chunk is resident in world.chunks at the service where the work counter completes. Selection and validation use known(), which also accepts unedited non-resident chunks. If the chunk is not resident the arm falls through to reset_task (jobs.rs:995) with no block removed and the designation kept. production::actions::execute has the same resident() gate (production/actions.rs:36). [crates/app/src/world/farming/jobs.rs:967] `if resident(world, p)`
- Haul tasks identify a dropped stack by exact f32 position equality in the global world.inventory.dropped pool, which is shared with the player and capped at 1024. Death, batch and mission cargo go to inventory.overflow once the pool is full (jobs.rs:481-497). Each select_task scans at most 256 drops through a per-worker rotating cursor (drop_scan.rs:2). [crates/app/src/world/farming/jobs.rs:948] `.position(|d| d.pos == pos && d.stack.count != 0)`
- When the game mode makes the player invisible to mobs (world.rules.mode.visible_to_mobs() false, adventure.rs:1866), every embodied actor's alert is zeroed each step, villagers included. That changes how long an embodied worker stays in 'Seeking safety' (jobs.rs:769). An unembodied body never passes through this code. This is a player-mode influence on task outcomes that the reader did not list. [crates/app/src/world/adventure.rs:1887] `e.alert = 0.0;`

## Open questions
- Index shift: Vec::remove in crew::service (crew.rs:310), crew::promote, crew::remove and take_worker shift later helpers to lower indices, hence different phases. From the code this can make a helper skip a service or be serviced twice in one pulse; I did not confirm it with a run, and no test covering it was found.
- The local phase-0 purge of completed build orders (jobs.rs:736-740) operates on world.farming.jobs only. For community Jobs I found only a mine.retain in communities/development.rs:391 and no equivalent build.retain outside check/test files. Whether development::plan cleans stale build orders another way was not read.
- trade_page.rs passes `eye` to crew::recruit while the tick path passes feet in metres. The unit and origin of `eye` in egui_ui::trade_page::draw were not traced.
- Not read in full: quarter::service, routine/social.rs, groups/model.rs and decision.rs, market.rs, supply/transport.rs and frame.rs, production/batch.rs, woodlot.rs and smelting.rs, communities/development.rs, founding.rs and transit.rs past line 250. Task transitions inside those are described only from their call sites.
- Embodied actors move every tick through adventure.rs:1945 with a shared resident_navigation_budget of 1 per tick; advance_worker uses a budget of 0. navigation::heading was not read, so the effect of a zero budget on an unembodied worker with an empty path is undetermined (with a job path, wildlife::tick bypasses heading).
- simulation/residency.rs uses worker threads and Instant::now() (line 271, the retry gate after a channel disconnect). Whether headless runs see 'Awaiting terrain' on run-dependent ticks because of thread completion order was not determined.
- In multiplayer, farming::tick still advances the clock under shared_tick while jobs::enabled is false. What happens to saved Jobs state and the agenda when a world moves between solo and hosted sessions was not traced.
- Whether any scenario in assets/scenarios runs with civilization_enabled() true and a populated crew (needed for the shadow comparison) was not checked.
- Many executor decisions use f32: distance sort keys, in_reach/arrived thresholds, hardness-derived mining pulses, path costs. Which of these the 'integer routine policy' extraction must reproduce bit-for-bit, versus replace with an explicit new rule, is a design decision this pass does not answer.

## Confirmed findings
- A1 PULSE is 30 ticks (0.25 s at 120 Hz), derived from PLAYER_STEP_SECONDS with rounding. [crates/app/src/world/farming/jobs.rs:31]
- A2 The tick unit is one fixed player step of 1/120 s. [crates/core/src/time.rs:5]
- A3 farming.clock is a u64 tick counter incremented once per farming::tick, before the ca_view early return; no other increment site was found outside tests. [crates/app/src/world/farming.rs:62]
- A4 adventure::tick calls farming::tick with an unbounded budget inside the shared_tick block; this is the only non-test caller found. [crates/app/src/world/adventure.rs:3560]
- A5 jobs::service is called right after farming::tick and vehicle::sync_driver, receiving the player's feet in metres as its 'player' argument. [crates/app/src/world/adventure.rs:3562]
- A6 The 'player' value passed to the worker executor is player.pos scaled to metres (feet, not eye). [crates/app/src/world/adventure.rs:3552]
- A7 The fixed-step driver calls adventure::tick only when 'legacy' (ca_view is none); both headless.rs:164 and main.rs:5012 go through fixed::gameplay. [crates/app/src/simulation/fixed.rs:146]
- A8 A habitats review pause returns from adventure::tick before the farming block, so neither the clock nor jobs advance that step. [crates/app/src/world/adventure.rs:3550]
- A9 jobs::service computes the pulse phase from the clock before checking enabled(). [crates/app/src/world/farming/jobs.rs:727]
- A10 Workers are disabled when a multiplayer context exists (feature-gated). [crates/app/src/world/farming/jobs.rs:233]
- A11 enabled() also requires civilization_enabled() and no CA region view. [crates/app/src/world/farming/jobs.rs:236]
- A12 farming::base::enabled is likewise false in multiplayer. [crates/app/src/world/farming/base.rs:111]
- A13 A paused quarter skips the whole local crew and runs only the community agenda that tick. [crates/app/src/world/farming/jobs.rs:730]
- A14 At phase 0 the local Jobs purges build orders whose cell already holds the ordered block (mine and claims are purged on lines 736-740); unknown cells are kept. [crates/app/src/world/farming/jobs.rs:739]
- A15 The lead worker (State.worker) is serviced only at phase 0, once per PULSE. [crates/app/src/world/farming/jobs.rs:742]
- A16 Every tick a contiguous slice of helper indices for that phase is serviced; the range is computed once from the current helpers length. [crates/app/src/world/farming/jobs.rs:746]
- A17 The community agenda runs after the local crew on every tick, with the same player value. [crates/app/src/world/farming/jobs.rs:749]
- A18 Crew capacity is 64 (lead + 63 helpers; MAX_HELPERS = CAPACITY - 1 on line 5). [crates/app/src/world/farming/jobs/crew.rs:4]
- A19 Helper index i has phase floor((i+1)*PULSE/CAPACITY) = floor((i+1)*30/64). This function is used only by a check file; the runtime uses phase_indices. [crates/app/src/world/farming/jobs/crew.rs:9]
- A20 phase_indices inverts that: first slot = max(ceil(phase*64/30), 1), end slot = ceil((phase+1)*64/30). Slot 0 is reserved for the lead. [crates/app/src/world/farming/jobs/crew.rs:12]
- A21 Slots map to helper indices by subtracting 1, clamped to the roster, so 2 or 3 helpers run per tick and small rosters bunch in the early phases. [crates/app/src/world/farming/jobs/crew.rs:14]
- A22 A check asserts each of the 63 helpers is visited exactly once per pulse and that at most 3 workers run per tick. [crates/app/src/world/farming/jobs/quarter/frontier/colony/check.rs:25]
- A23 crew::service runs a helper by swapping its per-worker fields into the top-level Jobs fields; the previous top-level worker state is parked in helpers[index] during the call. [crates/app/src/world/farming/jobs/crew.rs:302]
- A24 During a helper's service its delivery chest temporarily replaces the shared stockpile (priorities likewise on line 301); both are restored afterwards. [crates/app/src/world/farming/jobs/crew.rs:300]
- A25 A helper whose worker id became None during service is removed with Vec::remove, shifting later helpers' indices and therefore their phases. [crates/app/src/world/farming/jobs/crew.rs:310]
- A26 crew::service_person (agenda path) dispatches to the same service_worker for the lead, or crew::service for a helper index; there is no separate executor. [crates/app/src/world/farming/jobs/crew.rs:248]
- A27 The agenda pops the earliest due (due, site, person) entry, up to EVENTS_PER_TICK = 8 per tick (constant on line 17). [crates/app/src/world/farming/jobs/communities.rs:245]
- A28 After service_person the agenda reschedules the person at crew::next_wake. [crates/app/src/world/farming/jobs/communities.rs:286]
- A29 The community executor works by swapping the site's Jobs into world.farming.jobs (the local crew is parked in communities.borrowed). [crates/app/src/world/farming/jobs/communities.rs:228]
- A30 next_wake: the earliest possible wake is now + PULSE (30 ticks); a missing view also returns this. [crates/app/src/world/farming/jobs/crew.rs:256]
- A31 next_wake: a body with velocity, status, affixes, alert/flash, slow or stagger forces the earliest wake. [crates/app/src/world/farming/jobs/crew.rs:262]
- A32 next_wake: any task, path, carry, batch, mission, or non-Work routine phase (line 264) forces the earliest wake. [crates/app/src/world/farming/jobs/crew.rs:263]
- A33 next_wake idle rule: max(retry_at, now + PULSE if the Jobs has no homes, else now + 600 ticks = 5 s). [crates/app/src/world/farming/jobs/crew.rs:265]
- A34 next_wake: a future routine.due pulls the wake earlier, floored at now + PULSE. [crates/app/src/world/farming/jobs/crew.rs:266]
- A35 next_wake: a future next_meal for this id also pulls the wake earlier, floored at now + PULSE. [crates/app/src/world/farming/jobs/crew.rs:268]
- A36 service_worker suspends a worker who is following, guarding or mobilized (interrupt routine, release claims, reset task; status only). [crates/app/src/world/farming/jobs.rs:755]
- A37 service_worker looks for a live physical actor (type Enemy in world.adventure.enemies) for the worker id. [crates/app/src/world/farming/jobs.rs:766]
- A38 An embodied actor with flash > 0 or alert > 0 makes the worker drop its task and claims ('Seeking safety') for that service. [crates/app/src/world/farming/jobs.rs:769]
- A39 Worker position is the actor's position when embodied, else the saved worker_pos; neither present gives 'Awaiting resident'. [crates/app/src/world/farming/jobs.rs:784]
- A40 Elapsed time since the last service is capped at PULSE (30 ticks), so long agenda sleeps never grant catch-up movement. [crates/app/src/world/farming/jobs.rs:788]
- A41 If required terrain chunks are not resident the service returns 'Awaiting terrain' without running work(); accepted task state is kept. [crates/app/src/world/farming/jobs.rs:791]
- A42 Only when no actor exists is the body integrated inside the job service, toward path.front() (or its own feet). [crates/app/src/world/farming/jobs.rs:800]
- A43 Unembodied death (health <= 0 or below y -256) triggers communities::mark_dead, clear_worker (drops cargo/mission/batch, worker = None) and a travel.away entry. [crates/app/src/world/farming/jobs.rs:802]
- A44 advance_worker runs at most 30 substeps of 1/120 s through the same wildlife::tick mover. [crates/app/src/world/adventure/wildlife/vehicle.rs:261]
- A45 The unembodied mover passes a fake player position 1000 m away and time 0.0, so the real player position never enters it. [crates/app/src/world/adventure/wildlife/vehicle.rs:270]
- A46 Embodied villagers are not alerted by player proximity (the danger term excludes villager kinds); only flash > 0 from damage alerts them. [crates/app/src/world/adventure/wildlife.rs:191]
- A47 work() gives an active inter-settlement mission full ownership of the service before any task logic. [crates/app/src/world/farming/jobs.rs:829]
- A48 routine::service (meal/rest/social) runs before task selection and owns the worker whenever it returns true. [crates/app/src/world/farming/jobs.rs:854]
- A49 retry_at only gates an idle worker (no task) inside work(), after routine and groups have already run. [crates/app/src/world/farming/jobs.rs:857]
- A50 A held task is revalidated every service; failure triggers select_task. The player value is threaded into both. [crates/app/src/world/farming/jobs.rs:864]
- A51 Eight consecutive stalled services while walking release claims, reset the task and set a retry. [crates/app/src/world/farming/jobs.rs:894]
- A52 The blocked-route retry delay is 2 * PULSE = 60 ticks. [crates/app/src/world/farming/jobs.rs:900]
- A53 When select_task finds nothing, the retry delay is 8 PULSE (240 ticks) with no candidates, or 2 PULSE (60 ticks) with unreachable candidates; applied on line 1214. [crates/app/src/world/farming/jobs.rs:1213]
- A54 Candidate ordering key is (priority u8, not-group-preferred bool, f32 squared distance); the sort on lines 1163-1167 uses total_cmp on the float. [crates/app/src/world/farming/jobs.rs:1161]
- A55 select_task tries 4 candidates when the helpers Vec is empty, else 2, starting from the rotating next_candidate cursor. [crates/app/src/world/farming/jobs.rs:1170]
- A57 Mining duration is counted in service calls (j.work += 1 per service), from f32 hardness: ceil(3 + 4*hardness) clamped to 3..60. [crates/app/src/world/farming/jobs.rs:963]
- A58 After an execute arm completes (unless it returned early for work in progress), the task is always reset. [crates/app/src/world/farming/jobs.rs:995]
- A59 The Task enum starts on line 107 with 11 variants: Routine, Produce, Store, Mine, Haul, Fetch, Supply, Recover, Fund, Buy, Build (lines 108-118). [crates/app/src/world/farming/jobs.rs:107]
- A60 production::Task has 7 variants nested under Task::Produce. [crates/app/src/world/farming/jobs/production.rs:116]
- A61 Direct player use: the player's AABB (feet +/-0.4 m, 1.8 m tall) intersecting a build cell makes the order 'occupied'. [crates/app/src/world/farming/jobs/construction.rs:170]
- A62 unoccupied also tests every live actor in world.adventure.enemies, so which actors are embodied (player-proximity dependent) changes build outcomes. [crates/app/src/world/farming/jobs/construction.rs:175]
- A64 Player occupancy filters Build candidates in select_task: changes task choice. [crates/app/src/world/farming/jobs.rs:1080]
- A65 Player occupancy also filters Fetch/Fund candidates for an order. [crates/app/src/world/farming/jobs.rs:1110]
- A66 Player occupancy invalidates an already held Fetch/Fund task (and Build on line 1049) in task_still_valid. [crates/app/src/world/farming/jobs.rs:1037]
- A67 workers_clear uses saved crew positions (View.pos), so crew occupancy does not depend on embodiment. [crates/app/src/world/farming/jobs/construction.rs:185]
- A68 crew::recruit rejects when the stockpile is more than RANGE (24 m) from the passed player point. [crates/app/src/world/farming/jobs/crew.rs:162]
- A69 Recruit spawn spots must be more than 1.5 m horizontally from the player and at least 0.95 m from live actors: the spawn position is player-dependent. [crates/app/src/world/farming/jobs/crew.rs:175]
- A70 The UI passes its 'eye' value, not feet, to recruit. [crates/app/src/egui_ui/trade_page.rs:144]
- A71 Recruit iterates the 10 fixed constructor ids (CONSTRUCTORS = 10 on line 6), spawning a resident actor and a travel home when none exists. [crates/app/src/world/farming/jobs/crew.rs:168]
- A72 Constructor ids start at 0x6000_0000; is_constructor tests that range (line 18). [crates/app/src/world/farming/jobs/crew.rs:7]
- A73 Recruited constructors get priorities [haul 0, mine 0, build 1], i.e. build-only (the lead here; helpers on line 186). [crates/app/src/world/farming/jobs/crew.rs:184]
- A74 A worker's id is the actor's site.x when the kind is a villager and site.z == VILLAGER_Z (8). [crates/app/src/world/adventure/wildlife/vehicle.rs:193]
- A75 assign(): the first assigned villager becomes the lead (State.worker); later ones go through crew::add (line 337). [crates/app/src/world/farming/jobs.rs:344]
- A77 Dismissal via assign() on an assigned id clears the lead (then promotes) or removes the helper; it is refused while carrying, shopping or on a mission. [crates/app/src/world/farming/jobs.rs:307]
- A78 promote picks the first helper without a delivery chest as the new lead. [crates/app/src/world/farming/jobs/crew.rs:211]
- A79 worker_fell (actor death): if the id is not in the local Jobs it is routed to communities::fell; view None here means 'not a local crew member'. [crates/app/src/world/farming/jobs.rs:453]
- A80 In the agenda path, view None immediately after service_person is recorded as death at the current tick. [crates/app/src/world/farming/jobs/communities.rs:282]
- A82 An actor is spawned only if its saved or home position is within SPAWN_METRES (75 m, line 22) of the player and the terrain there is loaded. [crates/app/src/world/adventure/wildlife/vehicle.rs:438]
- A83 Actors 90 m or more from the player are dropped from the enemies list (single-player build). [crates/app/src/world/adventure.rs:3242]
- A84 On despawn a living worker's position and body are checkpointed into its Jobs. [crates/app/src/world/adventure.rs:3244]
- A85 The checkpoint sets motion_at to the current clock, so unembodied elapsed time starts at the despawn tick. [crates/app/src/world/farming/jobs/crew.rs:404]
- A87 Worker terrain readiness skips chunks already resident in world.chunks, so chunks streamed around the player are ready immediately. [crates/app/src/world/farming/jobs/residency.rs:101]
- A88 The asynchronous source pool is polled at most once per clock/PULSE interval, shared across local and community executors. [crates/app/src/world/farming/jobs/residency.rs:93]
- A89 The source lane's request path reads wall-clock time for its retry gate. [crates/app/src/simulation/residency.rs:271]
- A90 known() returns a block only if the chunk is resident or has no edit record; otherwise None (unknown). [crates/app/src/world/farming.rs:102]
- A91 Actor walk speed doubles for Build/Fetch/Fund/Buy tasks with a non-empty path: movement outcome, not player-derived. [crates/app/src/world/farming/jobs.rs:523]
- A92 presentation() derives the mining animation phase from clock % PULSE: presentation only. [crates/app/src/world/farming/jobs.rs:551]
- A93 Routine constants are in PULSE multiples: WORK_TICKS = 240*PULSE = 7200 ticks (60 s). REST_TICKS = 16*PULSE, SOCIAL_TICKS = 12*PULSE, TIMEOUT = 960*PULSE on lines 9, 10, 12. [crates/app/src/world/farming/jobs/routine.rs:11]
- A94 The first routine deadline is staggered by id: now + 60 PULSE + (id % 24) * 8 PULSE. [crates/app/src/world/farming/jobs/routine.rs:79]
- A95 A game day is 480*120 = 57,600 ticks (480 s); travel::day = clock / DAY_TICKS is used for the 'away' marker. [crates/app/src/world/travel.rs:23]
- A96 service_worker hands the whole Jobs (taken out of the world) to work() together with the player value. [crates/app/src/world/farming/jobs.rs:823]