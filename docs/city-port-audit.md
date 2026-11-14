# Axiom city simulation: port audit

Audit date: 7 November 2026. Updated after verified shared daily life, serial funded infill and exact worker reload acceptance. Each implemented result below is limited to its tested scenario.

## Scope and findings

- Prototype: `C:\Users\antis0007\Downloads\Java\civ_proto`. This is a dependency-free Rust library: 26 feature modules plus `lib.rs`, with 128 inline test annotations counted during inspection. It has no executable or benchmark target. The surrounding Java folder contains design/reverse-engineering documents and a reference JAR, not a completed portable city game.
- Private engine: `C:\Users\antis0007\Documents\Codex\2026-11-06\task\axiom-city-slice`.
- Shared engine: `C:\dev\voxel_v2_backup`. The 64-resident colony, six paid houses, household assignments and meal/rest/social/work loop were subsequently guarded-integrated and tested there. The funded two-home infill and district/roof additions are guarded-integrated and verified on the actual shared-source executable as well.
- No direct `civ_proto` import was found. Existing capabilities below are functional counterparts, not claims of code ancestry or complete feature parity.
- Physical construction, crops, recipes, cargo, meals, resident identities, saving and networking already have engine owners. Port decisions and missing state into those owners; do not install duplicate authorities.
- The integrated daily-life fixture verifies physical travel to meals, home rest and social visits for 24 housed residents among 64 live workers. The serial housing gate verifies eight additional occupants, keeping all original homes and continuing all 24/28 prior household routines during each build. This is not family formation, interpersonal relationships, immigration or an unrestricted society model.
- Prototype `coop.rs` contains authority predicates and a hard-coded summary size, not transport. Engine transport/session/netcode implementations exist; society authority, late join and replication remain unverified.
- Neither this audit nor the available benchmark establishes Songs of Syx performance or feature parity.

Paths in the module map are relative to the private engine. `W/` abbreviates `crates/app/src/world/`; `J/` abbreviates `W/farming/jobs/`. Prototype filenames are relative to `civ_proto/src/`.

## All 26 prototype modules

