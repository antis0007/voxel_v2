# mp_capacity

## Summary
READ-ONLY survey of the files on disk (dirty tree, much of farming/jobs untracked). Nothing was built or run.

Units: `farming.clock` is a u64 that increments once per fixed step (farming.rs:62); the step is 1/120 s (engine_core::time::PLAYER_STEP_SECONDS). PULSE = (0.25 / step + 0.5) as u64 = 30 ticks = 0.25 s. All routine constants are multiples of PULSE; the idle wake (600) and council INTERVAL (600) are raw ticks = 5 s.

(a) Multiplayer. `world.multiplayer` is an `Option<ContextFrame>` that the host session sets at the top of each host step and takes at the end; it is never set on a joiner. `jobs::enabled` returns false whenever it is Some, and `jobs::service` returns on that check before the quarter, the local crew and `communities::service`. The host's actor pass is what calls `adventure::tick`, so the host is disabled too (the docs say so explicitly). In any network mode the local loop runs zero fixed steps, so a joiner never reaches `jobs::service`. `farming::tick` still runs, so the clock advances while agenda dues stand still. Same pattern: regional `civilization::service`/`summary`/politics, guard defence, `invite_to_base`, quarter fixture install, and `base::enabled` (claim stakes). Planet freight differs: it blocks only actor passes where `shared_tick` is false. The player sees the trade page without guard/settler/crew/priority controls (`if !network`), only "Follow · 1 Trailcake"; the only resident network command is Trade/Donate/Service/Accompany. Replication ships the whole farming sidecar (JOB1/COM1 included when used) and travel sidecar as image parts; there is no worker/resident delta or command. No doc states a reason; one plan lists prerequisites for enabling.

(b) Capacity. `crew::CAPACITY = 64` is a per-executor cap (one `Jobs` state: lead + 63 helpers), applied to the player's crew and to each community site separately. Uses: save validation (homes, helpers, people, wakes, ledger residents, meetings = CAPACITY/2, housing overrides), admission (add, recruit, enlist, migrate), the phase scheduler, UI and diagnostics. Separate hard-coded 64s that mean residents: groups PEOPLE = 64, travel homes <= 64 (decode, invite, constructor spawn), authored ids `id < 64`, fixtures requiring exactly 64, scene roster whitelist, woodlot residents 3..=64.

At 128/256:
- Local crew phase math depends on CAPACITY, not roster size: helper index i runs at phase floor((i+1)*30/CAPACITY). Raising it re-phases every existing helper (tick order changes even for small crews) and raises per-tick visits from at most 3 to about 5 (128) or 9 (256); the colony check's `peak<=3` then fails.
- Residents past the 64th silently get no work-circle record (groups PEOPLE).
- Authored scenes using ids 0..N write `travel.homes` unchecked, but decode rejects more than 64 homes and ids 64..1023, so the travel file would be ignored on load.
- The community agenda handles 8 events per tick and an active resident re-wakes after PULSE, so about 240 active wakes per PULSE across all sites; 256 busy residents exceed that, and unembodied movement is clamped to PULSE/30 substeps per wake, so backlog loses movement time.
- Validation limits only loosen, so old saves still load; saves written at a higher capacity fail on a lower-capacity build. JOB1 has a 256 KiB decode limit with no encode check.

Population ceiling today: MAX_SETTLEMENTS = MAX_SITES = 512; 64 people per site is an admission limit (32,768 nominal). Communities are only seeded from generated households (at most LOTS = 6 per site) and migration moves existing people, so the seeded ceiling is 512 x 6 = 3,072 plus the 10 constructor ids. Id space: originals in (1<<28)..(1<<28)+(1<<21) with low 3 bits < 6; extensions in (1<<27)..+(1<<25).

## Summary corrections (verifier)
Nothing was built or run; all checks are by reading the files on disk. 75 of 82 findings are confirmed as stated, 7 are imprecise, none are wrong. The summary needs these corrections:

1. Multiplayer is not the only disable. `jobs::enabled` also requires `civilization_enabled()` and `ca_view.is_none()` (jobs.rs:236). The summary presents the gate as multiplayer-only.

