# routine_policy

## Summary
Read-only pass over the files on disk; nothing was built or run. All routine times are `world.farming.clock` ticks: the clock gains 1 per `farming::tick` (one fixed player step, 1/120 s), and PULSE = 30 ticks = 0.25 s.

**Phase machine** (per worker, `Resident` in routine/policy.rs; driven by `routine::service`, called from `work()` after `transit::walk` and stockpile checks, before `groups::service`):
- **Entry guard:** `home()` must yield (destination, entry, members). If it fails and phase != Work, `interrupt` runs: interrupted+1, phase Work, due = now+WORK_TICKS (7200), worker's meetings dropped. Needs do not advance on that call.
- **Needs:** lazy by clock. elapsed = now − needs_at (PULSE when needs_at == 0); fatigue and social_need both saturating_add(elapsed). social_need has no reader in the non-check code searched; only fatigue gates anything.
- **First call:** due == 0 sets due = now + 60·PULSE + (id%24)·8·PULSE and since = now.
- **Work → Meal:** phase == Work, now ≥ due, fatigue ≥ 1800, and hungry (production exists and now ≥ next_meal[id], default 0). Pantry stock is not checked here.
- **Meal:** service returns false so normal job selection runs. `Task::Eat` is offered only if the chest holds the meal item. A committed Eat calls `ate` → `meal_committed`, which moves Meal → Rest only; eating in Work changes no phase. Meal has no timeout.
- **Rest:** walk to home destination; dwell accrues min(now − last, PULSE) per arrived observation and resets to 0 otherwise. At dwell ≥ 480: rests+1, fatigue = 0, rested_at = now, enter Social. Occupancy blocks approach ("Home Occupied").
- **Social:** `social::goal`. On completion: socials+1, social_need = 0, partner and time recorded, enter Work, due = now + 7200 + (id%8)·30.
- **Rest/Social exits:** hungry again → interrupt then enter Meal; now − since > 28800 → interrupt. External interrupts come from `service_worker` (following, guard post, mobilized, seeking safety).

**Social pairing:** partner is positional: index of id in `members`, pair = members[index/2·2 .. +2]; no partner means waiting until timeout. Meeting points are entry + across·{−2,−1} for the first pair and {+1,+2} for every later pair, with across = (inward.z, 0, −inward.x) and inward = (destination − entry)/2 (truncating). Limits: 32 meetings, one per resident. Meeting dwell needs partner in Social with a valid home, both bodies arrived, and clear line of sight; required 360 ticks, cap PULSE per observation.

**Ordering hazard:** in the community (`j.homes`) branch, `members` is `crew::ids(j)`, and `crew::service` swaps the serviced helper into the primary slot. The list is therefore relative to whoever is being serviced, and the two sides compute different `ids` arrays. By reading, such meetings look unable to complete; I did not run it. The household branch uses the stable `h.members`, and the daily-life check exercises only that branch.

**next_wake:** earliest = now + PULSE. Return earliest if there is no view, the body needs a tick, or task/path/carry/batch/mission is set or phase != Work. Otherwise due = max(retry_at, now + (PULSE if homes empty, else 600)); if routine.due > now, due = min(due, max(routine.due, earliest)); if next_meal > now, due = min(due, max(meal, earliest)).

**Groups:** voluntary "work circles" (≤64 people, ≤16 circles, ≤8 members). model.rs and decision.rs are integer-pure, with u8 traits from `hash_u32(seed ^ id·0x9e3779b9)`. `groups::service` reads World to build the inputs `opportunity` and `goal_live`.

**Classification:**
- **PURE, movable unchanged:** `Resident::{valid, enter, request_meal, meal_committed, observe}`, `Meeting::observe`, `meetings_valid`, `ate`, `crew::{service_phase, phase_indices, is_constructor}`, all of groups/model.rs and groups/decision.rs.
- **State-only (no World, tied to the Jobs struct):** `interrupt`, `source_cells`, `next_wake`, `groups::{met, goal, preference, selected, committed}`.
- **Pure float:** `arrived`, `waypoint_reached`.
- **WORLD-READ, with the fact each needs:**
  - `home`: home usable, giving (destination, entry, member list).
  - `routine::service`: goal cell standable; resident arrived at goal; home occupied by another member; hungry; route.
  - `social::goal`: both points standable; both arrived; line of sight; partner's home valid.
  - `crew::physical`: resident position, or none when fleeing, following or guarding.
  - `groups::service`: nearest productive peer in sight; nearest ready funded order.