| Module | Actual engine counterpart | Status and reuse strategy |
|---|---|---|
| `det.rs` | `crates/core/src/hash.rs::{hash_u32,hash01}`; `core/src/time.rs::{TickId,Budget}` | Existing equivalent. Use engine hashing/ticks, not a second randomness or time owner. |
| `pawn.rs` | `W/farming/jobs.rs`; `J/crew.rs`; `J/reservations.rs`; `J/production.rs` | Existing physical jobs/priorities/cargo/meals. A persistent bounded need loop now drives housed residents through rest, social visits and work return. Skills and richer motivations remain missing. Do not copy the prototype task board. |
| `brain.rs` | `W/travel/civilization.rs::Ledger`; `civilization/politics.rs::{command,service}` | Compatible intent/decision code, missing adapter. `Brain::think` may propose actions from authoritative snapshots; debit inventory/treasury only through the existing committing system. Its independent reserve must not become duplicate stock. |
| `budget.rs` | `jobs::service`; `crew::service_phase`; bounded/rotating path search | Existing scheduler. Prototype selection scans, allocates and sorts the whole eligible pool. Reuse deterministic relevance/tie rules, not that implementation as a scale solution. |
| `population.rs` | `W/travel/civilization.rs::{residents,present,home,advance_journeys}`; `W/adventure/wildlife/vehicle.rs` | Persistent identities and regional journeys exist. Individual lifecycle/event state remains incomplete. Prototype `Schedule::due` scans everyone; `interrupt` does not make `next_event` due. Preserve current IDs and actor materialization. |
| `household.rs` | `J/quarter/frontier/colony/housing.rs`; `crates/worldgen/src/households.rs` | Integrated completion/capacity/assignment/override and physical home-rest routines; generated household IDs/homes also exist. Bounded funded growth adds two generated four-bed homes serially using real stock and preserves previous assignments. Attachment, decoration, family relationships and relocation policy remain unintegrated. Prototype `consider_move` only returns a recommendation. |
| `city.rs` | `J/quarter/layout.rs::{Plan,Parcel,generate}`; `quarter/geometry.rs`; `crates/worldgen/src/civilization/fabric.rs` | Physical parcel/street/building planning exists. Verified infill uses generated provenance, paid street anchors and the existing router. Reuse broader purpose/tier/district rules as generator inputs. Prototype relationships lack coordinates or navigation and cannot replace real geometry. |
| `churn.rs` | Existing quarter lots/projects and colony housing | Redevelopment policy is unintegrated. Prototype instantly replaces abstract buildings without payment, salvage or relocation. Adapt candidate scoring and household resistance; execute demolition/rebuild as real jobs. |
| `construction.rs` | `J/construction.rs::{fetch,place,queue_blueprint}`; `J/reservations.rs`; `quarter/frontier.rs` | Stronger physical implementation already exists. Reuse technique gates and invariants, not scalar material reservations. Prototype cancellation can refund completed projects. |
| `economy.rs` | `J/production.rs::{candidates,craft_ready,work_priority}`; `production/actions.rs::{transformed,execute}`; `W/block_data.rs`; inventory recipes | Actual conserved production and transfers exist. Reuse bottleneck calculation for diagnostics. Do not create a parallel economic stock ledger for the colony. |
| `settlement.rs` | `W/travel/civilization.rs::Ledger/advance`; `civilization/politics.rs` | Regional aggregate growth/migration/ownership exists; colony bridge is missing. Aggregate regional resources must remain distinguishable from chest contents. Do not mint physical actors from a second population count. |
| `history.rs` | `crates/worldgen/src/civilization.rs::History`; generated fabric; travel journeys | Generated founding/flood/abandonment history exists. Runtime causal route/disaster consequences need integration. Adapt event semantics into actual edits, journeys and ownership records. |
| `memory.rs` | `W/adventure/records.rs`, `chronicle.rs`, discovery | Existing records are not equivalent NPC beliefs. Bounded facts/confidence/staleness code is reusable, keyed to existing resident IDs; persistence and decision consumers are missing. |
| `policy.rs` | Existing job priority/candidate selection | Utility baseline is reusable. Neural policy is conceptual: illustrative operation counts are not inference, and development update ignores its delta. Establish measurable deterministic utility behavior before adding learned policy machinery. |
| `politics.rs` | `W/travel/civilization/politics.rs::{State,Campaign,command,service,actors,wound}` | Treaties, supplied expeditions and siege exist. Reuse deal/casus/reputation evaluation over that state, not a second treaty/campaign database. |
| `territory.rs` | Civilization charter ownership and politics state | Influence calculation is reusable; authoritative ownership integration missing. Use influence for proposals/contested presentation, not independent ownership writes. |
| `fortify.rs` | Civilization politics siege; `W/adventure/settlement_guard.rs::defend`; world effects | Partial capability exists. Fort grade/gate/repair state needs actual structure, navigation and paid-repair bindings. Abstract HP alone is not a functioning fortification. |
| `battle.rs` | `W/adventure/{engine,tactics,bestiary}.rs`; campaign actors | Prototype team/unit/budget validation is reusable scenario preparation. It does not simulate battle. Connect validated rosters to current actor/combat systems. |
| `counters.rs` | Bestiary/content roles and actual combat damage evaluation | Unintegrated pure policy. Map to existing IDs and apply once at the authoritative damage stage. |
| `keywords.rs` | Bestiary roles, status and actor movement capabilities | Reusable bitset/query code; content bindings missing. Do not create contradictory duplicate flying/armor/movement definitions. |
| `epochs.rs` | `W/progression.rs`, study/thresholds; construction/recipe gates | Engine progression is primarily player progression, not settlement epochs. Landmark/resource gating is conceptual until linked to completed structures and actual payment. |
| `gamemode.rs` | `W/rules.rs::{GameMode,Rules}` | Engine permission/mode authority exists. Translate desired modes there; do not layer conflicting prototype checks on top. |
| `spellmod.rs` | `W/adventure/wands.rs::{ShotMods,innate_mods}`; `spell_defs.rs`; `arcana.rs` | Existing spell definition/evaluation architecture. Port desired modifier semantics into it and existing content IDs, rather than installing another spell compiler. |
| `summon.rs` | Spell definitions' summon effects/palette and wand effect dispatch | Capability overlap, but prototype card/pool/cooldown/lifetime semantics are unbound. Verify actual spawn/expiration behavior and connect to current mana/actor ownership; no parallel resource pool. |
| `worldmap.rs` | `crates/app/src/egui_ui/map.rs` and waypoint modules; civilization locations | Map/discovery exists. Society ownership/knowledge overlays need integration. Reuse marker rules without replacing terrain-map persistence. |
| `coop.rs` | `crates/net/src/{session,protocol,commands,transfer,identity}.rs`; `crates/lobby/src`; `crates/netcode/src/{connection,reliable,snapshot,container,edits}.rs` | Real networking implementation exists separately. Prototype is policy scaffolding only. Society commands/outcomes, host authority, reconnect/late-join consistency and household replication require explicit implementation and tests. |