2. "All routine constants are multiples of PULSE" is too strong. It holds for routine.rs and groups/model.rs. The agenda uses raw ticks: 600, 120, 120 + site % 480, now + i + 1 and 2400 (communities.rs:253-257, transit.rs:206, crew.rs:265).

3. "Re-phases every existing helper" is slightly off. Helper indices 0 and 1 stay at phase 0 for 64, 128 and 256. The peak-visit figures (3, 5, 9) are correct by hand arithmetic against check.rs:19-25. That check only runs on the VOXEL_CITY_COLONY scenario path.

4. "64 people per site is an admission limit" hides that `people` keeps dead residents. The cap is on identity records ever held by the site, not on living population. A site with deaths has fewer than 64 live slots.

5. The agenda throughput point is misattributed to CAPACITY. The 8-per-tick budget is global across sites and shared with founding and council events. It can already be exceeded today with more than 240 active residents across sites (for example 41 sites of 6). The player's crew never uses the agenda, so raising CAPACITY for it only affects phase slicing.

6. "Validation limits only loosen, so old saves still load" is true for the CAPACITY-derived checks. It omits non-derived limits that would reject larger rosters: groups PEOPLE = 64 (silent drop), quarter housing_demand <= 64 (invalid record), and travel homes <= 64. A failure discards the entire farming or travel file on disk load, and returns an error in replication.

7. The extension-id worry (B47, B48 and the matching open question) is unfounded. The serial has 7 free bits because settlement ids have their low 3 bits clear, so `next_resident < 128` cannot collide.

8. Two of the reader's open questions are answerable from the code. designate and blueprint queueing sit inside the local step loop or behind `!network`, so a host cannot mutate Jobs through them. Resident bodies reach joiners in the ADVENTURE image section, with a 512-enemy budget.

9. The travel sidecar accepts civilization resident ids (households::role falls through to civilization::role). The "64..1023 rejected" statement is correct, but the summary's list of accepted ids is incomplete.

10. A15 and A16 describe checks that are effectively unreachable from their known callers, because world.multiplayer is None outside the host step. The player-facing gate for move-in is the UI `network` flag.

The population ceiling stands as stated: MAX_SETTLEMENTS = MAX_SITES = 512, at most 6 generated households per site, so 3,072 community identities plus 10 constructor ids. The statement that no doc records a reason for the multiplayer disable also stands. One further stale doc exists: docs/design/RESIDENT_GAMEPLAY_AUDIT_20261029.md:25 says multiplayer/actions.rs has no trade/service request, which the Resident command at actions.rs:17 now contradicts.

## Checks: 75 confirmed, 7 not

- [imprecise] A15 (crates/app/src/world/farming/jobs/quarter/fixture.rs:11) claim: Quarter fixture install refuses under a multiplayer context with a player-visible error.
  correction: The refusal exists at crates/app/src/world/farming/jobs/quarter/fixture.rs:10-11, but line 8 returns first: `if super::frontier::enabled() {return super::frontier::install(w,p);}` (VOXEL_CITY_START_SITE=1, frontier.rs:96), and frontier::install has no multiplayer check. Callers found are world setup (crates/app/src/main.rs:1575, crates/app/src/egui_ui/goals_hud/review.rs:130), outside host_frame's step where world.multiplayer is None (multiplayer.rs:2248). Whether the branch is ever reached, and whether the Err text is shown to a player, was not established.
- [imprecise] A16 (crates/app/src/world/travel/services.rs:12) claim: invite_to_base (resident move-in) refuses under a multiplayer context.
  correction: The check is at crates/app/src/world/travel/services.rs:11 (line 12 is the return). The only caller is crates/app/src/egui_ui/trade_page.rs:179, inside the `else` of `if network` (trade_page.rs:168) and at UI time, when world.multiplayer is None. The effective gate in a session is the UI `network` flag, not this check.