- **WORLD-WRITE:** `jobs::service`, `service_worker`, `work`, `crew::{service, service_person}`, `communities::service`, `production::actions::execute`, `transit::walk`.

## Summary corrections (verifier)
Nothing was built or run; this is a read-only check of the files on disk. Constants, units and the phase machine in the summary match the code. Of 86 findings, 78 are confirmed and 8 are imprecise (A4, A14, A22, A37, A38, A77, A78, A79); none are wrong.

Where the summary is misleading or understated:

1. **Clock unit.** "The clock gains 1 per farming::tick (one fixed player step)" overstates it. farming::tick runs only inside adventure::tick, which is skipped when ca_view is set (crates/app/src/simulation/fixed.rs:145) or habitats review is paused (crates/app/src/world/adventure.rs:3550). It is also reached from crates/app/src/multiplayer/authority.rs:81. The clock is a saved counter (crates/app/src/world/farming/sidecar.rs:138), not a session step count.

2. **Ordering hazard is understated.** In the homes branch the serviced resident is always index 0, so the {+1,+2} points never occur and every pair contains helper 0. At most one meeting can exist per community at a time, and the partner never receives a goal. By reading, community residents leave Social only through hunger (interrupt, then Meal, at next_meal = meal + 24000 ticks) or the 28800-tick timeout. They would then sit in "Social Waiting", which returns true and does no work, for most of each meal interval. This was not executed.

3. **Meeting-point degeneracy is real, not hypothetical.** The summary treats it as an open question, but settlers get entry == destination (crates/app/src/world/farming/jobs/communities/transit.rs:395), which yields two identical points.

4. **"Needs: lazy by clock" covers only routine::service.** The mission path (transit.rs:93-97) uses a different elapsed formula with no zero sentinel. It advances fatigue only and applies a net 11x recovery while resting. Missions can start in any phase without interrupting the routine.

5. **The routine's dependence on terrain loading is missing.** The summary says routine::service runs "after transit::walk and stockpile checks" but omits that service_worker returns earlier on "Awaiting terrain" (residency::prepare false, crates/app/src/world/farming/jobs.rs:791) and on "Stockpile unloaded" (jobs.rs:843-846). Combined with capped dwell and uncapped fatigue, async loading changes outcomes.

6. **"Eating in Work changes no phase" hides a consequence.** The meal still sets next_meal = now + meal_ticks. If hunger arrives while now < due, the meal is eaten in Work and the Rest/Social cycle cannot start for another 24000 ticks. The cycle period is therefore set by meal_ticks, not WORK_TICKS.

7. **The "home" fact is not single.** Listing `home` as needing one fact ("home usable") hides a conjunction: travel state, two standable cells, a level-walk query, and in the frontier branch project/growth state plus a float equality against travel.homes (crates/app/src/world/farming/jobs/routine.rs:43-52).

8. **Groups decision inputs.** "World-derived inputs are exactly opportunity and goal_live" is true for decide(). But contact() calls (peer proximity and line of sight) and retain(crew ids) also feed groups state, and they run only on PERIOD-gated turns (crates/app/src/world/farming/jobs/groups.rs:19).

## Checks: 78 confirmed, 8 not

- [imprecise] A4 (crates/app/src/world/adventure.rs:3562) claim: farming::tick and jobs::service run back to back in the adventure fixed step, only when shared_tick is true.
  correction: Not back to back: wildlife::vehicle::sync_driver(world, feet) runs between them (crates/app/src/world/adventure.rs:3561). Further gates: adventure::tick returns early at adventure.rs:3550 when habitats::review::paused(), and fixed::gameplay only calls it when ca_view is None (crates/app/src/simulation/fixed.rs:145 'let consumed = legacy'). A second caller exists at crates/app/src/multiplayer/authority.rs:81.
