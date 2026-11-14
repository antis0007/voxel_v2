# agenda

## Summary
Units: every time value here is `World.farming.clock`, a u64 count of fixed steps at 120 Hz (`PLAYER_STEP_SECONDS = 1/120`). `farming::tick` adds 1 per step, called from `adventure::tick` only on the legacy path with `shared_tick`. PULSE = 30 ticks (0.25 s), `development::INTERVAL` = 600 ticks (5 s), DAY_TICKS = 57,600.

Agenda: `BTreeSet<(due, site, person)>`, ascending tuple order. Resident ids are below 2^28+2^21, so at equal (due, site) residents sort by id, then COUNCIL (u32::MAX-1), then FOUNDING (u32::MAX). FOUNDING and COUNCIL never coexist for a site: stage<2 has only the FOUNDING wake, COUNCIL exists only at stage 2.

Invariant: agenda = {(wakes[p], site, p)} plus {(next_plan, site, COUNCIL) if stage==2}. `wakes`, `next_plan`, `stage` are saved; `owners`, `agenda`, `indexed`, `borrowed`, `spare`, `events`, `oldest_delay` are `#[serde(skip)]`. `restore()` rebuilds owners and agenda from exactly those saved fields, so the agenda round-trips. Not identical after load:
- `events` and `oldest_delay` reset to 0 (report-only).
- Per-worker `path`, `status`, `last_walk`, `stalled`, `last_placement` are skipped, and `next_wake` reads `path`.
- The save is a clone checkpointed against live actors (`worker_pos`, `body`, `motion_at = now`); the live state is not updated.
- The source pool clones to empty.

`service()`: up to 8 iterations per tick, each taking `agenda.first()` if due <= now. No event is dropped; the remainder waits, oldest due first, lower site id winning ties. `oldest_delay` is a session-lifetime max of now-due. Next wakes are computed from processing time `now`, not `due`, so backlog delay shifts all later wakes permanently.
- FOUNDING failure: now+120.
- FOUNDING success: each crew id at now+i+1 in roster order, COUNCIL at now+120+site%480.
- COUNCIL: now+600.
- Resident: `crew::next_wake` gives now+30 when busy (task, path, carry, batch, mission, non-Work phase, moving body); otherwise max(retry_at, now+600, or now+30 with no homes), lowered to routine.due or the next meal, never below now+30.
- Dead, enlisted or absent residents get no reschedule.

`with_site()` removes the Community from `sites`, swaps its Jobs into `w.farming.jobs`, parks the player's Jobs in `borrowed`, and moves the player's `sources` pool into the active executor. All sites and the local crew therefore share one terrain-admission pool: polled once per PULSE interval, 2 installs per call, 8 pending, LRU eviction, worker threads and `Instant`. During the closure the site is absent from `sites` and `city.jobs` is the empty spare. Nested calls return None.

Order dependence: events mutate shared world state immediately (chests, voxels, plots, `travel`, dropped items). Council and custody checks read only resident chunks (`world.chunks`), so results depend on what the player or the pool has loaded. The player position is passed into the executor.

After the swap: `transit::policy` (COUNCIL) can debit the source chest and start one mission, waking the traveller at clock+1. `transit::arrive` (resident) applies exchange, migration (person and worker move between sites, target wake clock+1), settlement (new Community, FOUNDING at clock+1), raid and scuffle damage synchronously in the same iteration.

Call order per fixed step: `travel::step` → `vehicle::service` → every 120 ticks `civilization::service` → `register` (before the clock increment); later `adventure::tick` → `farming::tick` (clock+1) → `jobs::service` → quarter, local main worker at phase 0, helper slice, then `communities::service` last.

`register()` is player-driven: ledgers are created for settlements within 110 m (550 m when discovering) of the player's feet, at most one community per second, lowest id first, FOUNDING at the current clock. The initial roster reads the local crew, `travel.away` and live actor positions. Observation changes history. Everything is disabled when `world.multiplayer.is_some()` or `ca_view` is set.

