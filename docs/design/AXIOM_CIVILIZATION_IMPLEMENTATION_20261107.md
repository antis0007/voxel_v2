# Axiom persistent civilization implementation

Architecture and implementation record, updated 11 November 2026. Grounded in the working tree and the supplied civilization brief. The architecture below remains the target; the following status distinguishes implemented behavior from that target.

## Implementation status — 11 November 2026

The planet editor offers **Living** and **Wilderness**. Wilderness disables generated civilization and freight sites, resident spawning, settler jobs, regional civilization and campaigns, onboarding, and settler blueprint controls. Wildlife and independent exploration, magic, personal construction, and industry remain available. This choice is stored in planet recipes, main saves, and multiplayer world snapshots. Old recipes and saves default to Living. Main save version is now 71; versions through 70 retain their existing layout.

The existing local work system now has these changes:

* Accepted tasks, work progress, retry timing, cargo, and physical worker state survive serialization. Visible bodies are captured when unloading and checkpointing. Paths remain disposable derived state.
* Workers outside the presentation radius use the existing collision mover and bounded asynchronous terrain source. Unavailable terrain pauses physical work while retaining custody. Unprocessed physical steps remain due instead of being discarded. Ordinary certified activities advance by elapsed simulation time; water, stairs, status effects and uncertified movement retain the physical executor.
* Crafting debits actual ingredients once into a saved batch. The batch retains progress across interruptions, holds the workstation, waits if output storage is full, and recovers its remaining goods at the station when the worker is released or lost.
* Crew servicing selects a contiguous phase slice and swaps worker state without shifting the roster. Accepted production tasks validate directly. A bounded 8,192-cell footing cache uses world identity and local chunk content versions, so unrelated edits do not invalidate every route.
* Construction clearance and social contact consult retained worker positions as well as visible actors. Work circles are available in ordinary civilization play.
* The development command `/civilization-status` reports workers, accepted work, batches, reservations, terrain requests, and navigation cache counts.

Newly discovered regional settlements now have autonomous communities that reuse this physical executor:

* Each original resident has one saved home, living/dead record, and worker owner. Recruiting and releasing transfer the existing worker; unfinished deliveries prevent transfer. Recruitment before town admission is reconciled. Recorded deaths do not use the old daily respawn rule.
* A town initializes a physical chest and workbench once, without replacing occupied or protected cells. Its explicit initial stock is two bread and six wheat per original resident, plus sixteen planks. Existing wheat plants are adopted without awarding harvested goods. After initialization, workers must harvest, replant, cook, eat, and accept actual imports.
* Community trade exchanges real chest contents with the player's inventory. A merchant must be within three metres of the stockpile. No daily refill supplies these offers. Campaign provisioning and defense debit actual bread and planks; returned surplus waits for storage.
* Ordered, saved wake queues service at most 64 individual events and one council per fixed tick. Separate planning and individual lanes prevent councils from consuming the movement/meal allowance. Each resident retains one deadline. Idle workers sleep until needs are due; changed stock or work wakes them at council review. Terrain sources and executor storage remain shared.
* Farming now writes a compressed COM2 community section. Loading validates unique identities, custody, deaths, and complete wake records before rebuilding derived indexes. Existing COM1 sections and older sidecars remain readable.
* The civilization report includes each community's stage, block positions, actual bread/planks, worker IDs, enlisted/dead IDs, next wake, and preparation obstruction, plus scheduled/overdue event counts.

The subsequent growth increment adds these source behaviors:

* Retained settlement capacity is 512, with a bounded 64-person executor per town. Historical sites still begin with at most six original residents. These are admission limits, not a demonstrated population or frame rate.
* A resident physically present at the chest coordinates stock targets and projects. Committed work records individual mining, farming, building and crafting experience. Completed planks and tools advance craft knowledge. Recipe dependencies use the authored catalogue, an eight-level recursion guard, a 128-call budget and bounded bills.
* Resource observations inspect admitted terrain near actual resident positions. Provisioned scouts walk outward, observe and return through the normal collision mover. Discovered plants and trees become farming/forestry work; exposed stone and ores become excavation orders. Forestry counts nearby timber rather than another town's distant logs.
* Workers craft and install a field smelter beside the chest. Saved batches reuse the existing copper, tin, iron and gold-concentrate rules, including voxel volume and coal fractions. Whole batches debit physical inputs once and retain output while storage is full. This connects ore extraction to metal-tool recipes without pretending that a workbench is an industrial machine.
* Paid house projects have pitched roofs, body-width doors and exterior steps for roof access. Blocked sites advance through a bounded candidate grid. A camp founder receives the first finished home; later completed vacancies can accept migrants.
* Founders carry and spend a chest, workbench, food, wheat seed and timber at an unoccupied generated site. They receive no historical starter endowment. Empty fields require ordinary seed collection and sowing. Migration transfers an existing person's identity, body, experience and ownership only after arrival at a completed vacant home.
* Merchants carry payment and provisions between actual chests. Arrival rechecks war status, quantities and food reserves. Unsold goods return physically. Targets include attainable founding provisions, and reserves leave a sellable surplus.
* Repeated actual foot traffic commissions bounded paving work. Certified travel records the traversed segment. Builders consume carried cobblestone; displaced soil remains as haulable stock. Crops, designated beds and protected bases are excluded, including farms designated after a road order was queued.
* Sustained shortages can cause a particular resident to attempt theft. Nearby residents oppose the thief in nonlethal, line-of-sight scuffles using actual body health. Each expedition records at most one diplomatic incident; repeated incidents can cause war. Existing supplied campaigns use the updated resident identities. Diplomacy examines one town and at most sixteen nearby holdings per service instead of a daily all-pairs scan.
* Terrain readiness installs up to two completed pages per request, preventing large farm/corridor requests from evicting their own earlier results. Development and navigation use resident source pages and request missing terrain asynchronously. Fine-scale travel prepares intervening source pages as well as endpoints.

The common activity increment now connects the individual executor, scheduler, presentation and checkpoint:

* Each resident owns a saved timed commitment for work, certified straight walking, rest or waiting. Mining, sowing, harvesting, forestry, crafting and smelting use this shared timing. Existing transfer, construction and consumption handlers commit their real effects at activity boundaries; they are not replaced by statistical stock production.
* Commitments record terrain dependencies and the world configuration. Voxel edits interrupt subscribed workers before mutation, including edits issued while an executor temporarily owns a town outside the registry. Work, walking and partial rest retain elapsed progress. Meals and routine deadlines can interrupt ordinary commitments. Paid batches keep their input/output custody through interruption, full storage, release and death.
* Ordinary walking checks the swept body and ground support, preserves native species/job speed and samples the same position for a visible or retained person. It handles the native foot clearance above a voxel surface. Corridors are at most 12 metres, inspect at most 4,096 unique cells, and share a 512-entry certificate cache. All corridor probes together have a 4,096-cell budget per tick; failure to certify retains physical movement.
* A shared resumable navigation service admits at most 32 active searches and expands at most 512 nodes per tick, in slices of at most 128. Waiting residents retain small requests, not independent large search buffers. Completed results are bounded at 128 and cached routes at 256. Missing terrain retains the frontier; completion wakes its owner. Queue admission and abandoned-request cleanup are also bounded.
* Actor collection filters retained workers by distance before owner lookups, removing the previous repeated whole-workforce scans. It still traverses the retained roster once per presentation refresh; planetary spatial paging is not implemented.
* World version 71 embeds the farming and travel payloads in the same checksummed manifest as physical inventories and committed terrain references. It no longer combines a new world inventory with an independently replaced farming/travel sidecar. Travel version 6 saves the background survey cursor. COM2 expansion is bounded at 256 MiB with a 32 MiB compressed limit; the overall world manifest remains bounded at 64 MiB. Snapshot serialization/compression is not yet paged or measured for large populations.
* A deterministic background geographical survey admits settlements independently of the viewer. Food targets scale as six meals per local resident plus the twelve-meal founding allowance once a town has four residents. Targets no longer stop at 128 bread. Housing candidate bounds and work territory grow together.
* Physical community campaigns now travel through the individual's mission, route and carried provisions. Arrival, defender contact and spent materials determine siege progress; camera presence does not authorize a crossing or strike. Legacy campaign crossings also require admitted terrain and collision checks. Repeated theft creates grievances; war requires a later diplomatic decision and available people/provisions instead of a random daily rich-neighbour trigger.

The civilization report includes stock targets, coordinator, craft/metal receipts, skills, projects, vacancies, scouts, travellers and cargo, road cells, active commitments, interruptions, navigation expansion counts and shared route/corridor hits. Mission, batch and commitment state are saved; older records receive defaults.

**Limits:** this is bounded regional simulation. Legacy saved ledgers retain their old economy; physical communities bypass formula stock/growth and banked remote construction. New settlements reuse existing generated sites, and migration redistributes existing people. Birth, aging, succession, dynamic state formation, full autonomous industrial/agricultural operation, large armies and regional tactics remain unfinished. Town coordination is a curated policy, not evidence of open-ended institutional emergence. The background survey is incremental admission, not a fully populated, paged planetary population.

The due-event budget can delay work under load; it is not a demonstrated performance capacity. Existing multiplayer restrictions on worker simulation remain. Paged person storage, broader activity types and complete political institutions remain future work. Other subsystem sidecars have not all moved into the atomic checkpoint. Terrain admission, exceptional physical movement and limited offscreen encounters still prevent an exact camera-invariance claim.

The common activity source passed an offline, locked local compile-only check with default features (target/civilization-activities-check.log). No tests, benchmarks, native launches or hosted CI were added or run for this implementation. This source does not inherit an older executable's performance or visual acceptance. Save/reload execution, long-run economics, large-save capture latency, supported population and frame rate remain unverified. The audit below describes the pre-implementation snapshot where it refers to skipped task fields or save version 68. Later stage gates remain open; raising constants does not satisfy them.

## 1 Architectural conclusion

Build **one persistent simulation of people, possessions, places, and commitments**, with several ways to execute the same commitments. Keep every established person. Eliminate repeated calculation when nothing consequential can change. Make settlement statistics read-only views of those facts.

The decisive improvement over the supplied brief is to choose an implementation boundary: **share execution without merging ownership or personal history**. A second statistical civilization simulator is unnecessary for the first useful version and dangerous as the default authority. Distance changes which geometry and animation are resident; it does not create a different economy.

Use a small set of typed activities: travel, transfer, production, construction, consumption, care, rest, learning, communication, and enforcement. Each carries its progress, custody, dependencies, next event, and interruption rules. Initially execute them individually. Then add exact shortcuts, one activity type at a time, when their preconditions can be checked cheaply.

The main new optimization is a **validity interval** attached to an activity or shared routine. It identifies why that activity can advance cheaply, which changes would invalidate it, and the earliest boundary at which it must be examined again. This is ordinary dependency tracking and event simulation applied rigorously, not a claim of a new universal algorithm.

The hardest limit is consequential interaction. A million idle people can be compact; a million people simultaneously contesting different possessions, escaping collapsing buildings, and interacting through portals cannot be made cheap merely by grouping them. When the useful independence disappears, execution returns to individual events. Capacity must be advertised in terms of due work, contacts, edits, and active regions as well as population.