- [imprecise] A14 (crates/app/src/world/farming/jobs/routine/policy.rs:91) claim: meetings_valid: at most capacity/2 meetings, distinct ids within and across meetings, distinct points, coordinates |n| < 2^28.
  correction: Points are only required to differ within one meeting (m.points[0]!=m.points[1], crates/app/src/world/farming/jobs/routine/policy.rs:93). Two meetings may share the same points; only ids are checked across meetings (policy.rs:92).
- [imprecise] A22 (crates/app/src/world/farming/jobs/routine.rs:90) claim: In any non-Work phase a carried stack is delivered first: task becomes Store and service returns false.
  correction: The task becomes Store only when the task is None or Task::Routine (crates/app/src/world/farming/jobs/routine.rs:88). With any other task and a carried stack, service leaves the task untouched and returns false (routine.rs:92).
- [imprecise] A37 (crates/app/src/world/farming/jobs/communities/transit.rs:97) claim: Mission rest: fatigue recovers 12x elapsed while resting; fatigue > 24_000 starts a 1200-tick rest (lines 101-102); the mission meal interval is 24_000 ticks (line 91).
  correction: Net recovery while resting is 11x elapsed, not 12x: line 95 adds elapsed unconditionally before line 97 subtracts elapsed*12 (crates/app/src/world/farming/jobs/communities/transit.rs:95-97). The rest trigger is strictly fatigue > 24_000 (transit.rs:101).
- [imprecise] A38 (crates/app/src/world/farming/jobs.rs:992) claim: productive counts executed tasks that changed build queue, production counters or carry; resumed (line 993) counts those when phase==Work and socials>0.
  correction: A carry change counts only for Task::Store and Task::Mine (crates/app/src/world/farming/jobs.rs:990). The production counters compared are planted, harvested, meals and batches, so 'eaten' does not count (jobs.rs:989). A Produce task whose execute returns false returns early at jobs.rs:926 and is never counted.
- [imprecise] A77 (crates/app/src/world/farming/jobs/groups/decision.rs:15) claim: Offer acceptance: recipient has no circle (line 14), belonging + trust >= autonomy + 40, and craft >= 30.
  correction: Acceptance also requires the World-derived opportunity to equal the offer goal and accept() to succeed: 'if willing && opportunity == Some(offer.goal) && self.accept(offer.clone(),now)' (crates/app/src/world/farming/jobs/groups/decision.rs:17). Otherwise the offer is refused, which sets retry_after = now + STALE (model.rs:122) and logs Declined.
- [imprecise] A78 (crates/app/src/world/farming/jobs/groups/decision.rs:56) claim: Proposer guard: craft >= 30, belonging*2 >= autonomy, and no outstanding offer from this person (line 57).
  correction: The stated guard is correct but incomplete. Lines 59-60 also stop the proposal when the proposer's circle already has MEMBERS (8) members, or the proposer has no circle and circles.len() >= CIRCLES (16) (crates/app/src/world/farming/jobs/groups/decision.rs:59).
- [imprecise] A79 (crates/app/src/world/farming/jobs/groups/decision.rs:65) claim: Offer recipient: circle-less person with no pending offer and trust >= 40 (line 64); tie-break is highest trust, then lowest id.
  correction: The recipient filter also requires self.can_offer(id,other,now), i.e. now >= the bond's retry_after for this direction (crates/app/src/world/farming/jobs/groups/decision.rs:64; model.rs:125-127). The tie-break (highest trust, then lowest id) is correct.