## Summary corrections (verifier)
85 of the 87 findings hold as written, A67 is imprecise only in claiming fixed::gameplay is the sole caller of adventure::tick, and the two conclusions that fail are A18 and A46. Checked by reading the cited functions on disk; nothing was built or run.

What is wrong or misleading in the reader's summary:

1. **Clock source.** "Called from adventure::tick only on the legacy path" is wrong. The multiplayer host path (multiplayer/authority.rs:81, driven from multiplayer.rs:2204-2231) also advances farming.clock once per step, while jobs::enabled keeps the agenda frozen. Clock time is therefore not agenda-active time. adventure::tick also returns before farming::tick when habitats::review::paused() is true (adventure.rs:3550). The clock is saved in the sidecar (sidecar.rs:49) and is separate from the session-local TickId (core/src/time.rs:1-3).

2. **Terrain reads.** "Council and custody checks read only resident chunks" has one exception: the council's hearth order uses Order::ready, which falls back to generated terrain (development.rs:347, construction.rs:65, farming.rs:102). The summary also omits that resident events use that fallback throughout, so "unknown" has two definitions today.

3. **Save/load.** "next_wake reads path" overstates the divergence. `task` is saved and accompanies every path, so the next computed wake is unchanged by the loss of `path` alone. The real differences after a load are:
   - routes are replanned and the stall counters restart;
   - the source pool is empty;
   - actors are missing until vehicle::service respawns them within 75 m of the player, so events run headless in the meantime.

4. **Observer effect.** "The player position is passed into the executor" understates it. The larger effect is actor attachment: spawn within 75 m and despawn beyond 90 m switches every resident event between actor.pos and the headless mover, which is capped at 30 ticks of movement per wake. The player's body, live actors, travel.following, guard posts and mobilisation are also read inside events.

5. **Disable conditions.** "Everything is disabled when multiplayer or ca_view" is incomplete.
   - The multiplayer check exists only under cfg(feature = "multiplayer") (jobs.rs:232).
   - jobs::enabled also needs civilization_enabled().
   - Registration additionally needs WorldType::Planet and generator_version >= geo::VERSION (civilization.rs:307-309).

6. **Registration cadence.** "At most one community per second, lowest id first" omits two conditions:
   - nothing registers unless the async 110 m survey at the player's current position is cached (civilization.rs:845);
   - only the lowest-id pending ledger is ever tried (line 895).

7. **oldest_delay.** Every FOUNDING scheduled by register is due at the pre-increment clock and is processed at clock+1 or later. oldest_delay is therefore at least 1 after any registration and is not a backlog signal by itself.

8. **Lost wake (A36).** The None path from with_site is not reachable from service() as written, because sites are never removed and service is not nested. If it ever fired for a stage-2 worker or for FOUNDING, State::valid would reject the next save (communities.rs:115-116), and sidecar::load would then discard the whole farming file (sidecar.rs:280).

9. **Not stated in the summary:**
   - No event can run twice in one tick. Every reschedule is at least now+1; only register inserts at the current clock, and that happens before the increment.
   - schedule() is a silent no-op for the site currently removed by with_site (communities.rs:129). That is why service() collects the reschedules and applies them after the swap is undone.

## Checks: 84 confirmed, 3 not

- [imprecise] A18 (crates/app/src/world/farming/jobs/crew.rs:263) claim: next_wake depends on the unsaved path (and task/carry/batch/mission/phase): any of these forces the earliest wake now+PULSE, so a reload can change wake times.
  correction: The quoted condition is right (crates/app/src/world/farming/jobs/crew.rs:263), but the conclusion that losing `path` on reload changes wake times is not supported. `task` is saved (crew.rs:39 `#[serde(default)] task: Option<Task>,`) and every site that fills `path` also sets `task` or runs under a saved mission (routine.rs:152, transit.rs:152, jobs.rs:908), so next_wake still returns now+PULSE after a load. What a reload does change is that the route is replanned and `stalled`/`last_walk` restart (jobs.rs:907-908). A wake already stored in `wakes` is saved unchanged.
