# transactions

## Summary
Read-only pass over the files on disk; nothing was built or run.

Time base: `farming.clock` advances by 1 per `farming::tick`, one fixed step at 120 Hz. `PULSE` is 30 ticks (0.25 s). Every `due`, `retry_at`, `next_meal` and `meal_ticks` is in clock ticks; `work` and batch `progress` count worker executions, at most one per PULSE.

1. Goods: there is no ledger. Holders are independent:
- chest slots in the global `block_data.entries` map (stockpile, hub and market seller are all ordinary chests);
- one `carry` stack per worker, tagged by `cargo` (Build{source} or Supply{source,target,project}), `building_material`, `shopping`, `sowing`;
- a crafting `Batch` holding debited inputs;
- a traveller's `Mission.cargo` pack and a founder's `Community.starter`;
- the global `inventory.dropped` list (cap 1024), spilling into the player's `inventory.overflow`;
- bare u32 counters for pantry (meals, biomass, purse), freight and nursery.
Every move is a read-modify-write of two of these under exclusive `&mut World`.

2. Reservations: the only stored reservation is `Claim{worker, order}`, created in `fetch` after the chest debit, or by `reconcile_claims` for unclaimed cargo. Stock reserved for a funded project, station use, courier exclusivity and task exclusion are recomputed from other workers' state each call. Meetings are stored pairs with a 960-pulse timeout. Homes are not reserved. Claims are released on death, enlist or transfer, mission start, follow/guard/mobilise, seeking safety, eight stalled pulses, an empty route, a store, a non-work routine phase and a supply pickup. `reconcile_claims` also purges any claim whose worker is gone or no longer carries matching material, so leaks self-heal once a worker reaches it. Claims persist while a worker waits on terrain or the stockpile is missing or unloaded.

3. Construction: `fetch` debits up to 8 units and claims that many cells. `place` checks, in order: claim ownership and cargo source, matching carry, player/actor/crew occupancy, `order.ready`, drop capacity. It then calls `effects::replace`, publishes the voxel, and only then decrements carry and removes the order and claim.
- Goods cannot be consumed with a failed voxel edit: every failure returns before the debit.
- No fallible step sits between the edit and the debit, so no worker placement goes undebited. The exception is founding a generated town, which places chest and workbench and mints starting stock without a debit, by design.
- Retry is implicit: the task is cleared after every execution and reselected next pulse, with no backoff. All four `Failure` variants are treated alike, so a permanently rejected cell keeps its claim and cargo and is retried indefinitely (inferred, not run).

4. Interruption: the whole `State` is serialised as RON in the farming sidecar and validated on load; paths, status and frames are rebuilt.
- Follow, guard, safety or a blocked route: claims released, cargo kept and walked back to its source chest.
- Rest or meal phase while carrying: same return.
- Death: carry, mission pack and batch contents are dropped at the body; orders stay queued.
- Batches keep their progress; mining progress resets.
- Build progress is not stored: each order is one voxel and projects re-read the world.

5. Conservation is checked only in opt-in scenario checks and unit tests (supply probe, lifecycle, livelihood, frontier, market, crew). At runtime there are structural `valid()` invariants and two `debug_assert`s; no total is checked.

6. Money: there is no currency and no wage. "Paid" means physical goods are present or consumed. Barter payment is an item moved between chests. The pantry purse is a counter that starts at 16.

Non-atomic pairs (sequential, no rollback) are listed in findings F1 to F8.

## Summary corrections (verifier)
Most of the reader's 84 findings hold; 11 are imprecise and none is wrong. The summary is misleading in these places:

1. **Indefinite retry is overstated.** `select_task` drops any owned order that fails `ready`, `unoccupied` or `workers_clear` (jobs.rs:1076-1082) and then selects Store. `Conflict` and `NoChange` are therefore normally unreachable from `place`. The livelock applies only when `effects::replace` fails while `ready` stays true: `Rejected` (factory anchor, mill water, protected orbital cell) or `Deferred` (non-resident chunk, activity queue full, a `World::set` environment skip, a sealed wet cell). For residents with a home it ends when the routine leaves Work (routine.rs:87-91).