Keep Rust, raw WGPU, mixed voxel sizes, local gravity frames, the existing World owner, native play, and the no-ECS decision. Do not make learned policies, CUDA, a universal planning language, or stochastic population aggregation prerequisites.

## 2 Project evidence and reconciliation

### Current owners and concrete gaps

These are source observations, not gameplay verification. The checkout contains extensive concurrent modifications; the file names and observations below describe the inspected snapshot.

| Responsibility | Existing source | Consequence for this implementation |
| --- | --- | --- |
| World and entity ownership | [World](../../crates/app/src/world.rs), [ADR 0010](../adr/0010-no-ecs.md) | Add explicit tables under World. Keep stable legacy handles and adapters; no ECS migration. |
| Generated settlements and initial sites | [worldgen civilization](../../crates/worldgen/src/civilization.rs), household generation | Reuse geography, plans, legacy identities, and initial conditions. Generation must stop deciding subsequent social outcomes. |
| Regional settlement behavior | [travel civilization](../../crates/app/src/world/travel/civilization.rs) | Current daily food/material formulas, food-triggered residents, population/charter-triggered buildings, and banked remote labor do not satisfy the new causal contract. Replace these writers for migrated settlements. |
| Local workers and cargo | [jobs](../../crates/app/src/world/farming/jobs.rs), [crew](../../crates/app/src/world/farming/jobs/crew.rs) | Reuse carrying, task execution, and actor integration. Crew capacity is currently 64. Task, work progress, and retry state are serialized as skipped fields; persistent commitments need explicit saved equivalents. |
| Production and reservations | [production](../../crates/app/src/world/farming/jobs/production.rs), [reservations](../../crates/app/src/world/farming/jobs/reservations.rs), [supply](../../crates/app/src/world/farming/jobs/supply.rs) | Useful physical substrate exists. Generalize inventory quantities, machine slots, and deliveries through adapters rather than creating a second stock database. |
| Shared supply calculations | [supply frame](../../crates/app/src/world/farming/jobs/supply/frame.rs) | Already derives project needs and cargo in transit. Its construction visits project cells and workers; making it shared is useful, but a small result does not bound its input scan. |
| Rest, meals, social behavior | [routine](../../crates/app/src/world/farming/jobs/routine.rs), [group decisions](../../crates/app/src/world/farming/jobs/groups/decision.rs) | Preserve these behaviors. Current work circles already have invitations, acceptance, refusal, and exit, but are bounded local construction associations, not a complete institutional system. |
| Physical construction | [construction](../../crates/app/src/world/farming/jobs/construction.rs), [effects](../../crates/app/src/world/effects.rs) | Placement checks claims, carried material, occupancy, and support, then consumes cargo after a successful effect. Extend this path. |
| Batch terrain edits | [World edit methods](../../crates/app/src/world.rs) | set_many_committed explicitly permits partial success. Do not treat it as an atomic transaction for an entire building or debit all materials in advance. |
| Navigation | [worker navigation](../../crates/app/src/world/farming/jobs/navigation.rs), [adventure navigation](../../crates/app/src/world/adventure/navigation.rs) | Reuse collision/footing queries. Add reusable surface connectivity and sliced route service; do not mistake existing local search for a scalable planetary navigation system. |
| Scheduling | [farming tick](../../crates/app/src/world/farming.rs), adventure service, vehicle service | Local jobs and regional civilization currently enter through different services. Route migrated society through one semantic scheduler; adapters must not advance it a second time. |
| Persistence | [main storage](../../crates/app/src/world/storage.rs), [background save](../../crates/app/src/world/storage/background.rs), [travel sidecar](../../crates/app/src/world/travel/sidecar.rs), farming sidecar | Observed main save version 68, travel version 5; main file limit 64 MiB, travel row limits and 128-settlement limit. A million-person store needs paged persistence, not a larger serialized sidecar. Recheck versions when implementing. |
| Multiplayer | [replication](../../crates/app/src/multiplayer/replication.rs), inventory, persist, profiles | Retain host authority and current transport. Add accepted society outcomes and coherent checkpoints; this audit does not establish existing replication coverage for every new fact. |
| Presentation | resident models, vehicle actors, existing worker poses | Draw the committed activity and cargo. Animation detail cannot decide production rate or resource transfer. |

Two particularly important facts in travel/civilization.rs:

* advance adds food from site harvest, population, workshops, and a weather roll; materials increase from a daily formula. These quantities are regional scores, not demonstrated transfers from the physical supply chain.
* start_project selects a workshop from population/charter conditions; remote_labor accumulates when distant and is spent when nearby chunks permit construction. service discovers ledger records around the viewer.

Those are useful prototypes, but extending them unchanged would preserve the hidden director and observation-dependent realization that the new brief rejects.

### Documents used

The controlling design evidence is [persistent civilization direction](../superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md), the supplied master brief, and the engine contracts in [Unified Engine](../specs/AXIOM_Unified_Engine_Implementation_Prompt.txt). Relevant portions of [Planetary Simulation and Co-op](../specs/AXIOM_Planetary_Simulation_And_Coop_Architecture.txt), [Living World](../specs/AXIOM_Living_World_Expansion_Prompt.txt), [Meridian Anchor](../NEW/MERIDIAN_ANCHOR_V2_DESIGN_AND_IMPLEMENTATION.md), [player freedom](../parked/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md), [solved geography](../superpowers/specs/2026-11-05-civilization-on-solved-geography-design.md), and the [villager slice plan](../plans/2026-11-06-villager-civilization-slice.md) inform the reconciliation. Existing [settlement research](../research/mod-family-settlements.md) and [navigation research](../research/pathfinding-nav.md) remain reference leads.

The November 6 slice plan says runtime settlement records and builders were absent. The current source contains both; that audit is stale. Its recommendation to start with a registry skeleton is also superseded here by an end-to-end livelihood slice.

V8_COMBINED_MASTER.md, its authoritative chapter set, and the separately named November 6 assessment were not located in the repository, available attachment texts, or targeted Downloads filename search. The complete implementation package mentions V8; that mention is not a substitute for inspecting V8. The separately named Emerson paste was not found, but the original published chapter was retrieved and read.

Resolve the old/new tension as follows:

* Authored ancient events, ruins, site grammars, and an initial settlement are valid starting conditions. Continuing growth must be commissioned by residents or a real organization.
* A parametric building plan is an efficient representation of a chosen project. It does not require a town director, and it does not imply free completion.
* Cohorts are indexes and statistics over retained identities initially. They are not permission to replace named or unnamed established residents with anonymous totals.
* Preserve the Anchor's engineered civic scale, layered history, and pixel-art direction. Simulation is not a reason to replace authored architecture with random voxel piles.
* Quiet building, exploration, automation, public service, governance, and conflict remain viable paths. A wealthy settlement does not summon an unlimited raid supply.

## 3 Architecture comparison

All candidates below must implement the same small reference economy, identities, rights, and interventions.

| Candidate | Strength | Cost or failure mode | Decision |
| --- | --- | --- | --- |
| Individually scheduled state machines | Simple ownership, good explanations, predictable debugging; event-driven already avoids frame-by-frame reasoning | Repeats common feasibility checks, route work, and routine bookkeeping | Implement first as the reference and permanent fallback |
| Persistent activities with shared execution | Retains individual consequences while sharing code, dependency checks, routes, and valid intervals | Invalidations, fanout, and payload diversity can cost more than they save | Selected production architecture, introduced through measured specializations |
| District populations with individual exceptions | Can cheaply advance sufficiently closed aggregate models | Ownership, family networks, unequal access, spatial cuts, and observation make exceptions widespread; reconstruction cannot recover erased facts | Use for derived statistics and optional later models with declared statistical guarantees |
| Shared learned policies or learned district closures | May improve unfamiliar choices or approximate hard dynamics | Training, observation generation, drift, diagnosis, and semantic fidelity are additional costs | Optional offline research after the deterministic baseline works |

The selected architecture combines the first two. It does not require a distinct person model for each distance tier.

Each optimization must have a ledger of its own:

| Optimization | Work removed | State retained | New cost | Stop using it when |
| --- | --- | --- | --- | --- |
| Scheduled thresholds | Polling every person's needs | Anchor value/time, rate, next threshold | Scheduler entry and dependency update | Rates or dependencies change too often |
| Shared routines | Repeated interpretation and common validation | Individual phase, entitlement, progress, exceptions | Member lists and split/rejoin work | Most members are exceptional |
| Organization delegation | Repeated negotiation and search by consenting members | Mandate, individual obligations, dissent, exit | Communication, monitoring, succession | Coordination costs exceed decisions saved |
| Route and field reuse | Rebuilding equivalent route portions | Origin, destination, profile, cargo, route revision | Cache lookup, invalidation, memory | Destinations or revisions churn |
| Immutable data pages | Full snapshots and incidental copying | Authoritative page versions | Changed-page allocation and retirement | Too much state changes every interval |
| Archived facts | Keeping all history in RAM | Referenced legal, kinship, and causal records | Bounded disk access and index maintenance | The interaction requires the cold facts; prefetch them |

Do not assume the elaborate implementation wins. Keep individual execution if a shortcut fails fidelity checks or does not reduce total CPU/memory cost under the actual scene.

## 4 Mathematical and semantic contract

### Authoritative state and clocks

Let X contain people, households, organizations, custody balances, claims, commitments, place geometry revisions, communication events, and random-process state. Derived census totals, render objects, and cached paths are not separate sources of truth.

Keep three clocks:

1. Semantic time: integer ticks anchored to the existing fixed simulation clock.
2. Presentation time: interpolation and animation between published outcomes.
3. Readiness: revisions available for collision, navigation, rendering, persistence, and replication.

Wall-clock worker completion never selects a winner or random draw. Use a stable order for conflicting effects: semantic tick, declared effect phase, resource arbitration result, stable operation identity. Independent effects can publish in parallel only when their read/write and hazard sets commute. Use resource-local fair arbitration, not permanent lowest-person-ID priority.

For the initial model retain a 20-minute game day as an explicit design assumption, not a claimed current setting. A 365-day year then takes about 121.7 real hours, and twenty years about 101.4 continuous real days. Show growth during ordinary play through migration, learning, projects, and institutions. Do not quietly accelerate biological aging. History generation and an explicitly persistent server may use different, declared time policies.

### Exact advancement

For a piecewise constant rate, store an anchor and arithmetic remainder:

~~~text
quantity(t) = anchor_quantity + rate * (t - anchor_tick)
valid_until = min(completion, exhaustion, capacity, schedule_change,
                  reservation_expiry, hazard_boundary, external_causal_boundary)
~~~

Use integer or rational fixed-point units with checked wide intermediate arithmetic. Retain remainders when changing anchors. Repeated rounding of small increments is not equivalent to one large increment.

A validity interval includes the relevant dependency IDs and versions, permissible rate law, access certificate, hazard coverage, and an exclusive endpoint. At that endpoint, process the scheduled boundary before extending the interval. Every possible writer to a certified dependency must either invalidate it or be ordered after the interval. Missing a dependency is a correctness bug.