- [imprecise] A46 (crates/app/src/world/farming/jobs/communities/development.rs:15) claim: Council policy reads terrain only through resident_block, i.e. only chunks currently in RAM; unknown cells yield None.
  correction: Not "only". development::plan also calls `order.ready(w)` for the hearth (crates/app/src/world/farming/jobs/communities/development.rs:347), and Order::ready uses the jobs-level `known` (crates/app/src/world/farming/jobs/construction.rs:65 `&& known(world, p) == Some(self.expected())`). That is farming::known, which samples generated terrain synchronously for any non-resident chunk that has no edit record (crates/app/src/world/farming.rs:102). Every other terrain read in development.rs and roads.rs does go through resident_block.
- [imprecise] A67 (crates/app/src/simulation/fixed.rs:145) claim: adventure::tick runs only on the legacy path (ca_view none) of fixed::gameplay, after the player update.
  correction: fixed::gameplay is not the only caller. The multiplayer host calls adventure::tick once per player from crates/app/src/multiplayer/authority.rs:81 (`let consumed=world::adventure::tick(w,p,dt,c.cast,c.cycle,c.mine);`), and travel::step from authority.rs:62. On that path jobs::enabled is false, so the agenda does not run, but farming::tick still advances the clock for the one actor that has shared_tick (crates/app/src/multiplayer.rs:2204 and 2221). adventure::tick also returns before farming::tick when habitats::review::paused() is true (crates/app/src/world/adventure.rs:3550).