- [imprecise] B4 (crates/app/src/world/farming/jobs.rs:31) claim: PULSE = 30 ticks at 120 Hz (0.25 s); every routine constant is a multiple of it.
  correction: PULSE = 30 ticks at 120 Hz (0.25 s) is right (crates/app/src/world/farming/jobs.rs:31). 'Every routine constant is a multiple' holds for routine.rs:9-12 (16, 12, 240, 960 x PULSE) and groups/model.rs:7-8 (16, 240 x PULSE). The agenda side uses raw ticks: idle 600 (jobs/crew.rs:265), council INTERVAL 600 (communities/development.rs:8), founding retry 120 and first council 120 + site % 480 (communities.rs:254, 257), post-founding stagger now + i + 1 (communities.rs:253), transit cooldown 2400 (communities/transit.rs:206).
- [imprecise] B6 (crates/app/src/world/farming/jobs/crew.rs:9) claim: Scheduling-rate assumption: helper index i is serviced at phase floor((i+1)*PULSE/CAPACITY). Changing CAPACITY re-phases every helper, including in crews of 64 or fewer.
  correction: Formula is right (crates/app/src/world/farming/jobs/crew.rs:9), but not every helper is re-phased: index 0 and 1 give phase 0 at CAPACITY 64, 128 and 256 (30/C and 60/C floor to 0). Index 2 and above move. service_phase is called only by the colony check (jobs/quarter/frontier/colony/check.rs:22); the runtime uses phase_indices (jobs.rs:746).
- [imprecise] B47 (crates/worldgen/src/civilization.rs:633) claim: Extension ids give 4 bits of serial per settlement (EXTENSION_LOTS = 16 at line 629).
  correction: The serial field is effectively 7 bits, not 4. Settlement ids have their low 3 bits clear (crates/worldgen/src/civilization.rs:475 `id & !7`), so `(settlement - BASE) << 4` leaves bits 0..6 zero and a serial of 0..127 fits without touching another settlement's bits (civilization.rs:633). EXTENSION_LOTS = 16 (line 629) is the number of extension plots, not the serial width.
- [imprecise] B48 (crates/app/src/world/travel/civilization.rs:513) claim: The legacy ledger allocates extension serials while next_resident < 128, which is wider than the 4-bit serial field in B47.
  correction: Line and quote are right (crates/app/src/world/travel/civilization.rs:513), and it applies only to non-physical ledgers (`if ledger.physical { ... continue; }` at lines 445-453). The inference is wrong: `next_resident < 128` matches the 7 free serial bits exactly (see B47), so serials 16..127 do not collide with other settlements' ids. Concurrent extension residents per settlement are still bounded by the 16 lots (civilization.rs:190, 199).
- [imprecise] B50 (crates/worldgen/src/households.rs:19) claim: Older household ids 1024..1024+262144 (and 3..7) also count as residents for the travel sidecar via households::role.
  correction: Lines 18-19 are quoted correctly, but crates/worldgen/src/households.rs:20 falls through to `crate::civilization::role(id)`, which returns Some for every civilization resident id (crates/worldgen/src/civilization.rs:470-472). So the travel sidecar also accepts original ids (1<<28 range) and extension ids (1<<27 range) for homes and guard posts; crates/app/src/world/travel/sidecar/resident_tests.rs:5 pins (1 << 28) + 1 as valid.