## Missed (verifier)
- Terrain readiness gates the whole routine. service_worker returns before work(), and so before routine::service, whenever residency::prepare is false ('Awaiting terrain'), and also when there is neither an actor nor a saved position. prepare includes the home cells (residency.rs:54). A delayed service later adds full elapsed fatigue but only min(elapsed, PULSE) dwell, so async loading changes routine outcomes. This is the load-order dependence the kernel must remove. [crates/app/src/world/farming/jobs.rs:791] `if !residency::prepare(world, feet) {`
- The routine clock is not a global step counter. adventure::tick, which holds farming::tick, runs only when ca_view is None; it also returns early at adventure.rs:3550 when habitats::review::paused(). A second caller passes its own dt at multiplayer/authority.rs:81. The clock is persisted (farming/sidecar.rs:138), so due, needs_at, since and next_meal are absolute values of a saved counter. [crates/app/src/simulation/fixed.rs:145] `let consumed = legacy`
- In the j.homes (community) branch the serviced resident is always members[0], so first is always 0. The {+1,+2} offsets are never used there. Every pair contains helpers[0] (the primary pairs with helper 0, helper 0 with the primary, every other helper with helper 0), so the overlap guard at social.rs:12 allows at most one meeting per community at a time. The partner's own goal() returns None, so it never walks to points[1]. [crates/app/src/world/farming/jobs/routine/social.rs:6] `let index=members.iter().position(|&member|member==id)?;`
- Degenerate home geometry does occur, which answers the reader's open question. A settler's home has entry == destination, so inward = (0,0,0) and both meeting points equal entry. social::goal would push that meeting if the cell is standable, and meetings_valid (points[0]!=points[1]) then fails Jobs::valid (jobs.rs:185). Lines 390 and 439 of the same file already special-case entry != destination. [crates/app/src/world/farming/jobs/communities/transit.rs:395] `person.home = Home { destination: at, entry: at };`
- The mission needs update has no needs_at == 0 sentinel, unlike routine.rs:74. A resident with needs_at == 0 (Worker::default, or an old save via serde default) gains fatigue equal to the whole clock on its first mission step. The kernel has two different 'elapsed' formulas to reproduce. [crates/app/src/world/farming/jobs/communities/transit.rs:93] `let elapsed = now.saturating_sub(j.routine.needs_at);`
- Starting a mission neither requires phase == Work nor interrupts the routine. transit::available (transit.rs:166-172) checks mission, carry, batch, shopping, following, guard, mobilized and position only. set_mission resets the task, not the Resident. While on a mission routine::service is skipped (jobs.rs:829), so phase, since and meetings freeze; a mission meal can still move Meal -> Rest through routine::ate (transit.rs:86). [crates/app/src/world/farming/jobs/crew.rs:285] `if j.worker == Some(id) { j.reset_task(); j.mission = Some(mission); }`
- Hunger pre-empts the ordinary executor in every phase. When the meal is due and the chest holds one, every production task other than Eat is invalid. With a carried stack only a matching Sow is valid (production.rs:219). This is the consumer of the 'meal available in pantry' fact outside routine::service. [crates/app/src/world/farming/jobs/production.rs:223] `if hungry { return task == Task::Eat(pile); }`
- The meal commit has guards beyond 'chest holds a meal': from == pile, no carry, now >= next_meal, and (actions.rs:35-37) the worker must be in production.workers and the pile must be a resident chest. restaff (communities.rs:206-219) drops following, guarding, mobilized and mission residents from production.workers, so they cannot eat. If take_hauled returns anything other than 1, nothing commits but execute still returns true and the task is reset. [crates/app/src/world/farming/jobs/production/actions.rs:82] `if from != pile || j.carry.is_some() || w.farming.clock < s.next_meal.get(&id).copied().unwrap_or(0) { return true; }`
- groups::service has its own per-person PERIOD gate before any work. Membership pruning (retain), person creation and the peer-contact test run only on a decision turn (at most once per 480 ticks per person), not on every Work-phase service. [crates/app/src/world/farming/jobs/groups.rs:19] `if j.groups.people.get(&id).is_some_and(|p|now < p.due) { return; }`
- Community wake times are not set only by next_wake. When founding completes, residents are staggered at now + i + 1 in crew::ids order, and COUNCIL at now + 120 + site%480 (line 254); a FOUNDING retry is now + 120 (line 257). return_home (communities.rs:326), launch (transit.rs:199) and migrate (transit.rs:455) schedule clock + 1. [crates/app/src/world/farming/jobs/communities.rs:253] `.map(|(i, id)| (id, now.saturating_add(i as u64 + 1)))`
- While the local quarter is paused, jobs::service runs only communities::service and returns. The local primary and helpers get no routine service, but the clock keeps advancing, so their needs jump on resume. The local primary is otherwise serviced only at clock % PULSE == 0 (jobs.rs:734-742). [crates/app/src/world/farming/jobs.rs:730] `if world.farming.jobs.quarter.as_ref().is_some_and(|q| q.paused) {`
- The frontier-household branch of home() needs more than one fact, including an exact float equality between the travel home and the destination in metres. It also needs project_layers non-empty, the id absent from housing_overrides, phase/growth liveness (routine.rs:43-44) and home_complete (routine.rs:47). [crates/app/src/world/farming/jobs/routine.rs:52] `if w.travel.homes.get(&id) != Some(&metres(destination,v)) || !stand(w, destination,v) || !stand(w,entry,v) { return None; }`