## Missed (verifier)
- Whether a resident event runs against a physical actor or the headless mover is decided by player proximity. Villager actors are spawned once per second only when their saved position is within SPAWN_METRES = 75 m of the player's feet (vehicle.rs:22) and the cave check passes. They are removed beyond 90 m (crates/app/src/world/adventure.rs:3242-3246), with remember_worker writing the pose back. service_worker then reads actor.pos, which the per-step actor update moves later in adventure::tick, instead of advance_worker. Every resident event near the player therefore has different inputs from the same event far away. [crates/app/src/world/adventure/wildlife/vehicle.rs:438] `if home.distance_squared(feet) <= SPAWN_METRES * SPAWN_METRES && cave::loaded(w, home)`
- There are two definitions of an unknown cell. Worker executor code (stand, work, Order::ready, transit::settle) uses farming::known, which returns a value for any non-resident chunk with no edit record by sampling generation synchronously through world.get. It is None only for a chunk that is edited and unloaded. Council, roads and raid line-of-contact use resident_block, which is None for anything not in world.chunks. A kernel `Fact::Unknown` has to reproduce both rules to match the current executor. [crates/app/src/world/farming.rs:102] `(world.chunks.contains_key(&c) || !world.regions.view.records.contains_key(&c)).then(|| world.get(p))`
- Headless movement per wake is capped at PULSE = 30 ticks regardless of how long the resident slept or how late the agenda ran. advance_worker also loops `ticks.min(30)` substeps of 1/120 s (crates/app/src/world/adventure/wildlife/vehicle.rs:261). A resident woken after a 600-tick idle, or after agenda backlog, moves at most 0.25 s of travel, while an attached actor moves every step. Travel progress is therefore a function of wake cadence and actor attachment. [crates/app/src/world/farming/jobs.rs:788] `let elapsed = world.farming.clock.saturating_sub(world.farming.jobs.motion_at).min(PULSE);`
- Founding is a non-atomic two-stage transaction. Stage 0 publishes the chest voxel, writes the stock and sets stage = 1 before the workbench step. If the workbench step fails ("Workbench Obstructed", "Workbench Missing", or publish failure), advance returns false with stage 1 kept and saved, and FOUNDING retries at now+120 ticks. An interrupted founding is a real intermediate state that the shadow comparison must model. [crates/app/src/world/farming/jobs/communities/founding.rs:97] `city.stage = 1;`
- A raid's outcome depends on chunk residency. contact_clear treats any non-resident cell on the line between raider and defender as blocked. With the target's terrain unloaded no defender is found, so the code skips the scuffle and goes straight to removing up to 8 meals (transit.rs:481-482). The same raid near the player becomes a scuffle with health damage and a 240-tick next_action delay. [crates/app/src/world/farming/jobs/communities/transit.rs:495] `w.resident_block(p).is_some_and(|block| !block.is_solid())`
- Custody leaks on a worker death. clear_worker drops mission cargo, carry and batch recovery into the global world.inventory.dropped list, and once that list holds 1024 stacks it pushes them into world.inventory.overflow instead. Both are shared with the player and with every site, so the result depends on global drop count and event order. founding::advance and development::observe similarly share the global plot cap (founding.rs:129 `if w.farming.plots.len() >= farming::MAX_PLOTS { break; }`, MAX_PLOTS = 4096). [crates/app/src/world/farming/jobs.rs:483] `} else { world.inventory.overflow.push(stack); }`
- Community state is also mutated outside the agenda. politics::service is called from civilization::service (civilization.rs:899), once per second in the travel::step phase, before the agenda in the same step. It kills campaign residents through worker_fell, deposits supplies into a site chest (politics.rs:450) and sets worker health (politics.rs:454). Player UI paths enlist and return_home also cancel and schedule wakes directly (communities.rs:313, 326). [crates/app/src/world/travel/civilization/politics.rs:432] `crate::world::farming::jobs::worker_fell(world, force.resident, at);`
- On a multiplayer host, farming.clock still advances exactly once per step. The frame is created with shared_tick true, and only the first actor serviced keeps it, so farming::tick runs once. jobs::enabled is false for the whole session, so every agenda entry goes overdue while the clock runs. This answers the reader's last open question. [crates/app/src/multiplayer.rs:2221] `if let Some(ctx)=world.multiplayer.as_mut(){ctx.actor=id;ctx.shared_tick=first;}`
- Registration is gated by a wall-clock survey at the player's current position, not only by the 1 Hz cadence. civilization::service returns before the pending-registration block whenever the 110 m survey around the player's feet is not cached, and again at line 851 when discovering and the 550 m survey is not ready. The survey runs on a background thread with a 256-entry shared cache that evicts. Only the single lowest-id pending ledger is tried (`.next()` at line 895), so a ledger that cannot register blocks all higher ids. civilization::enabled also requires WorldType::Planet and generator_version >= geo::VERSION (lines 307-309). [crates/app/src/world/travel/civilization.rs:845] `if !geo::prepare_settlements_near(&cfg, feet, 110.0) { return; }`
- The initial roster's deaths are scoped to the game day. founding::plan records a resident as dead only if travel.away holds today's day number for them, and travel.away is cleared at day rollover (crates/app/src/world/travel.rs:130 `self.away.retain(|_, d| *d == day);`). A villager killed before the settlement is registered is dead if the player triggers registration the same day (57,600 ticks) and alive with a fresh worker if registration happens a day later. [crates/app/src/world/farming/jobs/communities/founding.rs:21] `let died_at = (!enlisted && w.travel.away.get(&id) == Some(&crate::world::travel::day(w)))`
- A generated town's starting stock is created, not transferred. With starter == None, founding fills the chest with population*2 bread, population*6 wheat and 16 planks from nothing. A settler-founded site uses the traveller's pack instead. Conservation checks need this as an explicit source term at stage 0 to 1. [crates/app/src/world/farming/jobs/communities/founding.rs:87] `ItemStack::new(MEAL,population * 2), ItemStack::new(Item::Block(B::WheatCrop),population * 6),`
- routine.due, an input to crew::next_wake that the reader left open, is in farming.clock ticks. It is first set on the resident's first routine service to now + 60*PULSE + (id % 24)*8*PULSE, which is 1,800 to 7,320 ticks ahead, and is reset to now + WORK_TICKS (240*PULSE = 7,200 ticks) on interrupt (routine.rs:11 and 62). Traveller constants are also ticks: next_meal +24,000 (transit.rs:91), rest 1,200 (transit.rs:102), raid next_action 240 (transit.rs:477). [crates/app/src/world/farming/jobs/routine.rs:79] `j.routine.due = now.saturating_add(60*PULSE + u64::from(id%24)*8*PULSE);`