## Missed (verifier)
- jobs::enabled is not only a multiplayer switch. It also returns false when the planet's civilization mode is not Living or when world.ca_view is set (the --ca-cpu-region headless path). The same two conditions gate travel/civilization.rs:307, which additionally requires generator_version >= geo::VERSION and a Planet world. A shadow harness on the CA-region path would see workers disabled for a non-multiplayer reason. [crates/app/src/world/farming/jobs.rs:236] `_world.sample_config().civilization_enabled() && _world.ca_view.is_none()`
- The per-site cap counts identity records, not living residents. mark_dead only sets died_at; the Person stays in `people` (removal happens only on migration, transit.rs:385/436). Validation (communities.rs:94) and migration admission (transit.rs:230, 414) use people.len(), so dead and enlisted residents consume the 64 slots. The travel ledger, by contrast, removes the dead (travel/civilization.rs:282). [crates/app/src/world/farming/jobs/communities.rs:342] `p.died_at = Some(w.farming.clock);`
- A separate hard 64 would invalidate saves if a colony crew exceeds 64: housing_demand is set to workers minus housed with no cap (colony.rs:15), but quarter::State::valid requires housing_demand <= 64 (quarter.rs:94). Jobs::valid includes the quarter (jobs.rs:196), so more than 64 unhoused workers makes the JOB1 record invalid on reload. [crates/app/src/world/farming/jobs/quarter/frontier/colony.rs:15] `city.housing_demand = w.farming.jobs.production.as_ref().map_or(0, |p| p.workers.len().saturating_sub(housed)) as u32;`
- Any JOB1 or COM1 validation failure (oversized record, helpers > MAX_HELPERS, people > CAPACITY, housing_demand > 64) discards the whole farming file on disk load, including clock, plots, base claims and all communities, not only the worker section. [crates/app/src/world/farming/sidecar.rs:280] `Err(e) => eprintln!("[farming] Ignoring {}: {e:#}", path.display()),`
- In multiplayer replication the same decoders are not 'report and ignore': a farming or travel sidecar that fails validation (for example more than 64 travel homes, B30) returns an error from the joiner's image decode via `?`. What the session does with that error was not traced. [crates/app/src/multiplayer/replication.rs:323] `let travel = part(10).map(world::travel::sidecar::decode).transpose()?;`
- Resident bodies do reach joiners: the whole Adventure state (enemies, which include villager actors with site.z == 8) is serialized into the ADVENTURE section at replication.rs:154, and the joiner rejects images with more than 512 enemies. This answers the reader's open question about resident bodies and sets an embodied-actor ceiling for replication. [crates/app/src/multiplayer/replication.rs:303] `ensure!(others.len() <= 16 && a.enemies.len() <= 512 && a.projectiles.len() <= 4096, "gameplay entity budget exceeded");`
- The reader's open question on jobs::designate is answerable: the call sits inside the local fixed-step loop (opened at main.rs:4926, step_trace created at 4927, closed at 5808 after `step_trace.mark("gameplay-step-tail")`), which runs zero iterations in network mode. queue_blueprint is separately gated by `!network` at creative/blueprint.rs:185, and blueprints.frame (main.rs:5457) is inside the same loop. So neither path mutates Jobs while hosting or joined. [crates/app/src/main.rs:5479] `&& world::farming::jobs::designate(world, cell)`
- The 8-events-per-tick budget is shared by resident wakes, FOUNDING (u32::MAX) and COUNCIL (u32::MAX - 1) events across all sites, taken strictly in (due, site, person) order from the BTreeSet. Council and founding sort after every person of the same site at equal due time, and a lower site id always wins ties. [crates/app/src/world/farming/jobs/communities.rs:245] `let Some((due, site, person)) = w.farming.communities.agenda.first().copied().filter(|e| e.0 <= now) else { break; };`
- Agenda timing uses raw 120 Hz tick constants that are not PULSE multiples in origin: founding retry 120 ticks (1 s), first council at 120 + site % 480 ticks, first resident wakes staggered at now + i + 1, and transit policy cooldown 2400 ticks (20 s, transit.rs:206). A kernel that reproduces ordering needs these exactly. [crates/app/src/world/farming/jobs/communities.rs:254] `.chain([(COUNCIL, now.saturating_add(120 + u64::from(site % 480)))])`
- Task search depends on service cadence and identity: the broad search runs when (clock / PULSE + worker id) is a multiple of 7. Under phase slicing each local worker is visited once per PULSE; agenda-driven community residents are visited at arbitrary ticks, so the same rule fires on a different schedule. This is a scheduling-rate assumption not tied to CAPACITY but changed by any re-phasing. [crates/app/src/world/farming/jobs.rs:836] `let broad=(world.farming.clock/PULSE+u64::from(j.worker.unwrap_or(0))).is_multiple_of(7);`
- Production stock targets saturate with roster size: the per-crop harvest threshold is workers * 4 clamped to 32..128, so it stops growing at 32 workers (same clamp at production.rs:233). At 128 or 256 workers the target is 1 or 0.5 items per worker. production::State::valid separately allows up to 1024 workers (production.rs:92), so this is not a validation failure, only a behaviour change. [crates/app/src/world/farming/jobs/production.rs:176] `< (s.workers.len() as u32 * 4).clamp(32, 128) { tasks.push(Job::Produce(Task::Harvest(b))); }`
- Colony housing writes travel.homes for every household member without the 64-home check that services.rs:37 and crew.rs:171 apply. With a 64-resident colony plus previously spawned constructors, homes can exceed 64 and the travel file then fails decode (B30). Reachability beyond the colony fixture was not traced. [crates/app/src/world/farming/jobs/quarter/frontier/colony/housing.rs:109] `for id in &h.members { w.travel.homes.insert(*id, target); }`