## Open questions
- Homes-branch social pairing: by reading, each serviced resident is index 0 of its own member list, the two sides build different ids arrays ([A,B] vs [B,A]), and the second side gets None from the overlap guard. Does any community meeting ever complete at runtime? Not executed; the kernel must decide whether to reproduce this or define a new rule.
- social_need is accumulated and reset but I found no reader in non-check code. Is it intentionally inert, or is a guard missing? The extraction should carry it as state only.
- rested_at, social_partner, social_at and interrupted appear write-only outside checks; UI or report readers outside crates/app/src/world were not searched.
- engine_core::hash::hash_u32 was not read; Person::new depends on its exact definition for trait values.
- Not read: clear_level_walk, frontier home_complete, growth households_live, supply::destination, path_in_range internals and trace::solid. The 'home usable' and 'route' facts are described from call sites only.
- Meeting points degenerate when |destination-entry| < 2 on both x and z: inward=(0,0,0) makes both points equal to entry. Creation does not reject this, but meetings_valid would. Do home layouts guarantee a 2-cell offset?
- Later pairs (first >= 2) all get the same two meeting points relative to their home; whether households ever exceed 4 members sharing one home was not checked.
- I did not trace the caller of the adventure function containing farming::tick (adventure.rs:3560), so I have not confirmed it runs exactly once per 120 Hz step in both the window and headless drivers.
- The clock increment is a plain += 1 (debug overflow panic, release wrap) while everything else saturates; the kernel needs a defined overflow rule.
- Community residents wake through the agenda with at most 8 events per tick. A delayed service still adds full elapsed fatigue but only min(elapsed, PULSE) dwell, so backlog changes Rest and Social duration; no test pinning this was found.
- On a helper's service the Rest occupancy scan (routine.rs:104) iterates j.helpers, which then contains the swapped-out primary; in the communities path I did not check whether stockpile or priorities overrides alter anything routine reads.