## Open questions
- routine::Resident.due and routine::Phase semantics (inputs to crew::next_wake lines 264-266) were not read; routine.rs needs its own pass to pin exact resident wake times.
- In a continuing session, are physical villager actors written back into each community's Jobs (worker_pos/body/motion_at) other than via communities::remember and the save-time checkpoint clone? The size of the save/load divergence in A14 depends on this; the remember_worker callers were not traced.
- Does founding::plan run identically when a site is registered at different times? It reads w.travel.away, w.farming.clock, the local crew and live actors, so later discovery appears to give a different initial state; not confirmed by running a scenario.
- The site-absent mark_dead path (A86) skips travel-ledger resident removal only if resident_died also keys on the removed site; resident_died uses world.travel.settlements, so it should still run, but the full worker_fell/communities::fell path under an active borrow was not exhaustively traced.
- Headless and window drivers both call fixed::gameplay, but whether headless scenarios move the player near settlements (needed for register) was not checked; headless.rs was not read beyond its call site.
- Whether World::install_simulation_chunk (regions.rs:789) makes a chunk indistinguishable from a player-streamed chunk for all later readers, and when such chunks are evicted, was not traced; that decides how long 'resident' facts persist.
- The multiplayer path (multiplayer/authority.rs:81) calls adventure::tick per player with shared_tick only for the first; since enabled() is false there the agenda never runs, but whether farming.clock still advances once per step on a host was not verified beyond adventure.rs:3554-3560.