## Open questions
- No comment or doc records WHY workers are disabled in multiplayer. Docs only state the boundary and list prerequisites (docs/plans/2026-11-06-city-quarter-slice.md:56-59). The reason is not recorded.
- world.multiplayer is None outside the host step (set at multiplayer.rs:2204, taken at 2248), so enabled() is true for UI-time calls while hosting. jobs::designate (main.rs:5479) and queue_blueprint (creative/blueprint.rs:193, via blueprints.frame which receives network_mode at main.rs:5457) were not traced far enough to say whether a host can still mutate Jobs state through them.
- I did not read a joiner's session step in full. That joiners never run adventure::tick rests on: the only call sites are simulation/fixed.rs:146 and multiplayer/authority.rs:81, main.rs:4926 runs zero local steps in network mode, and the comment at multiplayer.rs:2230. Not confirmed by reading the join path.
- What a joiner sees of resident bodies: farming/travel sidecars are replicated whole, but whether villager actors (adventure.enemies) and their poses reach joiners, and whether vehicle::service spawns or retains residents on the host in multiplayer, was not traced.
- base.rs:204 (second use of base::enabled) was not read; I only know it also checks head equipment.
- Actual serialized size of a 64-worker JOB1 record versus the 256 KiB decode limit is unknown; it needs a measurement, not a build in this tree.
- COM1 allows 32 MiB (farming/sidecar.rs:259) while multiplayer/gameplay.rs:6 sets MAX_IMAGE = 16 MiB. Whether a large communities section can exceed the image limit in replication was not checked.
- Whether embodied resident actors are bounded by MAX_ENEMIES (24, with +64 allowances at adventure.rs:3578 and 3630) was not determined; vehicle.rs has no MAX_ENEMIES reference, so the embodied-actor ceiling for 128/256 residents is unknown.
- B48: the legacy ledger allows next_resident up to 127 but extension_resident ORs the serial into a 4-bit field. Whether serials of 16 or more are reachable (lots are capped at EXTENSION_LOTS = 16 but vacancies may refill) and collide with another settlement's ids was not traced.
- The id layout allows (key.x+64) up to 10 bits under MAX_RESIDENT_ID, while regional_charter masks 7 bits (travel/civilization.rs:85). The real range of region keys the generator emits was not checked, so the count of reachable settlement ids is unknown.
- The 3,072 seeded ceiling assumes no code path mints new community residents. Grep found people.insert only in founding.rs:26 and transit.rs:387/397/449 (moves); development.rs was not read in full.
- The peak-visit figures for CAPACITY 128 (5) and 256 (9) in B9 are my arithmetic from crew.rs:9-14, not a run of the check.