`lib.rs` is the twenty-seventh Rust source file and only the export/integration shell. A wholesale source copy would duplicate several already-functional engine subsystems.

## Concrete next daily-life hook

Verified source points on this date, before further concurrent edits:

- `W/farming/jobs.rs:378`, `goal`: assigned workers return their path front, or their current feet when the path is empty.
- `W/adventure/wildlife/vehicle.rs:220`, `daily_goal`: guard/follow controls precede jobs, then home/regional routine fallback. The unconditional assigned-worker goal prevents ordinary idle workers reaching that fallback.
- `jobs.rs:625`, `service_worker`: handles actor availability and swaps the authoritative job state into `work`.
- `jobs.rs:667`, `work`: reconciles cargo/claims, selects work, follows paths and executes commits.
- `J/production.rs:132`, `candidates`: due meals use the existing `next_meal` ledger; available food produces an Eat candidate.
- `J/production/actions.rs:93`, Eat execution: removes one real meal and updates `next_meal`, `served` and `counts.eaten`.
- `J/crew.rs`: worker-local persisted fields must participate in `Worker::swap`; task/path fields are transient.

### Smallest safe design

Add one defaulted resident routine record to the existing lead/helper state, swapped with the worker identity: phase (`Work`, `Meal`, `Rest`, `Social`), due tick, fatigue/need values, phase-start tick and bounded completion counters. Persist semantic goals/timing; rebuild transient routes after load. Keep home membership in existing housing and meal timing in existing production, not duplicate maps.

1. **Work:** current jobs remain authoritative. Routine changes must not abandon paid cargo. Return cargo through Store and release claims before switching away from a construction delivery. Never clear physical carry to satisfy a phase change.
2. **Meal:** keep the existing Eat candidate and stock debit. Observe successful meal commit to advance the routine; arrival or a timer alone must not count as eating. Hungry carriers first need a bounded, recoverable cargo-return path.
3. **Rest:** use an existing completed assigned home, excluding manual overrides/followers/guards. Add a semantic routine destination task to the same path/task owner. Arrival must use actual interior-position/clearance checks, not the generic work interaction radius. Accrue rest only while the actor is physically at an accessible home. A destroyed or blocked home cancels/replans rest without inventing completion.
4. **Social:** reserve distinct reachable meeting positions with limited capacity. Deterministically pair real resident IDs. Accrue social time only while both actors are present; release both reservations on interruption, disappearance or timeout. Keep pair reservation ownership in one small shared routine ledger, not reciprocal unsynchronized flags.
5. **Resume:** return to existing work selection after the completed dwell. No second movement loop, teleportation, additional inventory or parallel actor registry.