2. **Failed edits are not side-effect free.** No goods are consumed on failure, but `World::set` can refine the environment or create a soil patch before skipping (world.rs:2289-2297), so world state can change on a failure.

3. **"Walked back to its source chest" is true only for Build cargo.** Supply cargo continues to its target hub (supply.rs:29); untagged cargo goes to the current stockpile.

4. **Death does not drop batch contents at the body.** They drop at the batch station (jobs.rs:470); only carry and mission pack drop at the body. The reader's own finding I5 says this correctly.

5. **"Mining progress resets" holds for interruptions only.** `work` and `task` are serialised (jobs.rs:80-85), so a save/load mid-mine keeps the count. It resets on `reset_task` or when the target is out of reach (jobs.rs:917).

6. **`valid()` is not a runtime invariant.** Production code evaluates it only when decoding the sidecar (sidecar.rs:239, 261). There are also more than two `debug_assert`s on these paths (batch.rs:54, crew.rs:238, effects.rs:39/51/167, block_data.rs:133, environment/construction.rs:99/106).

7. **"The only stored reservation is Claim" contradicts the summary's own next sentence about stored meetings.** Home assignments and vacancies are also stored, so "homes are not reserved" is true only of interior occupancy during Rest.

8. **The persistence account omits the file split.** Chests are saved in the commit manifest, not the farming sidecar, the two are written separately, and a sidecar validation failure drops all farming state (sidecar.rs:280).

9. **The 1024 drop cap is per call site.** `travel::trade` pushes surplus output with no cap check (travel.rs:222).

10. **The holder list omits three things.** The campaign `supply` counter (politics.rs:24) is a holder, harvest is a minting point, and chest replacement is a custody move.

Nothing was built or run for this verification; every status comes from reading the files on disk.

## Checks: 75 confirmed, 9 not

- [imprecise] U1 (crates/app/src/world/farming/jobs.rs:31) claim: PULSE is 0.25 s expressed in fixed player steps: 30 ticks at 120 Hz. All jobs retry/backoff values are multiples of it.
  correction: PULSE = 30 ticks at 120 Hz is right (crates/app/src/world/farming/jobs.rs:31). Only the per-worker retry_at values are PULSE multiples (2, 4 or 8 PULSE at jobs.rs:900, jobs.rs:1214, routine.rs:146, routine.rs:153; PULSE*blocked at communities/transit.rs:133). Agenda dues are not: founding wakes are now+i+1 and the first council is now+120+site%480 (communities.rs:253-254), enlist/return/launch wakes are clock+1 (communities.rs:326, transit.rs:199).
- [imprecise] R1 (crates/app/src/world/farming/jobs/construction.rs:156) claim: The only stored reservation is Claim{worker, order}; it is created after the chest debit, one per fetched unit.
  correction: Claim is the only stored reservation of build cells against carried goods (crates/app/src/world/farming/jobs/construction.rs:156), but it is not the only stored reservation. Meetings are stored pair reservations (jobs.rs:44), each resident has a stored home assignment (jobs.rs:36 `homes`, communities.rs:37 `Person.home`), a shopping Trip is stored (jobs.rs:52), and migration consumes a stored vacancy list (communities/transit.rs:448).
- [imprecise] R3 (crates/app/src/world/farming/jobs/reservations.rs:32) claim: reconcile_claims, run in each worker's work(), garbage-collects every claim whose order left the queue or whose worker no longer carries matching building material; this is the leak backstop.
  correction: reconcile_claims keeps a claim only if the order is still in `build` and the claimant has `building_material` set and a cargo source matching the order (crates/app/src/world/farming/jobs/reservations.rs:32-34). It does not compare the carried item or count with the claim; that test exists only in claims_valid (reservations.rs:55-57). Claims in excess of the carried count are not trimmed (reservations.rs:41 only computes a shortfall).