## Confirmed findings
- A1 PULSE is the base routine unit: 0.25 s expressed in player steps, i.e. 30 ticks at 120 Hz (rounded float-to-u64 at compile time). [crates/app/src/world/farming/jobs.rs:31]
- A2 The player step is 1/120 s; this defines the tick unit for PULSE and every routine constant. [crates/core/src/time.rs:5]
- A3 The routine clock is world.farming.clock, incremented by 1 per farming::tick with plain (non-saturating) addition. [crates/app/src/world/farming.rs:62]
- A5 REST_TICKS = 16*PULSE = 480 ticks (4 s): required dwell at home to finish Rest. [crates/app/src/world/farming/jobs/routine.rs:9]
- A6 SOCIAL_TICKS = 12*PULSE = 360 ticks (3 s): required meeting dwell. [crates/app/src/world/farming/jobs/routine.rs:10]
- A7 WORK_TICKS = 240*PULSE = 7200 ticks (60 s): work interval before the next routine cycle may start. [crates/app/src/world/farming/jobs/routine.rs:11]
- A8 TIMEOUT = 960*PULSE = 28800 ticks (240 s): maximum time in Rest/Social and maximum meeting age. [crates/app/src/world/farming/jobs/routine.rs:12]
- A9 Resident::enter sets phase, since and last to now, and clears dwell and target; due, counters and needs are untouched. PURE. [crates/app/src/world/farming/jobs/routine/policy.rs:29]
- A10 Work -> Meal guard is pure integer: phase==Work, now>=due, fatigue>=threshold, hungry flag. [crates/app/src/world/farming/jobs/routine/policy.rs:32]
- A11 Meal -> Rest happens only via meal_committed and only when phase==Meal; a meal eaten in Work changes no phase. [crates/app/src/world/farming/jobs/routine/policy.rs:37]
- A12 Resident::observe: dwell saturating-adds min(now-last, cap) when present, resets to 0 when absent; last=now either way (line 41). [crates/app/src/world/farming/jobs/routine/policy.rs:40]
- A13 Meeting::observe uses the same rule but freezes (no dwell or last update) once dwell>=required. [crates/app/src/world/farming/jobs/routine/policy.rs:83]
- A15 Needs are lazy: elapsed is clock time since needs_at, or exactly PULSE when needs_at is the 0 sentinel. [crates/app/src/world/farming/jobs/routine.rs:74]
- A16 fatigue accumulates by saturating_add(elapsed); social_need does the same on line 76; needs_at=now on line 77. [crates/app/src/world/farming/jobs/routine.rs:75]
- A17 First service (due==0 sentinel) staggers the first cycle: due = now + 1800 + (id%24)*240 ticks; since=now on line 80. [crates/app/src/world/farming/jobs/routine.rs:79]
- A18 hungry = production exists and now >= next_meal[id] (missing entry = 0). Pantry stock is not consulted. [crates/app/src/world/farming/jobs/routine.rs:82]
- A19 Meal fatigue threshold is 60*PULSE = 1800 ticks (15 s). [crates/app/src/world/farming/jobs/routine.rs:84]
- A20 If home() fails, service returns false, interrupting first when phase != Work; needs are not advanced on that call (they catch up later through needs_at). [crates/app/src/world/farming/jobs/routine.rs:71]
- A21 interrupt: interrupted+1 only if phase != Work; clears a Routine task; enter(Work); due = now + WORK_TICKS; removes the worker's meetings (line 63). No World access. [crates/app/src/world/farming/jobs/routine.rs:62]
- A23 Meal phase returns false before the timeout check, so Meal has no timeout in routine::service; the ordinary job executor must eat. [crates/app/src/world/farming/jobs/routine.rs:94]
- A24 Rest/Social -> Meal: if hungry again during Rest or Social, interrupt runs (counter+1, due reset) and then Meal is entered. [crates/app/src/world/farming/jobs/routine.rs:96]
- A25 Rest/Social timeout: more than TIMEOUT ticks since phase entry interrupts back to Work. [crates/app/src/world/farming/jobs/routine.rs:98]
- A26 Rest occupancy rule 1 (state-only): with no target yet, wait if any other crew entry is in Rest with target equal to this home destination. [crates/app/src/world/farming/jobs/routine.rs:105]
- A27 Rest occupancy rule 2 (WORLD-READ, float): wait if another member's physical position is within 1.8 m of the destination. [crates/app/src/world/farming/jobs/routine.rs:109]
- A28 Rest -> Social when dwell >= REST_TICKS: rests+1, fatigue=0, rested_at=now (lines 130-132), then enter Social. [crates/app/src/world/farming/jobs/routine.rs:133]
- A29 Rest dwell is observed with cap PULSE only when the float 'arrived' test passes; otherwise observe(false) zeroes dwell (line 138). [crates/app/src/world/farming/jobs/routine.rs:127]
- A30 A blocked goal cell (stand false) zeroes dwell, sets last=now and keeps the worker owned by the routine. [crates/app/src/world/farming/jobs/routine.rs:122]
- A31 'arrived' is float geometry: flat distance < 0.23 m (line 16) and vertical gap < max(v,0.25)*0.25. [crates/app/src/world/farming/jobs/routine.rs:15]
- A32 Routine walking stall rule: 8 consecutive services without movement give retry_at = now + 4*PULSE and dwell=0; an empty path gives retry_at = now + 8*PULSE (line 153). [crates/app/src/world/farming/jobs/routine.rs:146]
- A33 The Eat task commits the meal: one item taken from the pile chest (line 85), then routine::ate; next_meal = now + meal_ticks on line 88. WORLD-WRITE. [crates/app/src/world/farming/jobs/production/actions.rs:86]
- A34 Eat is offered only when the meal is due and the chest holds at least one meal item; this is the 'meal available in pantry' fact. [crates/app/src/world/farming/jobs/production.rs:167]
- A35 Community production uses meal_ticks = 24_000 ticks (200 s); assets/city_colony.ron:122 and assets/city_livelihood.ron:23 use 24000 too. [crates/app/src/world/farming/jobs/communities/founding.rs:154]
- A36 A second writer of routine needs is the travelling mission: fatigue += elapsed and needs_at=now (lines 93-95); social_need is not advanced there. [crates/app/src/world/farming/jobs/communities/transit.rs:95]
- A39 Assigning the primary worker resets the routine to default (needs_at=0, due=0 sentinels); crew::add also uses Worker::default (crew.rs:144). [crates/app/src/world/farming/jobs.rs:343]
- A40 Community founders start with needs_at = now, unlike locally assigned workers. [crates/app/src/world/farming/jobs/crew.rs:220]
- A41 routine::service runs inside work() after transit::walk (line 829) and the stockpile checks, and before groups::service; returning true means the routine owns the worker this service. [crates/app/src/world/farming/jobs.rs:854]
- A42 next_wake: earliest = now + PULSE, returned when the worker has no view. [crates/app/src/world/farming/jobs/crew.rs:256]
- A43 next_wake returns earliest whenever the body needs a tick (line 262) or task/path/carry/batch/mission is set or phase != Work (line 264 holds the phase clause). [crates/app/src/world/farming/jobs/crew.rs:263]
- A44 Idle next_wake base: max(retry_at, now + PULSE when homes is empty, else now + 600 ticks (5 s)). [crates/app/src/world/farming/jobs/crew.rs:265]
- A45 next_wake is pulled earlier by a future routine.due, never earlier than earliest. [crates/app/src/world/farming/jobs/crew.rs:266]
- A46 next_wake is pulled earlier by a future next_meal (filter on line 267), never earlier than earliest. [crates/app/src/world/farming/jobs/crew.rs:268]
- A47 Community residents are re-queued with crew::next_wake after each agenda service. [crates/app/src/world/farming/jobs/communities.rs:286]
- A48 The agenda drains at most 8 due events per tick. [crates/app/src/world/farming/jobs/communities.rs:17]
- A49 Local crew helpers are serviced once per PULSE in a phase slice: helper index i runs at clock%PULSE == (i+1)*PULSE/64. [crates/app/src/world/farming/jobs/crew.rs:9]
- A50 crew::CAPACITY is 64. [crates/app/src/world/farming/jobs/crew.rs:4]
- A51 social::goal prunes meetings that are fully done or older than TIMEOUT on every call. [crates/app/src/world/farming/jobs/routine/social.rs:5]
- A52 Partner choice is positional: pair = members[first], members[first+1] with first = index/2*2 (line 7); an unpaired last member gets None. [crates/app/src/world/farming/jobs/routine/social.rs:8]
- A53 Meeting creation limits: at most CAPACITY/2 = 32 meetings, and neither id may already be in a meeting. [crates/app/src/world/farming/jobs/routine/social.rs:12]
- A54 Meeting points derive from home geometry: inward = (destination-entry)/2 with truncating integer division; across is its 90-degree rotation (line 14). [crates/app/src/world/farming/jobs/routine/social.rs:13]
- A55 Point offsets: first pair uses across*-2 and across*-1; every later pair uses across*1 and across*2, so later pairs share the same two points (line 16 builds them). [crates/app/src/world/farming/jobs/routine/social.rs:15]
- A56 Both meeting points must be standable at creation (WORLD-READ: block query). [crates/app/src/world/farming/jobs/routine/social.rs:17]
- A57 Meeting presence needs: partner in Social with a valid home (lines 24-25), both bodies arrived at their points (lines 26-28), and clear line of sight at +0.7 m (lines 29-30). [crates/app/src/world/farming/jobs/routine/social.rs:24]
- A58 Meeting dwell is observed with cap PULSE and required SOCIAL_TICKS, once per participant service. [crates/app/src/world/farming/jobs/routine/social.rs:32]
- A59 Social -> Work on completion: socials+1, social_need=0, partner and time recorded (lines 36-39); due = now + 7200 + (id%8)*30 ticks. [crates/app/src/world/farming/jobs/routine/social.rs:40]
- A60 The first side to complete a meeting registers a group contact for the pair. [crates/app/src/world/farming/jobs/routine/social.rs:41]
- A61 In the community (j.homes) branch, the social member list is the whole crew in crew::ids order. [crates/app/src/world/farming/jobs/routine.rs:40]
- A62 In the frontier household branch, the member list is the household's stored members (stable order). [crates/app/src/world/farming/jobs/routine.rs:55]
- A63 crew::ids yields the primary worker first, then helpers in vector order. [crates/app/src/world/farming/jobs/crew.rs:386]
- A64 Servicing a helper swaps it into the primary slot and parks the previous primary at helpers[index] (line 304), so crew::ids order depends on who is being serviced; the homes-branch pairing is therefore service-relative. [crates/app/src/world/farming/jobs/crew.rs:302]
- A65 home() returns None, forcing interrupt, for a followed or guard-post resident (WORLD-READ of travel state). [crates/app/src/world/farming/jobs/routine.rs:33]
- A66 home() in the homes branch requires destination and entry standable, plus a clear level walk between them (lines 38-39). [crates/app/src/world/farming/jobs/routine.rs:37]
- A67 crew::physical gives the live actor position unless it is seeking safety; with no actor it falls back to the saved worker_pos (line 395) unless following or guarding. [crates/app/src/world/farming/jobs/crew.rs:392]
- A68 Group limits: 64 people, 16 circles (line 5), 8 members per circle (line 6). [crates/app/src/world/farming/jobs/groups/model.rs:4]
- A69 Group decision PERIOD = 16*PULSE = 480 ticks. [crates/app/src/world/farming/jobs/groups/model.rs:7]
- A70 Group STALE = 240*PULSE = 7200 ticks. [crates/app/src/world/farming/jobs/groups/model.rs:8]
- A71 A group Goal is an integer area: build cell floor-divided by 8 (x,z) and 16 (y). [crates/app/src/world/farming/jobs/groups/model.rs:13]
- A72 Person traits are positional hashes: craft 30..=100, belonging 30..=100 (line 28), autonomy 10..=90 (line 29), from one hash word. [crates/app/src/world/farming/jobs/groups/model.rs:27]
- A73 Bond trust starts at 20 (line 113) and gains 20 per contact at least PERIOD apart, capped at 100. [crates/app/src/world/farming/jobs/groups/model.rs:103]
- A74 Effective trust decays by 1 per STALE elapsed since last contact (saturating). [crates/app/src/world/farming/jobs/groups/model.rs:118]
- A75 Bonds cap at PEOPLE*2 = 128; eviction picks a stale bond with trust < 20 by minimum (trust, at, pair), on line 109. [crates/app/src/world/farming/jobs/groups/model.rs:106]
- A76 decide runs at most once per PERIOD per person (guard on line 7; re-arm on line 8). Integer-pure. [crates/app/src/world/farming/jobs/groups/decision.rs:8]
- A80 Offers expire after 4*PERIOD = 1920 ticks. [crates/app/src/world/farming/jobs/groups/decision.rs:67]
- A81 groups::service runs only in Work phase and always force-enables groups first. [crates/app/src/world/farming/jobs/groups.rs:16]
- A82 Peer contact is WORLD-READ float: nearest productive peer within 4 m (squared 16.0, line 27), ordered by distance then id, then a line-of-sight test. [crates/app/src/world/farming/jobs/groups.rs:29]
- A83 The work opportunity scans up to 16 build orders from a per-person cursor; candidates sort by preferred-goal match, then float distance (lines 56-57). [crates/app/src/world/farming/jobs/groups.rs:45]
- A84 The World-derived inputs to the pure decision are exactly opportunity (first candidate goal) and the goal_live flag. [crates/app/src/world/farming/jobs/groups.rs:65]
- A85 Workers are disabled in multiplayer: enabled() returns false when world.multiplayer is set (feature-gated). [crates/app/src/world/farming/jobs.rs:233]
- A86 The daily-life acceptance check draws its eligible residents from frontier households, so it does not cover the homes-branch pairing. [crates/app/src/world/farming/jobs/routine/check.rs:35]