Handle routine transitions in `work` at a safe task boundary, before ordinary work candidate selection. Give routine movement tasks to the existing path queue so `jobs::goal` continues driving the same collision mover. Do not simply return `None` for every idle worker: the fallback is a daily/home goal, not proof of a usable routed trip.

Use phased due ticks so everyone does not eat/rest/socialize on the same frame. Keep gameplay timing in fixed ticks, not `Instant`. Preserve following, guard assignments and saved housing overrides. Avoid indefinite shared-depot waiting as an idle behavior.

### Acceptance for this step

Run multiple simulated days with the same IDs. Each eligible resident must perform real work, consume real meals, physically arrive home, finish rest and resume work. Social pairs must physically meet and dwell. Save/reload during travel, cargo return and rest; compare identity, phase, need values, inventory and destination. Obstruct homes and meeting access, interrupt with player commands, and verify bounded recovery. Report individual unmet needs instead of counting attempted actions as success.

## Scaling to 128 and 256 physical citizens

Do not equate a roster or regional aggregate count with active citizens.

- Update identity and save limits together: private `J/crew.rs:4` has capacity 64; `W/travel/sidecar.rs:12` reserves authored IDs below 64, and its home decoder at line 129 caps home rows at 64. Colony count validation and housing override limits also need reviewed migration. Production's 1,024-worker validation alone does not enable those actors.
- Revisit service budgets. The current 64-worker schedule promises at most three serviced workers per tick. Raising capacity to 256 without changing frequency/budget changes that cost contract.
- Supply enough actual housing. Six bootstrap houses provide 24 spaces; verified infill reaches 32, not 64/128/256. Preserve explicit unsheltered/override counts.
- Add distributed storage, meal stations, work access and distinct waiting positions. The failed baseline concentrates dozens of actors near one depot point.
- Profile and replace expensive whole-actor scans where demonstrated: identity lookup, collision/occupancy, candidates and path queries. Use spatial indexing/cached lookup without changing identity authority.
- Verify every actor's save/load and materialization behavior beyond the camera's immediate neighborhood. Do not silently count remote aggregate population as physical actors.
- Prove local daily life first. Multiplayer acceptance additionally needs two independent clients, authoritative commands, join/rejoin, changed-world synchronization, duplicate/out-of-order handling and inventory/household agreement. Existing network crates are not sufficient evidence of society replication.

### Required benchmark evidence

At 64, 128 and 256, record mean/p95/p99/max per stage: candidate decisions; navigation queries/expansions/budget failures; movement/collision; production/construction commits; housing/routines; actor lookup/spawn; save/load. Report render and simulation timing separately, plus allocations and memory.

Record live, loaded and moving actors; unique IDs making progress per time window; path arrival and distance; job completions; per-resident meals/rest/social visits; starvation and unreachable durations; claim ages; cargo conservation. Store fixed seed/configuration and executable/source identity. Compare equal productive workloads, not completed-workload time against idle time. A faster loop with stranded citizens is a regression.

## Verification boundaries and baseline evidence

The old full64 run at `..\city-colony-full64-20261107` ended after 600,000 ticks with `passed:false`, 4/6 projects and 680/763 paid cells. Its final claims array was empty; all 16 remaining plank orders were unclaimed. Final workers: 57 waiting for materials/access, six hauling, one crafting. Worker 5 held four planks and had eaten only three times, versus approximately 24 for most others.

Accounting: 720 planks produced = 676 placed planks + 40 consumed by five chest recipes + four carried planks. This supports no deposited planks remaining, not a proven reason carpentry stopped. Final depot slots/loose-stack indices were absent from that run, so an exact stall diagnosis cannot be asserted from those artifacts alone.

Its mean combined tick was approximately 1.93 ms, p99 8.81 ms; much of the run was stalled. These numbers do not demonstrate productive 64-citizen throughput, much less performance parity with another game.