- [imprecise] R8 (crates/app/src/world/farming/jobs/routine.rs:104) claim: Homes/beds are not reserved; occupancy is inferred from another resident's routine target at check time.
  correction: Home assignment is stored: `State.homes` keyed by resident id (crates/app/src/world/farming/jobs.rs:36), `Person.home` (communities.rs:37) and frontier household membership (routine.rs:27). Migration takes a stored vacancy (communities/transit.rs:448). Only interior occupancy during Rest is inferred, from other helpers' serialised `routine.target` (routine.rs:104-105) and from a physical position within 1.8 m (routine.rs:108-109).
- [imprecise] R15 (crates/app/src/world/farming/jobs/supply.rs:63) claim: Cargo is returned to its recorded source chest, not the current stockpile; if that chest is gone, store_valid fails and the worker keeps the stack until death.
  correction: Only Build cargo goes back to its recorded source. Supply cargo is routed forward to its target hub chest: `Self::Build { source } => source.into(), Self::Supply { target, .. } => target.into()` (crates/app/src/world/farming/jobs/supply.rs:29). Untagged cargo (mined or hauled items) goes to the current stockpile (supply.rs:63). The stuck-until-death case holds when the destination chest is gone, and ends early if a chest reappears at that cell (supply.rs:67).
- [imprecise] C10 (crates/app/src/world/farming/jobs.rs:995) claim: Retry is implicit: after any task execution the task is cleared and reselected on the next service, with retry_at left at 0 (no backoff after a failed placement).
  correction: Not after any execution. A Mine still short of its pulse count returns before the reset (crates/app/src/world/farming/jobs.rs:964-966), and a Produce task whose execute returns false keeps its task (jobs.rs:926). For Build, Fetch, Fund, Buy, Supply, Store and Haul the claim holds: reset_task at jobs.rs:995 sets retry_at to 0 (jobs.rs:221). A failed place is retried on the next pulse only if the order still passes ready/unoccupied/workers_clear in select_task (jobs.rs:1076-1082); otherwise the worker selects Store.
- [imprecise] I2 (crates/app/src/world/farming/jobs/routine.rs:89) claim: Rest/meal/social phases interrupt a carrying builder by releasing claims and routing the cargo to Store at its source.
  correction: The conversion to Store happens only when the carrier has no task or a Routine task: `if j.task.is_none() || matches!(j.task,Some(Task::Routine(_)))` (crates/app/src/world/farming/jobs/routine.rs:88). A builder with an accepted Build keeps it, places that one block, and is redirected on the following pulse after reset_task. It applies only to residents with a usable home; without one routine::service returns at routine.rs:70-73 and the phase never leaves Work.
- [imprecise] K7 (crates/app/src/world/farming/jobs/supply.rs:82) claim: Structural runtime invariant: at most one worker may hold Supply cargo.
  correction: Not a runtime check. supply::valid (crates/app/src/world/farming/jobs/supply.rs:80-83) is part of State::valid (jobs.rs:192), which production code evaluates only when the sidecar is decoded (sidecar.rs:239, sidecar.rs:261); every other caller is a scenario check or test. During ticks the single-courier rule is enforced by task admission in transport::occupied (supply/transport.rs:5-10).
- [imprecise] P4 (crates/app/src/world/farming/jobs/communities/founding.rs:86) claim: Goods are created at founding of a generated town: meals, wheat seed and 16 planks sized by population.
  correction: Only bread and wheat scale with population; planks are a fixed 16: `ItemStack::new(MEAL,population * 2), ItemStack::new(Item::Block(B::WheatCrop),population * 6),` then `ItemStack::new(TIMBER,16)` (crates/app/src/world/farming/jobs/communities/founding.rs:87-88).