There are three different execution savings:

* **Shared code:** apply one routine over many records. Usually still O(number of records).
* **Lazy individual evaluation:** calculate an individual's value from an anchor when needed.
* **Shared state transformation:** update one group phase or common offset, retaining individual residuals. Truly sublinear only where the promised effects allow it.

For example, resting people with independent beds, constant recovery rates, no visitors or hazards, and retained per-person start values can recover lazily until the earliest threshold. A shared kitchen still must preserve each eater's entitlement and actual consumed stock. Merely applying one average hunger number is not an equivalent shortcut.

Implement the resting shortcut with one immutable routine definition, a shared time anchor, per-person residuals, and a maintained minimum of individual next thresholds. Advancing the common clock need not touch every member. Joining, leaving, changing a rate, or reaching a threshold costs an indexed update, typically O(log n) for a heap of n members. Inspecting all n people still costs O(n). Split an injured or interrupted person before advancing through the changed condition; do not force their residual into the common law.

### Representation guarantees

| Subsystem | Exact invariants | Same consequential history under allowed execution changes | Statistical promise |
| --- | --- | --- | --- |
| Identity, birth, death, kinship, migration | No duplicates, deletion, or double residence | Yes, with the same canonical events and random realization | None substituted |
| Custody, reservations, recipes, debt | Exact admitted transfers and declared transformations | Yes for fixed integer semantic rules | None substituted |
| Independent rest, work, learning | Exact progress and resource accounting | Yes within verified validity intervals | Not needed |
| Route travel and constrained crossings | Same traveler, cargo, traversal permissions | Yes only with the same authoritative movement/contact model | Approximate crowd closures are a separate later option |
| Physical combat, collisions, fluid hazards | Accepted outcomes and ownership remain valid | Host replay within the defined model; no cross-vendor floating-point lockstep claim | No claim that a simplified fight matches detailed combat |
| Demographic hazards | Exact selected identities, eligibility, event bookkeeping | Yes for the versioned individual hazard model | Model calibration against desired gameplay distributions |
| Render LOD and passive camera motion | No authority writes | Must preserve semantic outcomes | Animation resemblance only |
| Experimental district closure | Exact transactions still required | Generally no | Only measured distributions and intervention responses |

Let F be the individual transition, Pi a retained projection, G a proposed cheaper transition, and R reconstruction. The requirement Pi F(X) = G(Pi X) holds only when Pi retains sufficient information. Reconstruction must use retained identity records and committed geometry; it cannot invent missing marriages or work histories.

A restricted exact example is independent linear recovery before any threshold. Retaining each start value and the common clock/rate permits lazy reconstruction, and sums advance by the corresponding summed rate. A counterexample is two households with the same total food and population but different food ownership and no exchange: one household may starve although the district mean is adequate.