## Confirmed findings
- A1 jobs::enabled returns false whenever world.multiplayer is Some; no host/client distinction in the check itself. [crates/app/src/world/farming/jobs.rs:233]
- A2 jobs::service returns on !enabled before quarter::service, the local crew phase slice and communities::service, so every worker path is skipped. [crates/app/src/world/farming/jobs.rs:728]
- A3 world.multiplayer is set only by the host session at the start of each host step (actor HOST); grep found no other assignment except None in open.rs:290. [crates/app/src/multiplayer.rs:2204]
- A4 The context is removed at the end of each host step, so code called outside the step (UI) sees world.multiplayer == None even while hosting. [crates/app/src/multiplayer.rs:2248]
- A5 jobs::service is called from adventure::tick under shared_tick; with the context set (host step) it reaches enabled() and returns, so the host is disabled too. [crates/app/src/world/adventure.rs:3562]
- A6 farming::tick still runs on the host's shared tick, so farming.clock advances in multiplayer while agenda dues are not serviced. [crates/app/src/world/adventure.rs:3560]
- A7 In any network mode (host or joiner) the local loop runs zero fixed steps; the session steps the world instead. [crates/app/src/main.rs:4926]
- A8 The only multiplayer call site of adventure::tick is authority::step, invoked from the host step loop (multiplayer.rs:2211, 2231). [crates/app/src/multiplayer/authority.rs:81]
- A9 communities::service (the agenda) has its own enabled() guard, as does communities::register at line 152. [crates/app/src/world/farming/jobs/communities.rs:241]
- A10 Regional civilization enabled() is false under a multiplayer context; it gates civilization::service (line 840), summary (915) and politics (politics.rs:264, 386, 418). [crates/app/src/world/travel/civilization.rs:304]
- A11 base::enabled is false under a multiplayer context; it gates claim-stake interaction (interaction.rs:99, 148) and base.rs:204. Not worker logic. [crates/app/src/world/farming/base.rs:111]
- A12 Planet freight recover/collect are blocked only for actor passes where shared_tick is false, unlike the blanket is_some() checks. [crates/app/src/world/farming/planet_freight.rs:565]
- A13 shared_tick is true for the first actor processed in a step (the host player, or the first ready joiner on a headless host) and false for later actors. [crates/app/src/multiplayer.rs:2221]
- A14 Player-visible text when a non-shared-tick actor uses the freight market counter. [crates/app/src/world/farming/planet_freight.rs:683]
- A17 Stationed resident guards do not defend under a multiplayer context: defend returns empty goals. [crates/app/src/world/adventure/settlement_guard.rs:21]
- A18 The trade page hides guard, Assign Settler, crew count, recruit, priorities and Clear Build Orders when network is true; line 168 then offers only Follow/Stay. [crates/app/src/egui_ui/trade_page.rs:123]
- A19 network is true for every non-Single mode (Opened, Hosted, Joined), so the host loses the worker UI too. [crates/app/src/multiplayer/coop.rs:24]
- A20 The only resident-related network command carries a travel::ResidentAction (Trade, Donate, Service, Accompany; travel.rs:162-167). No assign, designate, priorities or recruit command exists. [crates/app/src/multiplayer/actions.rs:17]
- A21 Replication ships the whole farming sidecar as image part 9 (travel sidecar is part 10 on the next line); no worker/resident delta stream was found under multiplayer/. [crates/app/src/multiplayer/replication.rs:173]
- A22 The farming sidecar includes the local Jobs state (JOB1) when used, so replicated images carry frozen worker state; COM1 communities likewise at line 121. [crates/app/src/world/farming/sidecar.rs:102]
- A23 Doc states the boundary includes the host; it gives no reason. [docs/plans/2026-11-06-city-quarter-slice.md:50]
- A24 Same doc lists prerequisites for enabling (lines 56-59): host-admitted commands, stable identities, revision checks, host-owned reservations, deltas, reconnect snapshots. [docs/plans/2026-11-06-city-quarter-slice.md:56]
- A25 Civilization implementation record confirms the restriction is still in place, without a reason. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:44]
- A26 Review doc records the disable as deliberate and local-only; no reason given. [docs/reviews/2026-11-06-city-livelihood.md:26]
- A27 ADR 0043 says its tests do not establish civilization support in multiplayer. [docs/adr/0043-network-transfer-fairness.md:25]
- B1 CAPACITY = 64: per-executor cap (one Jobs state = lead + helpers), applied to the player's crew and to each community site separately. [crates/app/src/world/farming/jobs/crew.rs:4]
- B2 MAX_HELPERS derives from CAPACITY (63); the lead worker lives in the Jobs state itself. [crates/app/src/world/farming/jobs/crew.rs:5]
- B3 Time unit: fixed step is 1/120 s. [crates/core/src/time.rs:5]
- B5 farming.clock counts fixed steps: +1 per farming::tick. [crates/app/src/world/farming.rs:62]
- B7 phase_indices is the inverse slice used at runtime; about CAPACITY/PULSE helpers per tick (2.13 at 64, 4.27 at 128, 8.53 at 256). [crates/app/src/world/farming/jobs/crew.rs:12]
- B8 The local crew is serviced by phase slice, outside the community agenda; the lead is serviced at phase 0 (line 742). [crates/app/src/world/farming/jobs.rs:746]
- B9 Colony check requires at most 3 worker visits per tick. By my arithmetic the peak is 5 at CAPACITY 128 and 9 at 256, so this check would fail. [crates/app/src/world/farming/jobs/quarter/frontier/colony/check.rs:25]
- B10 Save validation, per Jobs state: homes map capped at CAPACITY. [crates/app/src/world/farming/jobs.rs:180]
- B11 Save validation, per Jobs state: helpers capped at MAX_HELPERS. Raising CAPACITY loosens it; a save written at a higher capacity fails on a lower one. [crates/app/src/world/farming/jobs.rs:188]
- B12 Save validation, per community site: people capped at CAPACITY (per-site cap). [crates/app/src/world/farming/jobs/communities.rs:94]
- B13 Save validation, per community site: wake records capped at CAPACITY. [crates/app/src/world/farming/jobs/communities.rs:113]
- B14 Admission: enlisting a community resident into the player's crew is refused when the local crew is at CAPACITY. [crates/app/src/world/farming/jobs/communities.rs:302]
- B15 Admission: crew::add refuses at MAX_HELPERS (also refuses when !enabled, i.e. in multiplayer). [crates/app/src/world/farming/jobs/crew.rs:131]
- B16 Admission: constructor recruit loop stops adding at CAPACITY. [crates/app/src/world/farming/jobs/crew.rs:169]
- B17 put_worker only debug-asserts the cap; release builds rely on callers (enlist, migrate, founding) to check. [crates/app/src/world/farming/jobs/crew.rs:238]
- B18 Migration policy skips target sites already at CAPACITY (per-site cap). [crates/app/src/world/farming/jobs/communities/transit.rs:230]
- B19 Migration arrival rechecks the per-site cap before transferring the worker. [crates/app/src/world/farming/jobs/communities/transit.rs:414]
- B20 Meetings validation is parameterised by CAPACITY (limit CAPACITY/2 = 32 pairs, policy.rs:91). [crates/app/src/world/farming/jobs/routine.rs:19]
- B21 Runtime meeting creation is refused at CAPACITY/2 concurrent meetings per Jobs state. [crates/app/src/world/farming/jobs/routine/social.rs:12]
- B22 Travel sidecar validation: a settlement ledger's resident set is capped at CAPACITY (per-site cap in a different save file). [crates/app/src/world/travel/civilization.rs:184]
- B23 UI displays CAPACITY as the crew denominator; diagnostics do the same at jobs.rs:1245. [crates/app/src/egui_ui/trade_page.rs:140]
- B24 Colony housing overrides are validated and admitted against CAPACITY (also lines 81, 86, 123). [crates/app/src/world/farming/jobs/quarter/frontier/colony/housing.rs:29]
- B25 Test fixture: the colony scene roster is whitelisted to 12/24/32/48/64, so 128/256 cannot be requested without an edit; line 18 also rejects count > CAPACITY. [crates/app/src/world/farming/jobs/quarter/frontier/colony/scene.rs:10]
- B26 Separate hard-coded 64 for residents: work-circle people per Jobs state (bonds <= 128, offers <= 64). Not tied to CAPACITY. [crates/app/src/world/farming/jobs/groups/model.rs:4]
- B27 Past 64 group records a new resident is silently not registered, so contact/bonds/circles skip them if CAPACITY exceeds 64. [crates/app/src/world/farming/jobs/groups/model.rs:84]
- B28 Id-space assumption: authored scene residents use ids 0..64; the comment says this is independent of crew capacity. [crates/app/src/world/travel/sidecar.rs:13]
- B29 Travel sidecar resident_id accepts id < 64, the constructor ids, or ids with a households role; 64..1023 are rejected (resident_tests.rs:19 pins 64 and 1023 as invalid). [crates/app/src/world/travel/sidecar.rs:14]
- B30 Global cap: travel decode rejects more than 64 resident homes. Encode (line 75) writes homes.len() uncapped, and load() ignores the whole file on a decode error. [crates/app/src/world/travel/sidecar.rs:129]
- B31 Global cap: inviting or following a resident is refused once 64 homes exist. [crates/app/src/world/travel/services.rs:37]
- B32 Constructor spawning is skipped once travel.homes holds 64 entries. [crates/app/src/world/farming/jobs/crew.rs:171]
- B33 The colony scene inserts one travel home per authored resident id (3..count) with no cap check; a roster over 64 would produce a travel file that decode rejects (B29, B30). [crates/app/src/world/farming/jobs/quarter/frontier/colony/scene.rs:90]
- B34 Community agenda throughput is a global 8 events per tick across all sites (loop at line 244). [crates/app/src/world/farming/jobs/communities.rs:17]
- B35 An active resident's next wake is now + PULSE, so the agenda sustains at most 8*30 = 240 active wakes per PULSE; 256 busy residents exceed it. [crates/app/src/world/farming/jobs/crew.rs:256]
- B36 Idle residents back off to 600 ticks (5 s) when the Jobs state has homes, else PULSE. [crates/app/src/world/farming/jobs/crew.rs:265]
- B37 Elapsed movement time per service is clamped to PULSE, so agenda backlog loses unembodied movement rather than catching up. [crates/app/src/world/farming/jobs.rs:788]
- B38 advance_worker hard-codes 30 substeps (the PULSE value) per call. [crates/app/src/world/adventure/wildlife/vehicle.rs:261]
- B39 Local Jobs state has a 256 KiB RON decode limit with no matching encode check; a larger crew grows this record. [crates/app/src/world/farming/sidecar.rs:237]
- B40 MAX_SETTLEMENTS = 512 (travel ledgers); enforced on decode at travel/sidecar.rs:154 and at discovery (civilization.rs:850-875). [crates/app/src/world/travel/civilization.rs:12]
- B41 Community sites share the same ceiling: MAX_SITES = MAX_SETTLEMENTS = 512, so the nominal admission ceiling is 512 * 64 = 32,768. [crates/app/src/world/farming/jobs/communities.rs:16]
- B42 Communities are populated only from generated households (ids site.id..site.id+households); transit.rs moves existing people, and no other people.insert creates a new identity. [crates/app/src/world/farming/jobs/communities/founding.rs:12]
- B43 Generated households per site are capped by LOTS = 6 (line 18: const LOTS: u32 = 6), giving a seeded ceiling of 512 * 6 = 3,072. [crates/worldgen/src/civilization.rs:438]
- B44 original_settlement: id in BASE(1<<28)..BASE+(1<<21) with low 3 bits < LOTS; the settlement id is the id with the low 3 bits cleared. [crates/worldgen/src/civilization.rs:475]
- B45 resident_id: an original id, or an extension id in (1<<27)..(1<<27)+(1<<25). [crates/worldgen/src/civilization.rs:479]
- B46 Original id layout: region x in bits 11+, region z in bits 4..10, slot in bit 3, lot in bits 0..2. [crates/worldgen/src/civilization.rs:467]
- B49 Constructor ids are a separate reserved range of 10 starting at 0x6000_0000 (CONSTRUCTORS = 10 at line 6). [crates/app/src/world/farming/jobs/crew.rs:7]
- B51 Quarter state validation has its own resident ceiling of 256 and housing_demand <= 64 (line 94). [crates/app/src/world/farming/jobs/quarter.rs:95]
- B52 Test fixture: routine growth check requires exactly 64 workers; the same pattern is in neighborhood.rs:55, services.rs:42 and replay.rs:17. [crates/app/src/world/farming/jobs/routine/check/growth.rs:34]
- B53 Woodlot content validation hard-codes residents 3..=64. [crates/app/src/world/farming/jobs/production/woodlot.rs:28]
- B54 Design record calls 512 sites and 64 per town admission limits, not demonstrated capacity. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:29]
- B55 crew::planned scans every helper per candidate task, so task selection cost grows with roster size times candidates. [crates/app/src/world/farming/jobs/crew.rs:316]