## Missed (verifier)
- A voxel edit that effects::replace reports as Deferred is not side-effect free. World::set skips an edit silently on about a dozen conditions beyond effects::admit, and some skips come after the environment has already been refined or a soil patch created (world.rs:2289-2297). This answers the reader's open question on set_many_with: yes, it can leave side effects. [crates/app/src/world.rs:2296] `if !phase_change && !self.displace_for_construction(p,current,id) {`
- There is a concrete permanent-failure path for construction: a solid block placed into a wet or sediment-bearing cell with no open resident neighbour is refused (returns false at line 71-72). World::set then skips, replace returns Deferred, Order::ready stays true, and the builder retries every PULSE with claim and cargo pinned until its routine phase changes. Paving wet soil is the likely trigger. [crates/app/src/world/environment/construction.rs:3] `//! A sealed wet cell rejects construction; it cannot destroy the flood.`
- Persistence of goods is split across files and is not atomic. Chest contents are written into the commit manifest (storage.rs:429 inside session_footer), while carry, batch, mission pack, starter and claims go to the farming sidecar, which is renamed into place before the commit is written (background.rs:326). A crash between the two leaves a newer jobs state beside an older chest image. [crates/app/src/world/storage/background.rs:319] `super::super::farming::sidecar::save(&view.root, &job.farming)?;`
- A jobs or communities validation failure on load discards the entire farming state (clock, plots, pantry, freight, all crews and communities), not only the jobs section. Chests still load from the save body, so every unit held in carry, a batch, a mission pack or a starter pack is lost, and generated towns re-found with fresh minted stock. [crates/app/src/world/farming/sidecar.rs:280] `Err(e) => eprintln!("[farming] Ignoring {}: {e:#}", path.display()),`
- Any voxel edit that replaces a chest block moves its whole contents to world drops, or to the player's overflow once 1024 drops exist. This runs inside World::set (world.rs:2339) for every writer, so a stockpile can be emptied without any jobs transfer function being called. Replacing a cell with a chest resets its slots to empty (block_data.rs:172). [crates/app/src/world/block_data.rs:184] `dropped.push(DroppedStack { pos: centre, stack });`
- A settler's starter pack can be stranded. On arrival the pack moves into Community.starter (transit.rs:401). If no crew member is then within 3 m of the planned chest cell, founding is blocked and rescheduled every 120 ticks. Death drops carry, mission and batch (jobs.rs:478-498) but never the starter, so a founder who dies before stage 1 leaves the goods in the Community record indefinitely. [crates/app/src/world/farming/jobs/communities/founding.rs:79] `city.blocked = Some("Founder Absent".into());`
- Campaigns are another goods holder and a destruction site the reader did not list. Mobilising debits 16 bread and 4 planks from a physical town chest; the bread becomes a u8 `supply` counter and the planks vanish. Each siege round destroys 4 defender planks (politics.rs:515). Player reinforcement adds biomass items to the same counter (politics.rs:287-291), which is later deposited as bread (politics.rs:450-452). [crates/app/src/world/travel/civilization/politics.rs:238] `if health <= 0.0 || !spend(world, source.id, u32::from(SUPPLY), 4) { return "Settlement supplies unavailable"; }`
- Harvest is a goods creation point outside the reader's list of moves. The yield comes from a keyed function of world seed, cell and plot; one unit is implicitly consumed to replant and the remaining yield_count-1 are minted into carry (line 76) after the voxel commit. Seed (chest to carry, line 42) and Sow (carry to voxel plus plot) are the matching moves. [crates/app/src/world/farming/jobs/production/actions.rs:69] `let yield_count = farming::harvest_count(w.seed, b.at.into(), &plot, c);`
- The stock reservation is advisory outside the crew that computes it. Factory chest IO consults only world.farming.jobs, which is the player's crew whenever the factory runs, so community reservations are never honoured. The player's chest UI path (block_data.rs:292-301 `take`) checks no reservation at all. [crates/app/src/world/farming/jobs.rs:1229] `w.farming.jobs.unreserved_stock(w,pile,item)`
- Two different terrain-fact rules coexist. Workers use farming::known, which samples never-edited non-resident chunks synchronously (farming.rs:100-103). The community council shadows it with a resident-only lookup, so project completion and order queueing (development.rs:253, 265) change with chunk residency. A shadow kernel must reproduce both rules to match decisions under varied terrain completion order. [crates/app/src/world/farming/jobs/communities/development.rs:15] `w.resident_block(at)`
- Community build queues are never pruned (the reader's R16 and open question 3), and the council stops queueing at 128 orders. An order whose cell later becomes neither Air nor the target block is never ready and never removed, so it occupies a queue slot permanently. Only place() (construction.rs:229) removes community orders. [crates/app/src/world/farming/jobs/communities/development.rs:264] `if w.farming.jobs.build.len() >= 128 { break; }`
- A plan-change release path is missing from the reader's list: clear_builds wipes every build order and every claim of the player's crew in one call and resets all helper tasks. Cargo stays in hand and returns through Store. Changing the stockpile also invalidates claims indirectly, because matches_source compares the cargo source with the current stockpile (supply.rs:57). [crates/app/src/world/farming/jobs.rs:283] `world.farming.jobs.claims.clear();`

## Open questions
- Does a permanently rejected build cell (factory node anchor, protected orbital cell, out-of-bounds y) livelock a builder? The code collapses all Failure variants and reselects with no backoff, and the nearest owned order sorts first, but I did not run it.
- World::set_many_with was read only to line 2291. Can it leave side effects (environment refine or patch creation) for an edit that effects::replace then reports as Deferred?
- Is there any pruning of satisfied or obstructed build orders in community Jobs other than place()? I found only jobs.rs:739, which acts on the player's crew.
- The return value and failure semantics of inventory.remove_item in travel::trade were not read; the call ignores it after an earlier affordable() check.
- How felled trees become goods (expedition::harvest_log, sandbox bodies, drops) was not traced; woodlot conservation rests on that path.
- Not read in full: quarter/frontier/colony/housing.rs (household allocation), groups/model.rs (paid_blocks counter), planet_freight.rs, travel/civilization Ledger. The non-physical Ledger keeps food and materials as i32 counters, a second abstract economy beside the chests.
- No wage or per-worker payment flow was found. Person.experience, Person.grievance and groups' paid_blocks are counters. If a wage design exists elsewhere (docs or another branch) it is not in these files.
- When with_site swaps a community's Jobs into world.farming.jobs, outside readers see the player's crew only through communities.borrowed. Whether every external reader (UI, factory chest_io reservation lookup) handles that swap was not checked; chest_io reads world.farming.jobs directly.
- Mission return, migration and settle all wait in place and retry each wake when the target chest is full or its chunk is not resident, with no fallback. Whether this can strand a traveller indefinitely in practice is untested.
- supply/check.rs and its snapshot submodules restore world state by hand after probing. Whether those probes run in any default gate or only on request was not determined.

## Confirmed findings
- U2 The step unit is defined in engine_core: 1/120 s. [crates/core/src/time.rs:5]
- U3 farming.clock is the time base for agenda due, retry_at, next_meal, meal_ticks; it advances by 1 per farming::tick call (one fixed step). [crates/app/src/world/farming.rs:62]
- U4 Community meal period is 24,000 clock ticks (200 s at 120 Hz); council interval is 600 ticks (development.rs:8); DAY_TICKS is 480*120 (travel.rs:23). [crates/app/src/world/farming/jobs/communities/founding.rs:154]
- U5 Batch work is 12 executions of the task (unit: worker service calls, at most one per PULSE), not clock ticks. [crates/app/src/world/farming/jobs/production/batch.rs:16]
- V1 Verified starting fact: agenda is a BTreeSet of (due, site, person), serde-skipped and rebuilt by restore(); EVENTS_PER_TICK = 8 at communities.rs:17. [crates/app/src/world/farming/jobs/communities.rs:65]
- V2 Verified: workers are disabled when multiplayer is present (feature-gated). [crates/app/src/world/farming/jobs.rs:233]
- V3 Verified: the local crew lead is serviced directly at phase 0, helpers by phase slice, and only then the community agenda runs (jobs.rs:749). [crates/app/src/world/farming/jobs.rs:742]
- H1 Chest contents live in the global block_data.entries map keyed by position (not per chunk); every stockpile, hub and market seller is such a chest. Reads clone the slot vector. [crates/app/src/world/block_data.rs:101]
- H2 Each worker holds at most one carried stack (lead in State, helpers in crew::Worker.carry at crew.rs:31). [crates/app/src/world/farming/jobs.rs:69]
- H3 Carry is tagged with a purpose and source so it can be returned after interruption or reload: Build{source} or Supply{source,target,project}. [crates/app/src/world/farming/jobs/supply.rs:16]
- H4 A crafting/smelting batch takes custody of inputs by debiting the chest at acceptance; they then exist only in j.batch (serialised) until output is credited. [crates/app/src/world/farming/jobs/production/batch.rs:85]
- H5 Travellers carry a multi-stack pack inside the Mission (up to 12 stacks), separate from carry. [crates/app/src/world/farming/jobs/communities/transit.rs:22]
- H6 A settler's pack becomes Community.starter on arrival (transit.rs:401) and is credited to the new chest by founding::advance. [crates/app/src/world/farming/jobs/communities.rs:57]
- H7 Pantry stock and purse are bare u32 counters, not items in a chest; freight market/cargo and nursery_stock are likewise counters (freight.rs:150-157). [crates/app/src/world/farming/pantry.rs:60]
- H8 Claims are not a goods holder; the module states they never move stock. No module holds a combined ledger. [crates/app/src/world/farming/jobs/reservations.rs:1]
- M1 Chest to carry for construction: fetch debits the source chest via take_hauled for up to BATCH=8 units (construction.rs:7), limited by build_stock. [crates/app/src/world/farming/jobs/construction.rs:151]
- M2 Carry to chest: Store merges into the destination chest; any remainder stays in carry ('Stockpile full'). [crates/app/src/world/farming/jobs.rs:932]
- M3 World drop to carry: Haul removes the whole dropped stack, matched by exact f32 position. [crates/app/src/world/farming/jobs.rs:950]
- M4 Voxel to carry: mining replaces the block with Air and then creates one item in carry (assignment, not merge; guarded upstream by carry.is_none() in task_still_valid). [crates/app/src/world/farming/jobs.rs:977]
- M5 Hub supply: a courier takes up to LOAD=16 from the primary chest, or from a dropped stack (Recover), tagged Cargo::Supply. [crates/app/src/world/farming/jobs/supply/transport.rs:63]
- M6 Market funding moves exactly one payment item from the buyer stockpile to carry and records a shopping Trip. [crates/app/src/world/farming/jobs/market.rs:51]
- M7 Market purchase builds one new seller-chest image (goods -1, payment +1), then commits it and swaps carry to the goods. [crates/app/src/world/farming/jobs/market.rs:98]
- M8 Community custody primitive: exchange debits and credits a cloned slot image and writes it only if every leg succeeds. Used for mission launch, return, inter-town barter, raid and migration. [crates/app/src/world/farming/jobs/communities/custody.rs:45]
- M9 Player trade with a community commits the chest first; the player's inventory is changed afterwards by the caller. [crates/app/src/world/farming/jobs/communities/market.rs:61]
- M10 Factory chest IO draws from the same chests and honours the resident reservation at debit time. [crates/app/src/world/factory/chest_io.rs:112]
- M11 Regional politics debits community chests directly (withdraw for campaigns, via civilization.rs:240); deposit credits them back (communities.rs:399). [crates/app/src/world/farming/jobs/communities.rs:388]
- M12 Death moves carry to a DroppedStack at the body; when the drop list holds 1024 entries the stack goes to the player's SurvivalInventory.overflow instead (custody jumps to the player). [crates/app/src/world/farming/jobs.rs:496]
- R2 A cell can hold only one claim; reserve skips cells already claimed, so claims can be fewer than carried units. [crates/app/src/world/farming/jobs/reservations.rs:26]
- R4 Save-time invariant: a worker's claims never exceed its carried count (part of State::valid, checked on load). [crates/app/src/world/farming/jobs/reservations.rs:57]
- R5 Stock reservation for a funded project is derived, not stored: unreserved stock = chest count minus a recomputed requirement. [crates/app/src/world/farming/jobs/reservations.rs:83]
- R6 Task exclusivity (same order fetch/fund, same mine cell, same station, same drop, single supply courier) is derived from other helpers' current task, not stored. [crates/app/src/world/farming/jobs/crew.rs:317]
- R7 Meetings are stored pair reservations with two standing points; they are dropped when both sides finish or after TIMEOUT = 960*PULSE (routine.rs:12), and on interrupt, removal, transfer or death. [crates/app/src/world/farming/jobs/routine/social.rs:5]
- R9 Death releases the dead worker's claims in clear_worker, after dropping mission cargo and batch contents. [crates/app/src/world/farming/jobs.rs:488]
- R10 Moving a worker between owners (enlist, return home, settle, migrate) releases its claims and meetings in take_worker. [crates/app/src/world/farming/jobs/crew.rs:224]
- R11 Enlisting is refused while the resident holds cargo, a batch, a shopping trip or a mission, so custody never crosses owners mid-delivery. [crates/app/src/world/farming/jobs/communities.rs:301]
- R12 Follow, guard or mobilisation releases claims and clears building_material but keeps carry; the cargo is later stored back at its source. [crates/app/src/world/farming/jobs.rs:758]
- R13 Claims are deliberately held with no timeout while terrain is not resident; the worker returns before reconcile_claims. [crates/app/src/world/farming/jobs.rs:793]
- R14 When the stockpile block is no longer a chest, work() returns before reconcile_claims on every pulse, so existing claims and cargo stay pinned (same for unset or unloaded stockpile at 839-846). [crates/app/src/world/farming/jobs.rs:847]
- R16 Satisfied build orders are pruned only for world.farming.jobs at phase 0 of jobs::service. Community Jobs are swapped in only inside the agenda, so their queues lose orders only through place(). [crates/app/src/world/farming/jobs.rs:739]
- C1 place step 1: claim ownership and cargo-source check; failure sets a status and returns with nothing changed. [crates/app/src/world/farming/jobs/construction.rs:194]
- C2 place step 3: occupancy check against the player, live actors and this crew's saved positions; failure returns with nothing changed. [crates/app/src/world/farming/jobs/construction.rs:200]
- C3 workers_clear looks only at the crew of the Jobs currently swapped in; unembodied members of other owners (player crew versus a community, or two communities) are not checked. [crates/app/src/world/farming/jobs/construction.rs:185]
- C4 place step 4: the cell must still hold the expected block, be outside a protected base and have a solid neighbour; failure returns with nothing changed. [crates/app/src/world/farming/jobs/construction.rs:204]
- C5 place step 5 (paving only): refuses when the displaced block's drop cannot be stored in the 1024-entry drop list. [crates/app/src/world/farming/jobs/construction.rs:209]
- C6 place step 6: the voxel edit. Any Failure (Deferred, Conflict, Rejected, NoChange) is collapsed to one status and a return; carry, claim and order are untouched. [crates/app/src/world/farming/jobs/construction.rs:213]
- C7 place step 7: the inventory debit happens after the voxel commit and publication, with no fallible statement between them; the order and claim are removed after that (lines 229-230). [crates/app/src/world/farming/jobs/construction.rs:225]
- C8 effects::replace defers when the chunk is not resident; it never initiates I/O. [crates/app/src/world/effects.rs:82]
- C9 World::set can silently skip an edit; replace detects this by re-reading and returns Deferred, so the caller sees a failure rather than a phantom success. [crates/app/src/world/effects.rs:124]
- C11 Voxel without debit exists by design when founding a generated town: the workbench is placed and the chest is only debited for founder-supplied towns. The chest block and starting stock are likewise minted (founding.rs:86-95). [crates/app/src/world/farming/jobs/communities/founding.rs:114]
- C12 Readiness uses farming::known, which answers for non-resident chunks that were never edited by sampling synchronously; so 'ready' can be true where replace will defer. [crates/app/src/world/farming.rs:102]
- C13 Chest transfers gate on world.get(p), which for a non-resident edited chunk reads the stored region synchronously (world.rs:1825-1831). [crates/app/src/world/block_data.rs:329]
- I1 Jobs state (carry, cargo, claims, build, task, work, retry_at, batch, mission, shopping, sowing) is serialised as RON in the farming sidecar and rejected on load if State::valid fails. [crates/app/src/world/farming/sidecar.rs:239]
- I3 While carrying, the production Eat task is never valid; hunger reaches a builder only through the routine phase change above. [crates/app/src/world/farming/jobs/production.rs:219]
- I4 Cargo loaded without claims (legacy save, or goods bought at market with cargo None) gets claims allocated on resume, limited to unclaimed matching orders. [crates/app/src/world/farming/jobs/reservations.rs:41]
- I5 An interrupted batch is recovered as its inputs, or as its output if work finished but storage was full; recovery drops the stacks at the station (jobs.rs:469-475). [crates/app/src/world/farming/jobs/production/batch.rs:34]
- I6 Eight consecutive stalled pulses on a route release claims and send the cargo back, with a 2*PULSE retry delay (line 900). [crates/app/src/world/farming/jobs.rs:895]
- K1 Supply accounting is one pure formula (required minus stock minus in transit), covered by unit tests in the same file. [crates/app/src/world/farming/jobs/supply/accounting.rs:2]
- K2 The supply probe checks one borrowed unit through pickup, follow, guard, reload, death and recovery, then asserts source plus recovered equals the starting count. [crates/app/src/world/farming/jobs/supply/check.rs:116]
- K3 The livelihood check asserts meal conservation (chest + cargo + eaten == made), plus crop, timber and earthwork identities at lines 71-84, for one fixed scene. [crates/app/src/world/farming/jobs/production/check.rs:68]
- K4 The market check asserts payment items are neither lost nor created across buyer chest, seller chest and cargo. [crates/app/src/world/farming/jobs/quarter/market_check.rs:53]
- K5 The crew check asserts a rejected placement charges nothing and a player edit is not overwritten. [crates/app/src/world/farming/jobs/quarter/crew_check.rs:43]
- K6 In the transfer code itself the only total check is a debug assertion that a batch debit found all its inputs; release builds would under-debit silently if validation and debit ever diverged. [crates/app/src/world/farming/jobs/production/batch.rs:54]
- P1 'Paid' is a predicate over physical chest stock; funding a project debits nothing. [crates/app/src/world/farming/jobs/quarter/frontier/colony/growth/services/funding.rs:16]
- P2 Barter payment is an inventory item added to the seller's chest slots; no currency type exists in jobs. [crates/app/src/world/farming/jobs/market.rs:89]
- P3 The pantry purse is created from nothing at opening with 16 units, alongside 2 meals and 12 biomass; later it changes only through pantry::settle against player trades. [crates/app/src/world/farming/pantry.rs:119]
- P5 Goods are destroyed when a resident eats: one meal leaves the chest and only counters change. [crates/app/src/world/farming/jobs/production/actions.rs:85]
- P6 Travellers destroy one meal from their pack every 24,000 ticks; a traveller with no meal turns back. [crates/app/src/world/farming/jobs/communities/transit.rs:85]
- F1 Non-atomic: player trade. The community chest is already committed when the player's inputs are removed; the remove result is not checked and surplus output is dropped in the world. [crates/app/src/world/travel.rs:217]
- F2 Non-atomic: mission launch debits the source chest, then attaches the pack to the worker in a separate step (set_mission at line 197). [crates/app/src/world/farming/jobs/communities/transit.rs:192]
- F3 Non-atomic: sowing commits the sprout voxel first, then registers the plot and clears the carried seed (lines 55-56); failure of the edit keeps the seed. [crates/app/src/world/farming/jobs/production/actions.rs:52]
- F4 Non-atomic: mining commits the Air voxel, then creates the item; a failed edit is silently ignored and the task is reset. [crates/app/src/world/farming/jobs.rs:973]
- F5 Non-atomic: founding places the chest voxel, then writes its contents on the next line; the stock image is local until then, so a failed edit loses nothing. [crates/app/src/world/farming/jobs/communities/founding.rs:94]
- F6 Non-atomic: batch acceptance writes the debited chest, then stores the batch on the worker on the following line. [crates/app/src/world/farming/jobs/production/batch.rs:86]
- F7 Non-atomic: paving pushes the displaced block as a world drop after the voxel commit and before the carry debit. [crates/app/src/world/farming/jobs/construction.rs:221]
- F8 Non-atomic: the politics supply order removes items from the player first, then deposits; any part not stored is handed back on the next line. [crates/app/src/world/travel/civilization/politics.rs:351]