## Confirmed findings
- A1 The agenda is a BTreeSet of (u64 due, u32 site, u32 person) with default lexicographic tuple order; it is not serialized. [crates/app/src/world/farming/jobs/communities.rs:65]
- A2 Tuple field order is (due, site, person): ascending due, then site id, then person id. [crates/app/src/world/farming/jobs/communities.rs:137]
- A3 FOUNDING is pseudo-person u32::MAX, so it sorts last at equal (due, site). [crates/app/src/world/farming/jobs/communities.rs:18]
- A4 COUNCIL is pseudo-person u32::MAX-1, sorting after every real resident and before FOUNDING at equal (due, site). [crates/app/src/world/farming/jobs/communities.rs:19]
- A5 Real resident ids are bounded well below COUNCIL: original ids lie in [2^28, 2^28+2^21); extension ids start at 2^27 with a 2^25 span (line 479). [crates/worldgen/src/civilization.rs:20]
- A6 Save validation requires that a stage<2 community has exactly one wake, FOUNDING; so FOUNDING and COUNCIL/residents never coexist for one site. [crates/app/src/world/farming/jobs/communities.rs:115]
- A7 schedule(COUNCIL) removes the entry keyed by the old next_plan, stores due in Community.next_plan (saved) and inserts; COUNCIL is not kept in wakes. [crates/app/src/world/farming/jobs/communities.rs:131]
- A8 schedule(resident or FOUNDING) overwrites wakes[person] and removes the previous agenda key, so each person has at most one agenda entry. [crates/app/src/world/farming/jobs/communities.rs:136]
- A9 cancel(COUNCIL) removes the agenda entry but leaves next_plan unchanged; if no reschedule followed, restore() would re-add a COUNCIL event that the live session lacked. [crates/app/src/world/farming/jobs/communities.rs:142]
- A10 restore() rebuilds agenda entries for residents/FOUNDING purely from the saved wakes map. [crates/app/src/world/farming/jobs/communities.rs:83]
- A11 restore() re-adds COUNCIL from saved next_plan only for stage 2 communities. [crates/app/src/world/farming/jobs/communities.rs:84]
- A12 events counter is serde(skip): resets to 0 on load (owners, indexed, borrowed, spare, oldest_delay on lines 64-72 are skipped likewise). [crates/app/src/world/farming/jobs/communities.rs:71]
- A13 events and oldest_delay are read only by the JSON report, not by any scheduling logic. [crates/app/src/world/farming/jobs/communities.rs:440]
- A14 The saved communities blob is a clone checkpointed against live actors (worker_pos, body, motion_at=now overwritten in the copy only), so the loaded state differs from the continuing in-memory state. [crates/app/src/world/farming/jobs/communities.rs:124]
- A15 Communities are written as a RON 'COM1' section of the farming sidecar using checkpoint(). [crates/app/src/world/farming/sidecar.rs:121]
- A16 On load the sidecar validates and then calls restore() immediately (indexed becomes true). [crates/app/src/world/farming/sidecar.rs:262]
- A17 Per-worker path is not saved; after load every path is empty. [crates/app/src/world/farming/jobs/crew.rs:40]
- A19 The terrain source pool is skipped on save and Clone yields an empty pool. [crates/app/src/world/farming/jobs/residency.rs:13]
- A20 Per-tick cap is 8 events across all sites. [crates/app/src/world/farming/jobs/communities.rs:17]
- A21 An event is due when its due <= now; the loop always takes agenda.first(), breaking when the earliest is in the future. Events beyond the cap stay queued, none dropped. [crates/app/src/world/farming/jobs/communities.rs:245]
- A22 now is read once per service() call from w.farming.clock. [crates/app/src/world/farming/jobs/communities.rs:243]
- A23 The clock is a u64 incremented by exactly 1 per farming::tick call (one fixed step). [crates/app/src/world/farming.rs:62]
- A24 A fixed step is 1/120 s, so clock units are ticks at 120 Hz. [crates/core/src/time.rs:5]
- A25 PULSE = round(0.25 s / (1/120 s)) = 30 ticks. [crates/app/src/world/farming/jobs.rs:31]
- A26 Delay tracking is a session-lifetime maximum of (now - due) over processed events; never reset except by load. [crates/app/src/world/farming/jobs/communities.rs:248]
- A27 Successful founding wakes each crew member at now+i+1 ticks in crew::ids order (main worker, then helpers Vec order). [crates/app/src/world/farming/jobs/communities.rs:253]
- A28 Successful founding schedules the first COUNCIL at now + 120 + site % 480 ticks (site ids are multiples of 8, giving 60 distinct offsets). [crates/app/src/world/farming/jobs/communities.rs:254]
- A29 Failed founding (terrain pending, site occupied, founder absent, etc.) retries at now + 120 ticks (1 s). [crates/app/src/world/farming/jobs/communities.rs:257]
- A30 COUNCIL reschedules itself at now + development::INTERVAL, measured from processing time, not from due. [crates/app/src/world/farming/jobs/communities.rs:263]
- A31 development::INTERVAL is 600 ticks (5 s at 120 Hz). [crates/app/src/world/farming/jobs/communities/development.rs:8]
- A32 A resident event reschedules at crew::next_wake(jobs, person, now), again relative to processing time. [crates/app/src/world/farming/jobs/communities.rs:286]
- A33 next_wake idle branch: max(retry_at, now + 30 if the executor has no homes else now + 600 ticks), then lowered to routine.due or next meal, each clamped to >= now+PULSE. [crates/app/src/world/farming/jobs/crew.rs:265]
- A34 An event for an absent, enlisted or dead person does nothing and is not rescheduled (its wake was already cancelled). [crates/app/src/world/farming/jobs/communities.rs:265]
- A35 If the worker vanished from the executor during its service, the person is marked dead at now and gets no further wake. [crates/app/src/world/farming/jobs/communities.rs:282]
- A36 If with_site returns None the event's wake is silently lost (empty reschedule list); for COUNCIL next_plan stays stale. [crates/app/src/world/farming/jobs/communities.rs:287]
- A37 with_site is non-reentrant: any nested call while an executor is borrowed returns None. [crates/app/src/world/farming/jobs/communities.rs:224]
- A38 with_site removes the Community from sites for the closure's duration, so site-keyed queries (population, stockpile, mark_dead, cancel) see no entry for the active site. [crates/app/src/world/farming/jobs/communities.rs:225]
- A39 The community's Jobs is swapped into w.farming.jobs; the player's Jobs is parked in borrowed. [crates/app/src/world/farming/jobs/communities.rs:228]
- A40 The player's sources pool is moved into the active executor, so one pool is shared by the local crew and every site. [crates/app/src/world/farming/jobs/communities.rs:229]
- A41 with_site contains an expect() in non-test code that panics if borrowed was taken inside the closure. [crates/app/src/world/farming/jobs/communities.rs:232]
- A42 The shared pool polls for completed async chunk sources at most once per PULSE interval (clock / PULSE), whichever caller arrives first. [crates/app/src/world/farming/jobs/residency.rs:93]
- A43 prepare_cells installs at most 2 non-resident chunks per call and reports not-ready otherwise; readiness depends on which chunks are already in world.chunks. [crates/app/src/world/farming/jobs/residency.rs:102]
- A44 The underlying Residency pool allows 8 pending requests, served by worker threads with LRU eviction; earlier callers can exhaust it for later sites. [crates/app/src/simulation/residency.rs:16]
- A45 Residency.poll uses wall-clock time (500 µs drain cut-off, Instant retry timers), so terrain readiness is not a function of the tick count. [crates/app/src/simulation/residency.rs:250]
- A47 resident_block returns a value only for chunks present in world.chunks (or the CA view). [crates/app/src/world.rs:1800]
- A48 Chest custody transactions require the stockpile's chunk to be resident; cross-site trade, migration and launch fail or defer when it is not. [crates/app/src/world/farming/jobs/communities/custody.rs:6]
- A49 After the swap is undone, a COUNCIL event runs transit::policy for that site. [crates/app/src/world/farming/jobs/communities.rs:289]
- A50 After the swap is undone, every resident event (not FOUNDING) runs transit::arrive. [crates/app/src/world/farming/jobs/communities.rs:290]
- A51 transit::policy is gated to stage 2 and at least 2400 ticks (20 s) since last_expansion. [crates/app/src/world/farming/jobs/communities/transit.rs:206]
- A52 policy returns early until an async settlement survey within 512 m is ready (background thread), another non-tick-deterministic gate. [crates/app/src/world/farming/jobs/communities/transit.rs:220]
- A53 launch debits the mission cargo from the source chest immediately, in the same tick as the council event. [crates/app/src/world/farming/jobs/communities/transit.rs:192]
- A54 launch reschedules the chosen traveller at clock+1. [crates/app/src/world/farming/jobs/communities/transit.rs:199]
- A55 Traveller selection uses a positional hash of id^target (grievance for raids) with max_by_key, not stateful randomness. [crates/app/src/world/farming/jobs/communities/transit.rs:181]
- A56 Exchange on arrival mutates the target site's chest synchronously within the arriving resident's event. [crates/app/src/world/farming/jobs/communities/transit.rs:347]
- A57 Migration transfers the person and worker between sites immediately and schedules the migrant at the target at clock+1. [crates/app/src/world/farming/jobs/communities/transit.rs:455]
- A58 Settling creates a new Community immediately and schedules its FOUNDING at clock+1; this is a second creation path not triggered by the player. [crates/app/src/world/farming/jobs/communities/transit.rs:405]
- A59 A raid removes up to 8 meals from the target chest immediately. [crates/app/src/world/farming/jobs/communities/transit.rs:482]
- A60 Survey arrival re-enters with_site for the source (legal because arrive runs outside the borrow). [crates/app/src/world/farming/jobs/communities/transit.rs:358]
- A61 communities::service is the last call in jobs::service, after the local main worker (phase 0) and the helper slice for this phase. [crates/app/src/world/farming/jobs.rs:749]
- A62 When the local quarter is paused the local crew is skipped but communities::service still runs. [crates/app/src/world/farming/jobs.rs:731]
- A63 The local main worker runs only when clock % PULSE == 0, outside the agenda. [crates/app/src/world/farming/jobs.rs:742]
- A64 Local helpers run on a fixed phase slice of the PULSE, outside the agenda. [crates/app/src/world/farming/jobs.rs:746]
- A65 farming::tick (clock increment) runs before jobs::service within adventure::tick, both under shared_tick. [crates/app/src/world/adventure.rs:3560]
- A66 jobs::service is called with the player's feet in metres. [crates/app/src/world/adventure.rs:3562]
- A68 travel::step (the route to register) runs earlier in the same fixed step and is short-circuited whenever express::step returns true (player riding the lift). [crates/app/src/simulation/fixed.rs:68]
- A69 travel::step calls vehicle::service with the player's feet every step. [crates/app/src/world/travel.rs:314]
- A70 vehicle::service reaches civilization::service only when the pre-increment clock is a multiple of 120 (once per second). [crates/app/src/world/adventure/wildlife/vehicle.rs:385]
- A71 civilization::service looks up settlements within 110 m of the player's feet (after an async survey gate on line 845). [crates/app/src/world/travel/civilization.rs:846]
- A72 When discovering, ledgers are created for the region within REGIONAL_DISCOVERY_RADIUS = 550 m of the player. [crates/app/src/world/travel/civilization.rs:18]
- A73 At most one community is registered per civilization::service call: the lowest-id physical ledger with no community. [crates/app/src/world/travel/civilization.rs:894]
- A74 This is the only caller of communities::register. [crates/app/src/world/travel/civilization.rs:897]
- A75 register schedules FOUNDING at the current (pre-increment) clock, so it is due on the same fixed step's service pass. [crates/app/src/world/farming/jobs/communities.rs:158]
- A76 founding::plan seeds worker state from live physical actors present at registration time. [crates/app/src/world/farming/jobs/communities/founding.rs:28]
- A77 The initial roster depends on prior player actions: a resident already in the local crew is marked enlisted instead of getting a worker. [crates/app/src/world/farming/jobs/communities/founding.rs:20]
- A78 The whole system (register and service) is disabled under multiplayer. [crates/app/src/world/farming/jobs.rs:233]
- A79 It is also disabled when civilization is off or a CA view is active. [crates/app/src/world/farming/jobs.rs:236]
- A80 Worker servicing branches on whether a physical actor is attached: actor position if present, else saved position advanced headlessly. [crates/app/src/world/farming/jobs.rs:784]
- A81 The player's position is passed into the shared executor and affects construction placement/occupancy for community workers too. [crates/app/src/world/farming/jobs.rs:987]
- A82 Road traffic counters (saved) only accumulate where the destination chunk is resident. [crates/app/src/world/farming/jobs/communities/roads.rs:34]
- A83 MAX_SITES equals the travel settlement cap of 512. [crates/app/src/world/travel/civilization.rs:12]
- A84 A game day is 57,600 ticks (480 s). [crates/app/src/world/travel.rs:23]
- A85 return_home wakes the returned resident at clock+1, only if the community is at stage 2. [crates/app/src/world/farming/jobs/communities.rs:326]
- A86 A headless worker death inside a borrowed executor calls mark_dead, which finds no site entry (site removed by with_site); died_at is then set by service() line 282 instead. [crates/app/src/world/farming/jobs.rs:803]
- A87 The artisan tie-break in council planning is a positional hash of the id, not stateful randomness. [crates/app/src/world/farming/jobs/communities/development.rs:335]