For stochastic aggregation, equal outgoing rates into every aggregate class for every fine state in a class can establish a Markov aggregate. That does not establish equality of two seeded trajectories. Ownership, network position, and residence usually break the symmetry. This is why population cohorts are not the initial authority. [Banisch](https://arxiv.org/abs/1410.6277)

For a continuous-time model the required strong-lumpability condition is

~~~text
sum(q(x,y), y in C_b) = sum(q(x',y), y in C_b)
for every x,x' in C_a and every pair of coarse classes C_a,C_b.
~~~

This is a condition on the transition law, not on the averages at one instant. For the selected lossless execution shortcuts, reconstruction reads retained individual residuals and facts; it is not an inverse of a lossy population summary.

### Random events

Use per-person, per-channel integrated hazards:

~~~text
H_i,k(t) = integral of lambda_i,k over semantic time
E_i,k    = retained exponential threshold
fire at the first model tick where H_i,k >= E_i,k
~~~

Channels include illness onset or recovery, conception when eligible, and mortality causes. Rates depend on actual individual exposure, care, health, and life stage. Use separate competing-risk clocks; the earliest valid event wins, invalidates incompatible channels, and preserves exposure for still-valid channels. Pregnancy, apprenticeships, and travel have explicit delayed events, not fresh daily rolls.

Specify the discrete clock, quantile implementation, and rounding in the model version. Exactness means equality to that implemented model, not an exact continuum demographic law. Integrated propensity methods are a useful computational reference, not a claim that people are chemical molecules. [Anderson](https://arxiv.org/abs/0708.0370)

Random keys contain world identity, stable person/process identity, channel, semantic event sequence, model version, and draw role. Retries reuse the same draw. Repartitioning, camera motion, and worker identity are absent. Counter-based generators support this addressing; they do not make different models equivalent. Prototype a verified Philox implementation against reference vectors before choosing a dependency. [Random123](https://github.com/DEShawResearch/random123)

Do not tau-leap births, deaths, recruitment, consumption, or migration in the first implementation. Exact individual eligibility indexes are simpler. Later aggregate sampling needs a joint law over identities and scarce resources; clipping invalid counts or swapping selected people is not statistically exact. Interval subdivision must condition on retained randomness rather than reroll it. [Postleap checks](https://arxiv.org/abs/0708.0377)

The 2025 tau-splitting paper is an experiment, not a foundational dependency. It recursively splits proposed counts using retained rate-validity information and conditional sampling. Its theorem concerns a specified nonexplosive reaction system and a distributional result, not individual family histories or fixed real-time bounds. Rapid rate changes and scarce reactants can force refinement. Its linked implementation returned 404 during this inspection, so code and licensing could not be verified. [Paper](https://arxiv.org/html/2509.12631v1)

### Population and causal horizons

Define resident population by domicile, and physical occupancy separately:

~~~text
residents_next = residents + births - deaths + domicile_arrivals - domicile_departures
~~~

A migrant has exactly one custody/transfer record and one domicile transition; appearing in a travel roster does not count them as an additional resident. Children consume resources and require a care commitment. Guardianship, inheritance, and abandoned obligations are explicit transitions.

Regions may only commit through a frontier for which earlier inputs are closed. An interaction-capable region stays at the host's canonical frontier. Positive travel and communication delays may permit conservative lookahead elsewhere. A newly opened near-zero-delay portal requires synchronization at a common committed tick; it cannot deliver an intervention into another region's already committed future. Merge the affected scheduling domain when necessary.

Passive camera changes must never change the model. A player's physical arrival is an intervention and may change collision or knowledge. If a cheap movement mode cannot prove equivalence through a potential encounter, activate the required contact simulation because of that encounter, even offscreen. Otherwise explicitly classify it as approximate and exclude it from the same-history claim.

For one interval the cost is approximately:

~~~text
work = c_event * due_events + c_decision * decision_expansions
     + c_dependency * changed_dependencies + c_contact * contact_work
     + c_nav * graph_work + c_edit * admitted_edit_work
     + persistence + replication + bookkeeping
~~~

Population contributes persistent state and eventual events. It need not contribute a per-frame scan. No representation can remove the cost of arbitrary independent changes or unlimited unique histories.

## 5 Data representation and memory

### Storage layout

Use paged plain tables owned by World. A proposed world/society module owns people and commitments; existing inventory, terrain, factory, and actor systems continue owning their quantities. Society addresses those quantities through versioned adapters.

Use an array of compact headers for loading and inspections; separate columns for due-event scheduling and bulk needs/progress evaluation. Keep variable-length relationships, knowledge, and process payloads in typed arenas. Immutable species definitions, recipes, cultural patterns, and action schemas are interned. Avoid a heap allocation, trait object, or network-sized state vector for every resident.

The following are proposed interfaces, not existing types. Sizes are calculated for the shown C layout on the 64-bit target, not measured Rust allocations or serialized sizes.

~~~rust
#[repr(C)]
struct PersonCore {                 // 64 bytes, alignment 8
    id: u64,                        // monotonic within saved world namespace
    born_tick: u64,
    semantic_anchor: u64,
    household: u32,
    home: u32,
    workplace: u32,
    commitment: u32,
    facts: u32,
    needs: u32,
    relations: u32,
    knowledge: u32,
    generation: u16,
    species: u16,
    life: u8,
    flags: u8,
    padding: [u8; 2],
}

#[repr(C)]
struct ProcessCore {                // 96 bytes, alignment 8
    id: u64,
    principal: u64,
    started_tick: u64,
    advanced_tick: u64,
    next_tick: u64,
    work_done: u64,
    revision: u32,
    definition: u32,
    participants: u32,
    inputs: u32,
    outputs: u32,
    dependencies: u32,
    route: u32,
    authority: u32,
    phase: u16,
    flags: u16,
    payload: u32,
    random_sequence: u64,
}

#[repr(C)]
struct DueEvent {                   // 32 bytes, alignment 8
    tick: u64,
    owner: u64,
    sequence: u64,
    revision: u32,
    kind: u16,
    phase: u16,
}

#[repr(C)]
struct Dependency {                 // 16 bytes, alignment 8
    object: u64,
    revision: u32,
    kind: u16,
    flags: u16,
}
~~~

Persistent u32 references identify owner-managed rows, with an explicit absent sentinel. Do not recycle a referenced persistent row. Runtime handles pair index and generation; retire an exhausted generation rather than wrap it. A person's stable ID is never reused after death. Legacy u32 residents map through a saved origin table to canonical IDs.

Facts include lineage, skills, capabilities, health deviations, personality, and references to current aspirations. Species controls body plan and life cycle; political beliefs and cultural membership are learned facts. Ownership, obligations, and consequential kinship are not entries in an evictable social cache.

Use bounded hot working sets: initially eight salient relationships, eight actionable reports, and one foreground activity per person, with separate persistent obligations. These are cache targets, not maximum numbers of spouses, children, debts, or historical relationships. Cold facts remain addressable and can reenter the working set through a relevant event.

### Full accounting

Let N be living people, E active directed relationship entries, K active knowledge entries, A live processes, D dependencies, R reservations, Z hazard states, Q scheduled events, H households, O organizations, and B inventory balance rows.

The storage equation must include:

~~~text
M = 64N + needs/facts + 24E + 24K + rights/obligations
  + 32B + 128H + 256O + 96A + process_payloads
  + 16D + 24R + 32Z + 32Q + route_membership + indexes
  + arena_slack + navigation_cache + geometry_plans + embodied_state
  + archive_cache + checkpoint_pages + ready_queues + immutable_definitions
~~~

An explicit representative population estimate:

| Component | Assumed average | Bytes per living person |
| --- | --- | ---: |
| Person header | One | 64 |
| Need anchors and rates | One compact row | 32 |
| Skills, traits, lineage references, aspirations | Compact/sparse rows | 64 |
| Active directed relationships | 8 × 24 B | 192 |
| Active knowledge reports | 8 × 24 B | 192 |
| Memberships, rights, obligations | Amortized | 64 |
| Inventory balance rows | 4 × 32 B | 128 |
| Household data | Four people per 128 B household | 32 |
| Organization headers | One 256 B organization per 32 people | 8 |
| Live process and typed payload | 96 + 32 B | 128 |
| Dependency records | 3 × 16 B | 48 |
| Reservations | 2 × 24 B | 48 |
| Hazard clocks | 2 × 32 B | 64 |
| Scheduled event entries | 2 × 32 B | 64 |
| Route membership and traversal state | Amortized shared corridor references | 32 |
| ID, eligibility, spatial, and reverse indexes | Amortized | 48 |
| **Subtotal** | Before page slack and fixed caches | **1,208** |
| **With provisional 25% arena/index slack** | Estimate, not an allocator measurement | **1,510** |

These averages must be measured. Extra jobs, households with many obligations, detailed personal inventory, species requiring more hazards, and large projects add their actual rows; they do not fit invisibly in the header.

| Living population | Estimated population-linked working state |
| ---: | ---: |
| 1,000 | 1.51 MB |
| 10,000 | 15.1 MB |
| 100,000 | 151 MB |
| 1,000,000 | 1.51 GB, about 1.41 GiB |

A desktop planning allowance of 192 MiB navigation, 256 MiB site/geometry plans, 16 MiB embodied state, 64 MiB archive cache, 64 MiB checkpoint pages, 32 MiB ready queues, and 8 MiB definitions adds 632 MiB. Thus the representative million-person civilization working set is about 2.02 GiB before unusual exceptions, separately from terrain, renderer, physics, audio, and operating system memory.

If everyone needs 64 hot relationships and 64 reports, the additional 112 × 24 B per person becomes 3.36 GB at one million after the same slack assumption. The population-linked estimate rises to about 4.87 GB. Do not claim million-person capacity for that workload using the sparse-case estimate.

### Identity, history, and disk

Store every living person compactly initially. Preassigned logical identity slots could reduce storage for untouched background populations, but their birth, death, parents, household, traits, and obligations still need a time-consistent representation. They are unsuitable as a shortcut for visit-triggered recruits. Revisit them only after a precise lifecycle model demonstrates savings.

Retain current legal and family facts, unresolved promises, deaths needed for succession, and important causes in an indexed cold store. Compact repetitive completed routine receipts into conserved account ranges and household/service summaries. Never compact away an unresolved claim or the only evidence for a current consequence.

For scale intuition, one 32 B household summary per game day at four people per household is 8 MB/day for a million people. Another 0.01 significant 64 B events per person/day is 0.64 MB/day. Together that is roughly 3.15 GB per 365-day game year before indexes, lineage, edits, and exceptional events. This is a storage-growth illustration, not a forecast.

Bound RAM and processing of archives, not the information content of an arbitrarily long civilization. Offer an explicit history-retention policy for routine detail and retain important facts. Unlimited exact history with bounded total disk is impossible. Immutable pages and bounded compaction keep save work incremental; a full checkpoint must not duplicate all history in RAM.

## 6 People and society algorithms

### Bounded decisions

Give people competent primitives and heterogeneous interests. The playable world starts with basic gathering, carrying, shelter, communication, and care competence. Experimental worlds can remove selected capabilities; do not confuse the two initialization policies.

On a meaningful event or predicted threshold:

1. Update only the affected belief, need, or opportunity estimate.
2. Continue the current commitment if it remains feasible and satisfactory.
3. Inspect at most 32 indexed local opportunity rows, preserving a continuation cursor.
4. Consider at most eight candidates. Evaluate at most 32 plan nodes at depth at most two for ordinary choices.
5. Score survival, care, safety, attachment, income/access, autonomy, craft, status, tradition, curiosity, and leisure using that individual's weights and knowledge.
6. Choose the first satisfactory option in stable preference order, or use a declared fallback such as safe rest, asking for help, or retaining the current task.
7. Acquire reservations through the authority owner. A failed reservation produces a real failed attempt and a bounded retry.
8. Record the considered candidates, salient estimates, selected reason, and outcome. Explanations come from these records.

Initial stability defaults: keep a feasible commitment unless an alternative is 10% better after switching cost; do not voluntarily reconsider within eight semantic seconds; danger, a missed care obligation, and invalid prerequisites override that hold. Back off failed searches at 2, 4, 8, then 32 seconds, while subscribing to relevant changes that can wake them sooner. These are tuning defaults.

An opportunity index is a query aid over vacancies, reports, notices, and observations. Its authoritative availability check can prevent double spending, but it cannot inform a resident of stock they do not know about. Reachability estimates use known routes; uncertainty stays visible.

Prevent herding with expiring quantity-specific offers, travel costs, individual preference, and explicit capacity. A price quotation does not reserve stock. Prevent endless supplier switching with delivery contracts and switching costs. Proposals use discoverable location/purpose IDs so people can join an existing proposal they know about, without a global rule that prohibits independent invention.

Keep richer deliberation for consequential decisions: migration, a disputed promise, a new enterprise, or an unfamiliar construction problem. Its continuation is saved and its computational budget is explicit.

### Commitments as resumable activities

Each action definition declares typed inputs, effects, spatial requirements, interrupt conditions, dependency classes, progress law, and recovery. Start with hand-written implementations and a tagged payload enum. A generic compiler is unnecessary.

~~~text
start(intent, now):
    validate actor knowledge separately from authoritative feasibility
    bound participants, inputs, dependencies, candidate work positions
    reserve output memory and downstream effect capacity
    atomically acquire quantities/workspace/time slots
    create process with operation identity, revisions, progress anchor
    subscribe dependencies and schedule its earliest boundary

advance(process, target):
    reject stale process generation/revision
    advance only to min(target, valid_until)
    evaluate the defined rate law or one bounded discrete phase
    submit a prepared effect; do not mutate from a worker

commit(effect):
    if operation already resolved: return the stored disposition
    reject reused identity with a different payload
    validate dependency versions, expected custody, reservations, authority
    preflight the small mutation and reserve receipt/publication storage
    apply supported cell/account/process changes without yielding mid-effect
    record accepted amounts and revisions; publish invalidations
    schedule the next boundary

invalidate(dependency, at_tick):
    mark its new revision and the unsafe spatial facts immediately
    advance affected processes under their old law only up to at_tick
    prevent any later effect using the old revision from committing
    enumerate affected subscriptions in bounded continuation pages
    distinguish physical invalidity from what residents have learned

suspend(process):
    preserve work-in-progress, custody, consumed inputs, hazards, obligations
    release only reservations whose contract permits release
    schedule the relevant recovery condition or bounded retry

cancel(process):
    apply a typed settlement of unspent reservations and remaining obligations
    retain waste, partial products, delivered materials, injuries, and debts
    do not reverse already completed physical work

complete(process):
    commit outputs and final obligations once
    free dependencies incrementally after readers release their revisions

resume(process):
    load the saved phase, balances, progress, thresholds, and sequence
    rebuild derived paths if needed; revalidate changed dependencies
    continue the existing operation rather than starting a fresh batch
~~~

Important ordering detail: dependency invalidation must be ordered before a later completion, even if the invalidation's subscriber list is still being enumerated. Version checks and the regional commit frontier provide that fence. Merely sending an asynchronous notification does not.

At 60% completion a sawyer may have consumed timber and produced boards plus waste. Their injury suspends the remaining work; custody is in a work-in-progress account. A replacement worker can accept the remainder if skills and access permit. The previous worker keeps earned compensation and obligations.

Animation receives the process phase, work position, tool, cargo, and progress. A hammer contact event may align the presentation of a committed increment, but does not independently authorize it. A blocked actor cannot keep receiving full productive work by playing a stationary animation.

### Economy, property, and transfers

Represent free, reserved, carried, in-transit, work-in-progress, waste, and finished stock separately. Reservations are claims on available quantities; possession is a physical custody relation; title is a recognized ownership relation; permission and enforcement are additional facts.

For a transfer of q units from A to B:

~~~text
operation = (world_namespace, initiator, durable_sequence)
require A.revision == expected_A and B.revision == expected_B
require custody_A >= q and capacity_B >= q
require applicable reservation or declared taking action
require physical transfer conditions and downstream capacity
apply custody_A -= q; custody_B += q
update title only if this action transfers title
resolve reservation amount; increment revisions; record receipt
~~~

Retries return that result before rechecking current stock. If exact receipts expire, persistent sequence fences still reject old operations. Keep payload digests for retained receipts. Do not depend on a cache miss to make an operation fresh.

For terrain construction, use one bounded cell or supported atomic micro-stage as the unit. If a legacy edit batch partially succeeds, commit a separate material debit and receipt for each confirmed cell. Completion means all required cells and access checks succeeded, not that a batch was submitted.

Theft moves actual custody and invalidates the reservation's feasibility; it does not fail merely because another resident reserved the timber. Discovery depends on witnesses, inventory checks, and reports. Player protection settings and network authentication remain hard protocol rules, separate from simulated law.

One affordance contract must cover player builds, NPC work, machinery, golems, and magic. Recipes declare material and energy transformations and explicit sources/sinks; conservation does not mean all recipes preserve item count. Production cannot consume a promised delivery before it arrives.

### Households, care, and everyday life

Households allocate actual food, shelter, care, storage, and time through their own distribution rules. Membership need not imply one shared owner or a single preference. A carer can intrinsically value a dependant and forego paid work; shared care arrangements release time only when someone else actually accepts the obligation.

Children mature through a species lifecycle, education, and practice. Birth creates lineage and a care requirement, not a free adult worker. Illness and injury affect abilities; death creates succession, grief, replacement care, and unfinished claims. Migration is a person's decision and a journey with belongings, knowledge, dependants, and receiving arrangements.

Rest, play, friendship, ritual, recovery, and study consume time and provide specific benefits. They are not residual animations after production. A family dispute can change residence or distribution without changing anybody's species or instantly converting their ideology.

For history generation, advance the same demographic and institutional events where feasible, skipping valid stable intervals. Coarser ancient history is a separately identified initial-condition generator. Its ruins and present-day facts are committed at world creation; it does not pretend to have reconstructed an unrecorded detailed lifetime.

### Organizations and political scale

Use a compact organization record with members, assets, claims, charter/rule references, communication channels, decision rule, delegates, contribution/distribution obligations, monitoring, enforcement capacity, amendment, succession, exit, and dissolution.

| Layer | What it supplies |
| --- | --- |
| Authored primitives | Speaking, posting a notice, offering, contributing, voting, delegating, sanctioning, resigning; architectural and production vocabularies |
| Engine services | Search indexes, valid transfers, calendars, physical rules, saved facts, execution budgets |
| Resident and organization decisions | What to propose, whom to trust, what to share, whom to appoint, where to build, whether to join or fight |
| Emergent outcomes | Durable households, cooperatives, enterprises, councils, contested jurisdictions, trade networks, polities, fragmentation |

The engine cannot decide that a settlement requires seventeen farmers. A kitchen association can publish meal demand, contributors can delegate procurement, and a manager can recruit workers using that authority and information.

Express several provisioning systems using the same ownership and obligation primitives: household sharing, gifts, reciprocal exchange, barter, wages, commons access, cooperative shares, and public provision. Give none an automatic productivity multiplier. Rules change monitoring costs, incentives, exclusion, distribution, and exposure to failure.

Communication has carriers, reach, delay, and evidence. Notices, books, couriers, markets, radio, and magic can reduce uncertainty without granting telepathy. Trust updates from observations and attributed reports, with provenance and bounded memory.

Organizations can save computation because an accepted standing agreement replaces repeated choices. Keep the individual participation contract and exit conditions. Crisis, dissent, debt, or succession breaks the shared routine where necessary.

A regional organization forms when existing groups face recurring cross-group problems and negotiate a mandate: road maintenance, arbitration, water allocation, trade protection, or defense. Political boundaries can overlap and cross engine partitions. Jurisdiction, legitimacy, compliance, and actual force are different variables.

Larger scale does not require every citizen to deliberate about every citizen. Delegates act within recorded limits, report back through communication channels, and can be replaced. This can produce a federation, a monopoly, a coercive state, or fragmentation; none is a mandatory technology tier. Ostrom's polycentric-governance work is a framing reference, not evidence that this particular algorithm will produce viable institutions. [Publication](https://www.aeaweb.org/articles?id=10.1257/aer.100.3.641)

### Ideation and construction

A useful problem produces a bounded proposal record: proposer, observed evidence, beneficiaries, candidate methods, expected costs, risk, and the authority required. A remembered technique is distinct from a funded project.

Use typed process graphs for candidate solutions. Compare ordinary recipe selection first, then bounded composition: beam width 8, at most 64 expanded nodes per proposal, depth 4, and one active experiment per participating workshop. Search can continue across scheduled turns; an experiment spends actual time, materials, workspace, and attention.

Example held-out problem: grain must reach a dry loft across a flood-prone yard, but no stored plan solves this layout. Available parts are a chute, powered belt, storage bin, threshold sensor, and a small levitation spell. Search proposes a composition, a physical trial reveals a clearance or energy failure, and an amended composition succeeds. The final arrangement must be absent from the plan library. The learned process travels through a teacher, plan, or observation; installing shared behavior code does not broadcast the discovery.

Use a few explicit beliefs about collaborators—reliability, likely acceptance, understood expertise—instead of recursive simulation of everyone else's mind. Two research hypotheses motivated by the book are: retained collaborator models reduce failed negotiation; and reusable, transmissible procedures improve recovery and novel composition. Compare with those mechanisms removed. The book's arguments about recombination and coordinated intelligence do not establish a performance result or autonomous civilization. [Origins](https://whatisintelligence.antikythera.org/chapter-01/), [Other Minds](https://whatisintelligence.antikythera.org/chapter-05/)

Construction proceeds through need, proposal, permission, contributions, site survey, procurement, work stages, occupancy, and maintenance. A project owns a dependency graph of entrances, foundations, support, scaffolding, utilities, and rooms, with accessible work positions.

Architectural grammars supply competent proportions, circulation, climate responses, material palettes, and extension patterns. Inhabitants choose requirements and feasible alternatives. Old roads, topography, parcel claims, rivers, and inherited structures constrain the result. Alignment is a preference; diagonals and irregular sites are allowed. Incremental enlargement and maintenance leave visible history.

An unrendered building can persist as a compact plan plus its exact completed stage/cell deltas, but completion still requires paid inputs, work, and valid access. Collision and navigation need a valid query representation of that state. If the site cannot be evaluated without more geometry, schedule that geometry work before advancing construction. Never replace it with banked labor awaiting the player's arrival.

### Military organization

Mobilization recruits actual people from actual obligations. Equipment leaves stores, food enters military custody, families lose labor or receive substitute care, and commanders have finite information and authority. Formations share route and order processing while retaining roster identities, wounds, desertion, and personal possessions.

A defeated force reduces its source's personnel, equipment, and willingness to fight. Retreat, surrender, negotiation, and desertion can interrupt combat commitments. Prosperity is an incentive for finite actors, not a raid timer. Initially simulate consequential battles with the same physical authority; add coarse combat only as an explicitly approximate later model.

## 7 Execution and engine integration

### Small module boundary

Add proposed modules under world/society only as their first behavior lands:

| Module | Owns | Uses existing owners through |
| --- | --- | --- |
| people | Identity, needs, facts, household membership | Legacy resident/crew ID adapters |
| activity | Saved commitments and typed phase implementations | Worker, inventory, machine, care, and construction adapters |
| schedule | Due events, dependency subscriptions, commit frontier | Existing fixed-step scheduler |
| institutions | Mandates, offers, contributions, obligations, outcomes | Existing work-circle and town-board interfaces |
| storage | Society pages, operation receipts, migrations | Existing world save/checkpoint owner |

Navigation belongs beside the existing navigation owner, not inside a generic society framework. Inventory and voxel mutations remain with their current owners. Keep derived census and UI inspection functions small. Do not add empty modules merely to match this table.

~~~mermaid
flowchart LR
    Facts[Authoritative facts and due events] --> Decide[Individual or delegated choice]
    Decide --> Reserve[Reservations and effect admission]
    Reserve --> Activity[Persistent activity]
    Activity --> Commit[Validate and commit bounded effects]
    Commit --> Facts
    Commit --> Physical[Terrain, custody and physical actors]
    Commit --> Durable[Journal and replication]
    Physical --> Views[Animation and map views]
    Physical --> Dependencies[Geometry and hazard revisions]
    Dependencies --> Activity
~~~

### Navigation matched to actual demand

Emerson already supplies sector portals, cost/integration/flow fields, caching, movement classes, dynamic changes, and sliced work. It explicitly discusses downstream boundary costs reducing reuse, and lists overlapping-sector 3D support as future work. These are the starting point, not features to claim as new. [Chapter](https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf)

Use four cooperating layers:

1. A regional graph of routes, crossings, entrances, platforms, moving-frame links, and portals.
2. Walkable surface patches in local gravity frames, with overlapping floors stored separately.
3. Individual corridors for uncommon destinations and cached local fields for demonstrably shared traffic.
4. Capacity calendars at constrained crossings plus local locomotion and authoritative collision.

For a safe short direct corridor, use a clearance-checked sweep first. Otherwise obtain a sliced hierarchical route. Build a reusable field only when repeated demand is likely to amortize it. Thousands of distinct homes should share common streets and connectivity without allocating a planet-sized destination field for every front door.

The break-even condition is:

~~~text
k * P > F + I + k * L
k > (F + I) / (P - L), when P > L
~~~

Here k is compatible requests before invalidation, P individual route cost, F field build cost, I expected repair/eviction overhead, and L lookup cost, all in the same time units. Add memory opportunity cost to admission separately. With hypothetical P = 50 microseconds, F = 250, I = 50, and L = 1, reuse starts winning at seven requests. These values illustrate the calculation; they are not Axiom measurements.

For a patch T:

~~~text
V_T(x) = min over terminals p of [d_T(x,p) + V(p)]
~~~

Terminals include all necessary boundary states and any interior goal. This decomposition is exact for the chosen static, additive-cost graph when the boundary state retains all future-relevant information. A state may need location along an aperture, floor, direction, load profile, or traversal mode. A single portal center is not generally sufficient.

Compare three implementations: local multi-source Dijkstra seeded by downstream values; cached distances to frequently used exits; and a small boundary-to-boundary transfer graph with local refinement. Start with the first and selective exit caches. Dense precomputation is expensive: 4,096 local cells × 16 exits × 4 bytes is 256 KiB per patch per profile, or 256 MiB for 1,024 patches, before directions and metadata.

For compatible boundaries, subtracting the same finite minimum from every boundary value leaves the chosen exit unchanged. This permits exact normalized cache keys, provided terminal geometry, costs, traversal profile, and topology revision also match. Do not normalize away unreachable states.

If boundary values are rounded to within epsilon/2, the local value error is at most epsilon/2 and the selected exit's regret at most epsilon, assuming exact local distances. That is a local bound. Repeated approximate choices can accumulate error or loop; a global guarantee needs a bounded sequence of patches and a valid decreasing potential. Begin with exact integer costs and no quantized route policy.

Use integer graph shortest paths initially. A smooth Eikonal solver is an optional comparison for broad open areas, with discretization and convergence measured separately. Use wide checked path sums and an explicit infinity value.

Cheap portals invalidate a naive world-distance heuristic. Use zero as the safe baseline, then evaluate lower-bound distances on a graph that already includes cheap links. A heuristic must respect every directed edge. Scheduled lifts and ferries require arrival-time state or a bounded reservation calendar; static distances estimate travel but cannot book a seat. For non-FIFO edges, retain the necessary time-expanded states and explicit waiting options.

Walkers use support patches and clearance appropriate to body shape, load, slope, and turning. Swimmers use traversable water volumes and currents; flyers use sparse free-space cells and landing links. Moving structures reuse their local geometry, with transforms and docking/attachment links updated independently. A new gravity frame changes traversal semantics, not a world-Y flag.

Recast/Detour provides sliced queries, but its raycast traverses polygons using a 2D intersection operation. The API's 3D positions are not proof that an arbitrary overlapping-floor or planetary query is safe. Retain explicit floor/frame identity and validate the actual integration before reuse. Its sliced query also retains query state; do not mix incompatible calls on the same instance. [Official source](https://github.com/recastnavigation/recastnavigation/blob/main/Detour/Source/DetourNavMeshQuery.cpp)

### Traffic, collisions, and edit readiness

Doors, single-file bridges, stairs, lifts, and loading bays have capacity and admission order. Initially reserve only the next constrained segment and its exit space. Do not hold one bottleneck while acquiring another. A short rollback/refuge segment handles a failed onward reservation; starvation aging and alternating directional service keep opposing traffic moving.

Use a spatial index whose normal bins admit at most 32 ordinary actors, with at most 16 avoidance neighbors per query. Split or control entry before bin occupancy becomes excessive. Truncating an avoidance neighbor list is not a collision guarantee.

Authority still checks swept movement against the complete relevant occupancy set, in resumable batches if necessary; an unfinished movement query cannot commit the move. Exceptional compression from a collapse or forced displacement enters a bounded contact-resolution backlog with local motion held until safe results exist. That is an overload/failure case to measure, not proof that dense physical crowds have constant cost.

Local avoidance can improve motion, but it does not choose fair access or resolve every deadlock. Global multi-agent pathfinding is too expensive as the general service; reserve coordinated searches for small trapped groups. ORCA and continuum fields are comparison methods, not licenses to discard physical contacts or individual destinations. [ORCA](https://gamma-web.iacs.umd.edu/ORCA/), [Continuum Crowds](https://grail.cs.washington.edu/projects/crowd-flows/)

On destructive edits, remove support/collision authority and fence the affected traversal links at the same committed tick. Existing routes can follow only a verified safe prefix. On additions, a minimal visible representation and usable collision must be ready before the new surface becomes traversable. Use immediate dirty/blocker masks and minimal edit rendering while full meshes are rebuilt.

Publish collision, nav, and visual products with source revision and installation generation. Reject an old worker result after a newer edit. A newly opened shortcut may wait for route improvement; a newly closed bridge cannot wait to stop admitting traffic.

A bridge deletion may invalidate routes across a whole region. Fence it immediately, then enumerate and repair dependencies incrementally. D* Lite can reuse search work for suitable repeated queries, but neither it nor a dirty flag guarantees constant-time repair of an arbitrary graph cut. [D* Lite](https://idm-lab.org/bib/abstracts/papers/aaai02b.pdf)

### Work units and queues

Begin with one host semantic frontier and a simple due-event heap. Add a timing wheel only if measured distributions justify it; use a heap for far-future or irregular deadlines. Intrusive handles permit replacement/removal of an event instead of accumulating unlimited stale entries. Group calendars are useful when agreement semantics actually share a schedule.

Workers operate on immutable bounded snapshots. They produce proposals with read versions; the owning scheduler resolves conflicts and publishes results in canonical order. Memory allocation, installation, retirement, and serialization are included in the budget.

Proposed initial units and limits:

| Stage | Maximum cooperative unit | Saved continuation | Initial backlog rule |
| --- | --- | --- | --- |
| Event dispatch | 64 events; stop earlier at a measured slice boundary | Due cursor and canonical ordering cursor | Persistent future events counted in population storage; ready work has an age limit |
| Candidate discovery/decision | 32 inspected rows / 32 plan expansions | Index cursor, partial candidates | One pending decision per person; merge reasons without deleting consequences |
| Dependency propagation | 64 subscription entries | Dependency revision plus subscriber cursor | One dirty record per dependency; commit fence effective immediately |
| Local navigation | 128 node expansions | Search frontier and revision | Fixed request/scratch pool; share equivalent requests |
| Inventory/process effect | At most 8 balance rows and 8 dependencies initially | Prepared operation | Preflight all resources; no partial publication |
| Construction | One cell or validated micro-stage | Project cursor and per-cell receipts | At most 32 cells per installation slice; downstream chunk work reserved |
| Readiness installation | One small page/product handle | Installation cursor | Bounded bytes as well as count; no whole-city swap |
| Archive/checkpoint | 16 KiB page or bounded record batch | Page index and durable watermark | Fixed snapshot window; no full-world clone |
| Reclamation | One page or 64 small entries | Retirement cursor | Reserved service; destructors cannot recursively free a city |
| Replication | 16 KiB payload assembly unit | Per-client sequence and interest cursor | Bounded reliable queue; admitted effect capacity includes it |

These units are algorithmic limits, not measured wall-clock bounds. Target less than 0.10 ms for the largest ordinary foreground unit on the development machine and 0.25 ms on the reduced profile; subdivide any unit that misses. A timer checked after an arbitrarily expensive query is not a bound.

Bound inner work as well: a route slice also stops after 512 examined edges and retains its adjacency cursor; a surface-query slice retains its voxel/shape cursor; an installation copies at most a fixed small byte range before yielding. Do not gather, sort, allocate, or destroy an unbounded list before entering these loops. A large valid transaction must be expressed as separately meaningful micro-effects before admission.

Start with reserved service shares for urgent effects, normal activities, distant due work, and persistence/retirement. Use age-based fair scheduling inside each class and per-region quotas. An urgent battle cannot permanently starve distant households or saving.

Measure queues in work units as well as rows:

~~~text
Q_next = max(0, Q + admitted_work - completed_work)
~~~

For bounded bursts sigma, arrival rate rho, service rate mu > rho, and a service latency L, a rate-latency service model gives backlog at most sigma + rho*L and delay at most L + sigma/mu. These are conditional bounds; actual service interruptions and variable work costs must fit the assumptions.

Example: 600 work units/s arriving, 1,000 units/s serviced, and a burst of 2,000 units needs five seconds to drain the excess at 400 units/s. A 2,500-entry queue can remain bounded while still violating a 200 ms age target. Admission therefore needs both a capacity reservation and an age forecast.

A million simultaneous real events still cost a million effects. Do not move birthdays or deaths to convenient ticks to conceal an event storm. Canonical offsets may spread inherently independent routines, but logical event timing cannot depend on current frame load.

### Proposed operating envelopes

All numbers below are admission/profiling targets, not measurements. The RTX 4090 reference and the i3-1005G1/8 GB reduced reference come from the brief; current CPU, adapter/backend, driver, display mode, and usable memory were not verified in this design pass.

| Workload axis | Reduced profile normal target | Desktop normal target | Stress experiments |
| --- | ---: | ---: | --- |
| Persistent people | 10,000 | 100,000 | 1k, 10k, 100k, 1m independently |
| Interaction-relevant districts | 2 | 8 | 16–64, including disconnected players |
| Embodied ordinary actors | 64 | 512 | 64, 256, 1,024, 4,096 |
| Visible animated people | 128 | 1,024 | Separate full rigs, simplified rigs, and impostors |
| Ordinary decisions due per real second | 200 | 2,000 | 20,000 and synchronized crises |
| Scheduled semantic effects per real second | 2,000 | 20,000 | Burst and high-fanout sweeps |
| Live production processes | 1,000 | 10,000 | 100,000, varying shared dependencies |
| Construction cell effects per real second | 64 | 512 | 4,096 plus demolition and streaming |
| New route requests per real second | 32 | 512 | 4,096, shared and diverse goals |
| Concurrent unique destinations | 256 | 4,096 | One per embodied actor and one per resident |
| Concurrent player regions | 2 | 4 players within the district budget | Players activate distinct cities and portals |

The normal population figures assume many continuing activities. For example, 24 decisions/person/day at a 1,200-second day yields 2,000 decisions/s for 100,000 people and 20,000/s for a million. With an assumed 20 microseconds/decision these consume 0.04 and 0.4 CPU-seconds/s before navigation, effects, or observation. At four decisions/person/second, a million people would require 80 CPU-seconds/s at the same assumed cost. The assumption must be benchmarked.

| Resource | Reduced profile target | Desktop target |
| --- | --- | --- |
| Presentation | 60 Hz aspiration at reduced settings; evaluate a declared 30 Hz fallback separately | 60 Hz reference; 120 Hz stretch |
| Foreground civilization work | 0.5 ms/presented frame | 1.25 ms at 60 Hz; 0.5 ms at 120 Hz |
| Civilization worker CPU | 0.35 CPU-seconds/real second | 2 CPU-seconds/real second |
| Incremental civilization GPU work | Prefer CPU semantic path; at most 0.2 ms of optional presentation work | Initially at most 0.5 ms optional animation/field work |
| Civilization working RAM | 256 MiB target | 1 GiB normal; 2.5 GiB million-person experiment |
| Total game working memory | Initially target below 4 GiB including shared graphics allocations; measure OS headroom | Initially target below 12 GiB, with fixed streaming limits |
| Extra resident GPU allocation | 64 MiB within shared-memory budget | 256 MiB; no per-person GPU state for dormant people |
| Network | 128 KiB/s per client target | 512 KiB/s per client; 2 MiB/s host for four clients |
| Journal throughput and buffering | 512 KiB/s service target, 8 MiB buffer | 4 MiB/s service target, 64 MiB buffer |
| Readiness | Prefetched interactive patch within 250 ms target | Same target; cold visual refinement within 1 s target |

Whole-game CPU critical path and GPU frame time must each fit the presentation interval with margin. For the 60 Hz reference, target CPU critical path below 13.5 ms and GPU below 14 ms; for 120 Hz below 6.75 and 7 ms respectively. These overlap; adding them together is incorrect. The reduced profile is not supported merely because the civilization budget fits while terrain or graphics misses.

Record p50/p95/p99/p99.9 and maximum frame time; counts above 16.67, 33.3, 50, and 100 ms; CPU-seconds by worker class; peak RAM/VRAM; and every queue's oldest age. Separately target routine decision latency below 2 real seconds, ordinary committed semantic work age below 100 ms, and local admitted interaction response within two fixed gameplay steps under the supported envelope. Actual input-to-visible latency also includes presentation and device delays.

### Overload and cold arrival

Before accepting a new expensive project, route batch, or physical operation, reserve its predictable simulation, geometry, persistence, and replication consequences. Defer an unaccepted proposal or stage an operation explicitly. Accepted damage, cargo, births, deaths, and paid work are never dropped.

First remove optional animation detail, redundant search, speculative future geometry, and cache work. Next withhold new expensive commitments whose downstream work cannot meet the age limit. Continue essential scheduled life and already accepted effects with reserved service.

If an uncontrollable crisis exceeds service capacity, the normal operating envelope has failed. Preserve state, save safely, and expose the slow simulation. A declared host policy may slow the shared semantic clock while presentation remains responsive; that changes elapsed gameplay speed and must be shown. It is not a successful fixed-rate benchmark or a cure for unbounded population growth. Never selectively freeze distant people or tie fertility to frame rate.

Forecast predictable lifecycle storage, such as pregnancies and scheduled arrivals, and reserve pages early. Memory pressure cannot veto an already modeled birth or silently substitute a death. If essential population growth exhausts the configured working envelope, keep its facts in paged storage and expose the capacity failure; do not change demographic rules to conceal it.

Physical arrival requires collision, access, and relevant state at the current frontier. Prefetch from travel intent and portal activation. A teleport contract must disclose any preparation before acceptance; once accepted it cannot promise an immediately usable interior and then wait invisibly for generation. Passive camera travel may show a coarse view while visual detail arrives, without advancing society.

### Persistence, multiplayer, and GPU use

Use host-owned commands for shared outcomes, prediction for supported local interactions, and interpolated presentation elsewhere. Do not require clients to reproduce floating-point physics bit for bit. Interest is the union of player interaction regions and cross-region causal dependencies, not just render frusta.

At 256 actors × 48 bytes × 10 updates/s, raw pose traffic is about 120 KiB/s per client before protocol overhead and reliable events. Use distance-dependent pose rates and interest filtering. Reliable accepted transfers and deaths are never discarded to meet a pose-traffic target.

Cross-region transfers use one operation record: origin custody, in-transit custody, then destination custody. A departure fence stops origin execution; arrival materializes the same identity and cargo. Retries and reconnects consult the same durable sequence namespace. Joining snapshots include a checkpoint watermark and subsequent accepted operations.

Save immutable society pages and a coherent root manifest alongside existing world state. The manifest identifies the matching terrain, cargo, person, and operation boundary. Journal small accepted effects, batch durable flushes, and checkpoint within a fixed page window. Applied feedback can precede durability; the displayed durable watermark must be honest.

Recover from a coherent checkpoint plus journal. A save must not retain a placed wall while restoring its timber. Current sidecar APIs are integration points, not proof that their existing independent files already provide this transaction boundary.

Keep sparse branching, social facts, receipts, and custody on CPU. Consider GPU animation, regular field kernels, or perception batches only after profiling includes dispatch, data movement, sorting, readback, and rendering contention. A CPU path remains sufficient for core civilization behavior. Use the existing pinned WGPU version and checked adapter features when implementing; no dependency upgrade is required by this design.

## 8 Worked causal trace

This is a proposed scenario, not a report of existing Fernwake behavior. There are 24 people in six households, a grain source across a river, a carpenter, a difficult crossing, and no commissioned bridge or mill. Costs below are illustrative; the actual blueprint validator supplies quantities and work positions.

| Actor and situation | Knowledge and motivation | Choice and reservation | Physical consequence and next event |
| --- | --- | --- | --- |
| Sena, grain carrier | Records several costly trips and one spoiled delivery | Posts a crossing proposal; reserves time to survey two known sites | Survey produces actual route lengths, risks, and land claims |
| Tessa, carer of child Ivo | Knows her household's meal shortfall; has limited free time | Offers food and two work periods only if another person accepts care | Care agreement occupies that person's time; Ivo remains a dependant |
| Bram, carpenter | Knows one bridge pattern and its material requirements | Offers a staged estimate, contingent on land permission and delivered timber | Surveyed work positions and scaffolding become prerequisites |
| Kell, injured former carrier | Cannot perform heavy hauling; knows a supplier | Offers record-keeping and negotiation | A real message reaches the supplier; no imaginary hauling capacity |
| Nerin, dissenter | Expects little personal benefit and doubts the organizer | Refuses the contribution request and proposes a ferry alternative | Refusal persists; comparison can change other people's decisions |
| Oda, landholder | Controls one approach and values continued access | Grants limited access with a maintenance obligation | Project authority includes that condition, not ownership of the entire bank |
| Several consenting contributors | Have different benefits, trust, and resources | Form an association with a quorum, steward, public-access rule, and exit terms | Actual contributions enter association custody |
| Supplier and haulers | Know price, destination, and capacity | Reserve 24 timber units and 8 stone units in separate deliveries | Cargo leaves stores only on pickup; downstream work waits for arrival |
| Bram and two helpers | Know delivered stock and safe work positions | Accept the first supported stage; reserve tools, positions, and a crossing slot | Paid increments alter terrain; partial work survives interruption |
| Pavo, noncontributor | Learns the crossing is open | Uses the public bridge under its access rule | Benefits without a retroactive invented contribution; may later support maintenance |
| Sena and a grain processor | Observe cheaper deliveries after opening | Compare expanded milling with other uses of their resources | A mill is proposed only if knowledge, inputs, rights, and participants support it |

The proposal can fail. Oda may reject access, the ferry may be cheaper, food may be needed immediately, trust may be too low, or the association may fail to acquire enough timber. Failure leaves an inspectable reason and any already spent survey effort. It does not trigger the engine to fund a bridge anyway.

For a simple accounting checkpoint, suppose six of the reserved 24 timber units have arrived, four are incorporated, and two remain at the site. There are 18 still at the supplier or in explicitly recorded transit. Project progress cannot claim more than the four installed units. Tool wear, waste, and conversions follow the chosen recipe; the following interventions operate on that same state.

| Intervention | Persistent result | Computation needed |
| --- | --- | --- |
| Player steals two reserved timber units | Custody moves to the player; title may remain disputed; project has a two-unit shortage | One taking effect, reservation/dependency revision, local discovery/report work |
| Flood destroys a partial span | Installed material becomes declared debris/loss; remaining stock and paid work survive | Immediate support/link invalidation, affected cargo/contact simulation, sliced route repair and project revision |
| Supplier dies | Stock passes through succession; promises become estate obligations or defaults | Death, kinship/estate lookup, affected agreements; no global supplier scan |
| Organizer dies | Association retains assets and charter; successor/quorum or dissolution rules apply | Notification, bounded membership decision, transfer of delegation; no reset of project |
| Another group blocks trade | Actual checkpoint, patrol, or physical obstruction changes feasible access | Route and risk updates; reports arrive with delay; possible negotiation or alternative procurement |
| Conveyor becomes available | Its owner offers transport; installation, energy, and maintenance have real costs | Reevaluate affected known delivery choices at contract boundaries |
| Costly portal becomes available | Entry, fuel, permissions, and capacity enter the route graph | Synchronize connected causal domains; invalidate lower-bound heuristics as needed; no free cargo |
| Everyone remains distant for weeks | People continue the same projects, meals, care, hazards, and claims | Scheduled effects and certified advancement; no remote-labor allowance or visit-triggered completion |
| Two players approach from different directions | Both see the same people, stock, project phase, and damage | Union interest, shared materialization generation, one authority owner |

Continue into mobilization: three contributors join a defensive force, taking issued equipment and rations. Their building commitments suspend or pass to willing substitutes; care arrangements must change. A battle casualty is the same household member who previously contributed timber. A retreat returns surviving people and actual remaining equipment, not a regenerated garrison.

The important trace is causal: cheaper transport changes a person's perceived options; it does not directly increment a settlement level. If nobody can finance or operate a mill, none appears.

## 9 Benchmark and falsification plan

These are proposed experiments for the implementation stages. None was added or run in this design pass.

### Reference and measurement

Keep an individually scheduled reference executor with the same fixed-point arithmetic, ordering, random keys, and physical rules. Compare shared execution against that reference on identical initial states and interventions. Hash authoritative facts and accepted operations, excluding presentation caches, worker completion timestamps, and derived layout.

Record the actual machine, CPU power policy, adapter/backend, driver, resolution, display refresh, game build, dirty-source fingerprint, content versions, save, seed, route, and workload. A compile, a headless conservation run, a rendered scene, and user acceptance are different evidence.

Separate population from embodiment: sweep 1k/10k/100k/1m people against 64/256/1,024/4,096 actors where memory permits. Independently vary destinations, changed dependencies, project edits, supply scarcity, and player regions. Report infeasible combinations rather than excluding their hidden cost.

At normal load, record at least 30 minutes after warmup; use a multi-hour soak for storage, memory, and oldest-age growth. Repeat representative runs to expose driver/OS variation. Measure the complete game with terrain streaming, rendering, saves, and multiplayer active, plus subsystem timings for diagnosis.

### Failure matrix

| Experiment | Required result or rejection signal |
| --- | --- |
| Passive camera tours and render LOD changes | Identical authoritative reference history; any production or demographic difference is a failure |
| Worker count, completion order, cancellation, retries | Identical outcomes for deterministic semantic subsystems; stale proposals never overwrite new facts |
| District repartition and execution promotion/demotion | Same identities, balances, commitments, and eligible event history within the claimed exact model |
| Rejected/subdivided stochastic intervals | Retained random realization or declared distributional agreement; no retry-selected luck |
| Last meal, many claimants | One valid consumer/custody result, fair declared arbitration, explainable unmet claims |
| One machine, scarce inputs | No duplicate reservation, output, workspace occupancy, or lost partial product |
| Birth, death, inheritance, migration during handoff | Exact lineage, domicile, custody, and obligations; no resurrection or double resident |
| Bridge destroyed with people and cargo on it | Immediate safety change, actual loss/recovery, no stale successful delivery |
| Shared army goal versus thousands of distinct goals | Measure hit rate, churn, route expansions, memory, and end-to-end readiness; adaptive choice beats or falls back to individual routes |
| Opposing traffic at a narrow passage | Fair throughput, bounded waiting in the feasible case, no unreported deadlock |
| Dense forced crowd in a tiny space | Safe stopped/resolved authority, measured contact cost, explicit overload if needed |
| Mass construction and demolition during streaming | No unlimited dirty, collision, upload, save, or replication backlog |
| Famine, epidemic, strike, or refugees | Scarcity and correlations survive; invalidation and decision storms remain visible |
| Battle in a functioning city | Military costs appear in civilian supply, labor, and care; no disappearing actors |
| Multiple distant players and portal connections | One authority, union interest, coherent causal frontier; capacity includes all active places |
| Cold-city camera and physical teleport | Separate visual readiness from accepted interactive readiness; no fabricated history |
| Long saves, recovery, history generation | No duplicated goods/people, coherent durable boundary, measured disk growth and peak memory |
| Sustained overload | Oldest age and simulation rate exposed; no apparently smooth success with increasing unpaid work |
| Modded production/magic | Declared dependencies or bounded conservative fallback; unknown effects cannot claim analytic advancement |

For approximate models, use independent world seeds as experimental units, with paired common interventions where useful. Start with at least 30 seeds for broad behavior comparisons, then choose more from observed variance; rare famine or collapse tails may require hundreds or more. Thousands of correlated people in one world are not independent samples.

Compare distributions of household food insecurity, mortality causes, migration, wealth/access, travel delay, and recovery time. Include tails, variance, cross-household and spatial correlation, event timing, and response to intervention. Predeclare tolerances before viewing results: for a proposed closure, start with mean service error below 2%, p95 delay error below 10%, and catastrophic-shortage probability within an absolute 0.5 percentage point confidence bound where sample size supports it. These are proposed acceptance criteria, not universal definitions of realism.

An exact optimization must pass the exact comparison; statistical closeness cannot excuse a changed inheritance or duplicated timber.

### Emergence and invention

Run the organization-from-disorder experiment after the first causal slice: 128 residents, heterogeneous skills and preferences, seeded kin/friend pairs, basic survival/communication competence, known local resource patches, and finite starter supplies. Provide no occupations, center, market, government, build agenda, or invisible coordinating agent.

Measure whether people form repeated exchanges, share care, specialize, establish places, and create durable delegated rules. Failure is allowed if its causes are visible. Success requires an evidence chain from local choices, not a desired number of organizations.

Then run several groups facing shared crossings, trade protection, water conflicts, and arbitration. Check formation of cross-group institutions, survival of independent groups, contested authority, and fragmentation. A small work circle is not evidence of a functioning regional polity.

Run the held-out transport invention problem with the final arrangement removed from all plan libraries. Require a bounded proposed composition, a paid experiment, a failure/revision where appropriate, and explicit transmission to another learner. Compare with simple recipe choice.

Ablate communication, social memory, mandates, process sharing, and route caches separately. Removing reservations should be confined to an instrumented failure experiment, not normal play. If removing social mechanisms leaves the same organization sequence, look for hidden scripting. If sharing fails to save net work after bookkeeping, retain the simpler executor.

Finally follow one named person in native first person: meal, work, rest, care/learning, a refused proposal, a disrupted delivery, and resumption after leaving and returning. Inspect the cause of a shortage and the material progress of a building. Visual readability, architecture, and enjoyable intervention require this review; a headless count cannot establish them.

## 10 Implementation sequence

Each stage extends the same ownership model. Preserve existing generator gates and unmodified legacy saves. Baselines and verification below are work for the implementation task, not already completed evidence.

| Stage | Visible delivery and existing owners | Main new contract | Budget and recovery gate |
| --- | --- | --- | --- |
| 1 One continuing livelihood | 24 residents, six households, food source, kitchen/workshop, hauling, rest/care, and one shortage proposal; farming jobs/routine/supply/construction | Saved ProcessCore, inventory adapters, due events, per-effect receipts | Start with 64 live processes, 256 dependencies, 128 reservations, 512 pending events, 256 pending construction cells; no lost progress/cargo across interruption |
| 2 One authority near and far | Same settlement continues unrendered; travel civilization becomes a derived view for migrated sites | Validity intervals, compact geometry queries, common semantic clock | Camera invariance and obstruction behavior pass before increasing population |
| 3 Several hundred people and two settlements | Household transitions, trade journeys, bridge association, crisis recovery | Persistent relationships, domicile transfer, charter and communication | Begin with 512 people and 2,048 processes; fair queues and save/recovery survive supply failure |
| 4 Institutions and regional politics | Disorder experiment, rival associations, maintenance, regional negotiation and fragmentation | Mandates, jurisdiction, information, succession and enforcement | No authored agenda; institutions explain both cooperation and failure; exceptions and fanout measured |
| 5 Measured scale | 1k through 100k people independently of bodies; more active cities | Specialized exact shared routines, navigation cache selection, paged storage | Lower total cost than reference while preserving exact outcomes and all age/memory gates |
| 6 Invention and mobilization | Held-out composition, learning transmission, actual civilian-to-military transfer | Bounded plan search and experimental provenance | New capabilities pay actual costs; care, food, casualties and equipment remain coherent |
| 7 Million-person research envelope | Large persistent populations with bounded active interaction | Only optimizations justified by earlier profiles; optional controlled closures | Full memory/CPU/renderer/network envelope passes; otherwise publish the smaller supported envelope |

For every stage capture the reference scenario first, retain the slow exact path for diagnosis, and record the counterexample that would reject the optimization. Increasing caps without the gate is not a milestone.

### First coding-session handoff

Implement **one carrier's saved delivery and one paid construction continuation inside a 24-resident fixture**, including the existing meal/rest loop and one shortage-derived proposal. The first session must change visible behavior; it must not end with only type registries.

Use the existing crew and supply paths, stable resident IDs, real stockpile chests, effects::replace, and sidecar/checkpoint infrastructure. Choose an already supported terrain/voxel-scale fixture; do not combine the first proof with new planetary geometry.

The fixture begins with authored homes and initial livelihood knowledge, which is allowed for an engineering proof. A household's actual shortage causes its resident to post a small covered-storage or shelter proposal. Other residents can accept or refuse using their own available time and resources. No settlement population threshold starts it.

The baseline records 24 identities, all relevant inventories/cargo, household membership, current work, terrain revision, and the timing of the existing delivery. Record its behavior at a blocked route and across save/load before replacing the path.

Completion scenario:

1. A carrier reserves and picks up real material. A kitchen consumes delivered ingredients and allocates actual meals.
2. A resident leaves work for a real rest/care obligation, preserving cargo and unfinished work.
3. Another resident proposes the small structure after an observed shortage. Accepted contributions and work appear on the board.
4. Save halfway through delivery and after a partially completed construction stage. Reopen at the same phase with the same custody and paid progress.
5. Remove the crossing while cargo is in transit. The carrier waits or reroutes; the destination cannot consume the missing delivery.
6. Leave the area for two game days, then return. Repeat with a passive camera visiting the area. Match the exact semantic outcomes.
7. Inspect the named person's reason, current commitment, delivered quantities, consumed inputs, and next event in the native game.

The concrete acceptance gate is the complete trace with no duplicated stock, lost paid work, observation-driven progress, or unbounded queue age, plus a recorded foreground/worker budget. The design contains these verification requirements; running them needs the later implementation task's scope.

Migration is explicit and one-time. Preserve legacy identity, existing physical cargo, homes, geometry, and actual inventories. Import regional food/material scores only as historical metadata unless a clearly recorded initial endowment is deliberately part of the scenario. Do not silently mint inventory from scores. Legacy banked labor remains provenance, not permission to place unpaid geometry.

Before changing a settlement's owner, fence both old and new services at one semantic tick, reconcile references, write the migration marker, and checkpoint. For that settlement, the old regional update and the new process executor must never both run. Keep untouched legacy worlds on their prior behavior until migrated; this design does not authorize rewriting old generator history.

Later world creation assigns initial identities and state independently of player discovery. Historical settlement initialization is a deterministic creation task with saved results, not an action triggered by whichever player visits first.

### Concrete simplifications

Retire the duplicated daily regional stock/growth writer for migrated settlements. Derive census and faction summaries from real people and organizations. Unify the crew's repeated task state with saved process state when that slice proves equivalent. Cache project obligations by project revision instead of rebuilding them on each worker call. Keep one inventory owner and one authoritative construction effect path.

Do not start by rewriting the renderer, adopting an ECS, adding a universal process compiler, or implementing every political rule. The smallest viable result is a person who continues the same real activity across distance, interruption, and reload.

## 11 Research ledger

Sources below support specific mechanisms or boundaries. A source's own benchmark does not estimate Axiom capacity. No external implementation was imported. Library branches/doc versions were inspected as reference; no dependency commit or release was selected for integration.

| Source | Evidence used and boundary | Decision |
| --- | --- | --- |
| [Emerson, flow-field tiles](https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf) | Original chapter; portal reuse, boundary-cost tradeoff, dynamic repair, sliced integration; broad 3D support listed as future work | Adopt principles; prototype demand-aware reuse on Axiom surfaces |
| [Factorio belts 0.15](https://www.factorio.com/blog/post/fff-176) | Stable relative spacing permits changing a few boundary values instead of every item position | Adopt the search for invariants, not its speedup figures |
| [Factorio multithreaded belts](https://www.factorio.com/blog/post/fff-364) | Dependency groups and ordered wake-up publication matter for determinism | Adopt explicit dependencies and canonical publication |
| [Veloren RTSim actions](https://docs.veloren.net/src/veloren_rtsim/ai/mod.rs.html), [NPC records](https://docs.veloren.net/veloren_rtsim/data/actor/struct.Npc.html) | Inspected 0.18.0 docs and action source: composable ongoing actions, reset/cancellation, reports and sentiments; not a complete Axiom economy | Structural reference; [workspace declares GPL-3.0-or-later](https://github.com/veloren/veloren/blob/master/Cargo.toml); no copied code |
| [Banisch 2014](https://arxiv.org/abs/1410.6277) | Markov structure and symmetry-based aggregation; the abstract was inspected, not a complete proof audit | Use to constrain exactness claims; no general social-cohort shortcut |
| [Equation-free computation](https://arxiv.org/abs/physics/0209043) | Fine simulations can support coarse analysis where an adequate closure exists; abstract-level inspection | Later offline closure experiment, not identity reconstruction |
| [Gibson and Bruck](https://authors.library.caltech.edu/records/zkvwc-5wc86) | Caltech record describes efficient exact next-reaction scheduling; full paper was not retrieved | Scheduling reference; account separately for dependency degree and fanout |
| [Anderson modified next reaction](https://arxiv.org/abs/0708.0370) | Integrated propensities, time-dependent rates, and delays in a specified jump process | Adopt individual hazard/exposure concepts |
| [Cao, Gillespie, Petzold](https://cse.cs.ucsb.edu/sites/default/files/publications/NewTau052.pdf), [Anderson postleap checks](https://arxiv.org/abs/0708.0377) | Leap accuracy requires controlled propensity changes; conditional rejection/refinement matters | No demographic tau-leaping initially; reference for later controlled experiments |
| [Solan and Getz 2025](https://arxiv.org/html/2509.12631v1) | Inspected algorithm, conditional split/resample mechanism, stopping theorem, and code link; no bounded per-frame guarantee | Prototype only after a reference; [linked code](https://github.com/ormorni/sde) returned 404, license unverified |
| [Random123](https://github.com/DEShawResearch/random123) | Deterministic key/counter interface; official source and [BSD-style three-clause license](https://github.com/DEShawResearch/random123/blob/main/LICENSE) inspected | Candidate Philox reference; verify chosen Rust implementation separately |
| [Recast/Detour](https://github.com/recastnavigation/recastnavigation), [D* Lite](https://idm-lab.org/bib/abstracts/papers/aaai02b.pdf) | Official sliced-query and raycast source via Context7; incremental-search paper; Recast's [zlib-style license](https://github.com/recastnavigation/recastnavigation/blob/main/License.txt) inspected | Prototype selected query concepts; no wholesale navigation replacement |
| [Continuum Crowds](https://grail.cs.washington.edu/projects/crowd-flows/), [ORCA](https://gamma-web.iacs.umd.edu/ORCA/) | Primary project descriptions of fields and reciprocal avoidance; neither supplies institutions or bottleneck service rules | Compare locally after traffic/collision authority works; implementation reuse/license not selected |
| [Werfel, Petersen, Nagpal](https://ssr.princeton.edu/publications/designing-collective-behavior-termite-inspired-robot-construction-team) | Local sensing constructs a user-specified target through generated rules | Adopt environmental coordination; do not claim target invention |
| [Ostrom](https://www.aeaweb.org/articles?id=10.1257/aer.100.3.641) | Publication record verified; full lecture/PDF access failed | Framing for multiple institutional forms; detailed Axiom rules are proposals |
| [What Is Intelligence, Origins](https://whatisintelligence.antikythera.org/chapter-01/), [Other Minds](https://whatisintelligence.antikythera.org/chapter-05/) | Relevant passages on recombination, reuse, modular coordination and modeling others | Turn into collaborator-memory and procedure-transmission ablations |
| [Computational Life](https://arxiv.org/abs/2406.19108) | Abstract describes self-replicating programs under particular substrates, including a counterexample | Separate research toy only; not a production civilization controller |
| [FLAME GPU 2](https://github.com/FLAMEGPU/FLAMEGPU2) | Current README describes CUDA execution and declares AGPL-3.0 | Reject as core dependency for the CPU-capable raw-WGPU game; no Axiom speed claim |

The research supports eliminating repeated work, maintaining explicit dependencies, and being conservative about aggregation. The particular combination, its useful capacity, and its social outcomes remain Axiom engineering hypotheses.

## 12 Adversarial review

| Likely failure | Why this design is vulnerable | Rejection experiment and response |
| --- | --- | --- |
| Hidden director returns through opportunity generation | A global query can quietly preselect the correct jobs and buildings | Disorder experiment plus removal of notices/communication. If outcomes remain scripted, move substantive proposal choices back to identifiable residents with limited knowledge. |
| A missed dependency makes cheap advancement lie | A bridge, owner, carer, power supply, or hazard changes during an interval | Randomized intervention/repartition trace against individual reference. Remove that shortcut until its read/hazard set is complete. |
| Crisis destroys all useful sharing | Dense coupling creates wake storms, route repairs, and distinct decisions | Famine plus bridge loss, battle, and multiple players. Measure worst age and service. Keep the simpler executor, reserve essential service, and reduce the advertised supported workload. |
| Physical realization changes the economy | Visible workers wait for collision while distant timers complete freely | Passive-camera and repeated materialization traces with obstruction and crowding. Unify movement/service semantics; if equivalence is impossible, declare and validate the approximation rather than calling it exact. |
| Progress appears correct but persistence or social history is lost | Independent saves, skipped task state, and overaggressive compaction erase causes or duplicate resources | Recovery at every delivery/build/transfer boundary and long-history inspection. Use coherent manifests, persistent process state, durable sequence fences, and retain referenced facts. |

The first four stages can deliver a materially more capable Axiom without any speculative population closure: real households and work, interruption that preserves consequences, local organizations that can fail, physical construction, and a society that continues when the player leaves. Million-person capacity, unfamiliar invention, beautiful unscripted cities, and regional political emergence remain acceptance targets. The architecture earns those claims only through the stated evidence.