Separate code inspection identified a real drop-discovery risk: fixed `dropped.take(256)` ignored later entries while forestry counted all dropped logs toward its target, with tree cleanup allowing 1,024 entries. A private per-worker rotating cursor now scans at most 256 entries and survives retries/save defaults/worker swapping. Six standalone pure tests passed. That establishes bounded discovery behavior, not that the baseline cause is proven or that full native acceptance passed.

The prototype's design documents describe future performance budgets and milestones. They must not be presented as benchmark results. Full application all-target tests were reported blocked by unrelated existing API drift; native executable checks and focused tests must be reported separately from a complete suite pass.

## Downloads folder scope verification

A subsequent read-only inventory confirmed the immediate Java folder contains only `civ_proto/`, `doc/`, `doc_villager/`, `NPC_SIMULATION_REVERSE_ENGINEERING.md`, `VILLAGER_DESIGN.md` and `re_challenge.jar`. The two documentation folders contain 20 and 21 Markdown files respectively, with no additional source trees or native executable projects found outside `civ_proto`.

Static archive inspection strongly identifies `re_challenge.jar` as compiled Songs of Syx reference-game code, not the completed original voxel port. Its manifest names `init.Main`; `init/constant/C.class` contains the strings `Songs of Syx`, `https://songsofsyx.com` and `info@songsofsyx.com`. `init/Main.class` contains Songs of Syx-specific Steam overlay instructions for Mac. Both `NPC_SIMULATION_REVERSE_ENGINEERING.md` and `doc/01_stack.md` explicitly identify the JAR as Songs of Syx by Gamatron AB. `game/VERSION.class` exists, but the exact build version was not established. No standalone license/notice/copyright entry was found; this does not establish permission to copy its code.

The archive contains substantial compiled humanoid AI (`settlement/entity/humanoid/ai/{main,work,consume,home,service,idle,crime,danger,battle,subwalk}`), city rooms/jobs/recipes/statistics, entity management and threaded pathfinding. Its 23 bundled Java source entries are dependency/module sources (22 under `org/lwjgl/system/jemalloc`, one module descriptor), not a separate original application source project. Package presence alone is not behavioral or performance verification.

`VILLAGER_DESIGN.md` explicitly proposes a new 3D simulator extending the reverse-engineering findings. `doc_villager/17_roadmap.md` presents future M0-M10 deliverables and an unresolved Java/C++/Rust choice, not evidence of completed implementation. The 26-module map above audits the separate local Rust prototype and Axiom integration. Verify source authorship/licensing before redistribution. Use the reference game's general architectural techniques for independent implementations against Axiom's existing authorities; do not wholesale copy decompiled game code, assets or content. No JAR execution, application-class loading, or complete reference-game parity verification was performed.

## Latest bounded expansion evidence

`docs/city-funded-housing.md` records the exact executable, two private and one shared passing 64-resident native growth runs, component conservation ledger, strict cargo/final reload checks and workload. The original v1 adapter proposes one four-bed house from three authored candidate sites. The verified v2 extension generates two compatible parcels and paid access, funds one at a time, and preserves exact identities and household routines through sequential reloads. Both use physical materials and the existing construction authority. No prototype inventory, transport or population authority was duplicated.

`docs/city-district-design.md` and `docs/city-roof-egress.md` record separately verified street/parcel design fixtures and real paid construction. District geometry is not yet a general autonomous growth policy. Generalizing the verified small-house infill to styled multistory districts, broadening service/job needs, measuring hundreds of active residents and validating multiplayer save/late-join authority remain next dependencies, not completed claims.
Shared acceptance and promoted executable provenance are recorded in `task/Axiom-City-Neighborhood-Verified-20261107.json` and `docs/city-serial-infill.md` (the earlier single-home baseline remains in `docs/city-funded-housing.md`). The 64-resident need-driven growth fixture and three-builder styled district fixture remain distinct scenarios; the latter is not evidence of autonomous multi-district population growth.
