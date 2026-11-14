# AXIOM / MERIDIAN — COMPLETE IMPLEMENTATION DIRECTIVE

**Revision:** Consolidated implementation baseline, 29 October 2026  
**Audience:** Coding agent working inside the existing game repository  
**Status:** Implementation requirements and original design decisions, not a report of implemented features  
**Engine:** Windows-first Rust + raw WGPU; no Bevy; preserve the existing native game  
**Working names:** Axiom is the engineering/project name; Underkingdom is an existing working game title; Meridian names established world/infrastructure concepts. Preserve the repository's actual names rather than renaming the project.

## How to execute this prompt

You are the lead integrator, gameplay engineer, graphics engineer, technical artist, and skeptical reviewer for the existing game. **Implement the game described here inside the actual repository. Do not respond with another design-only document, a replacement engine, an isolated browser demo, or a collection of unconnected prototypes.**

This is the complete target and its implementation contract, not an instruction to attempt every subsystem in one uncontrolled patch. Read sections 0–5 and 22–26 first, audit the repository, select the earliest unmet high-priority gate, and implement a complete, useful slice. Consult the relevant subsystem chapters before touching their code. Preserve later requirements while delivering the current slice. Continue through the dependency-ordered milestones as gates pass. Do not restart completed systems merely to match names or pseudotypes in this document.

A successful session leaves actual code/assets/tests and a reproducible player-visible improvement, or a precisely documented environmental blocker plus the safe work genuinely completed. It never labels tests-only scaffolding as gameplay, a beauty render as native graphics, or a plan as implemented code.

The instructions below are self-contained. Earlier briefs are provenance and useful detail, not prerequisites that force the agent to reconstruct this conversation before every session. Where the audited repository has a more mature compatible implementation, extend it instead of installing a competing one.

## Contents

- [0. Product promise and non-negotiable decisions](#0-product-promise-and-non-negotiable-decisions)
- [1. Definition of a worthwhile playable game](#1-definition-of-a-worthwhile-playable-game)
- [2. Mandatory repository audit and safe execution](#2-mandatory-repository-audit-and-safe-execution)
- [3. Canonical world, ownership, frames, and persistence](#3-canonical-world-ownership-frames-and-persistence)
- [4. Bounded work, responsiveness, and resource lifetime](#4-bounded-work-responsiveness-and-resource-lifetime)
- [5. Multiplayer: deterministic where proven, responsive everywhere supported](#5-multiplayer-deterministic-where-proven-responsive-everywhere-supported)
- [6. Planetary geology, history, and coherent world generation](#6-planetary-geology-history-and-coherent-world-generation)
- [7. Art direction, assets, and inhabited visual identity](#7-art-direction-assets-and-inhabited-visual-identity)
- [8. Renderer, LOD continuity, streaming, and loading](#8-renderer-lod-continuity-streaming-and-loading)
- [9. Coupled 3D materials, structural building, and destruction](#9-coupled-3d-materials-structural-building-and-destruction)
- [10. Construction, inventory, crafting, and early convenience](#10-construction-inventory-crafting-and-early-convenience)
- [11. Science, machinery, energy, and player-designed electronics](#11-science-machinery-energy-and-player-designed-electronics)
- [12. Magic: expressive, physical, powerful, and understandable](#12-magic-expressive-physical-powerful-and-understandable)
- [13. Ecology, inhabitants, institutions, and a useful economy](#13-ecology-inhabitants-institutions-and-a-useful-economy)
- [14. Exploration, navigation, dungeons, enemies, and bosses](#14-exploration-navigation-dungeons-enemies-and-bosses)
- [15. Cosmology, parallel dimensions, and realm rules](#15-cosmology-parallel-dimensions-and-realm-rules)
- [16. True immersive portals and cross-frame transfer](#16-true-immersive-portals-and-cross-frame-transfer)
- [17. Player-built voxel spacecraft, flight, and orbital construction](#17-player-built-voxel-spacecraft-flight-and-orbital-construction)
- [18. Meridian infrastructure, radio, skyhooks, and real cargo](#18-meridian-infrastructure-radio-skyhooks-and-real-cargo)
- [19. The Hall of the Crimson King — persistent multiversal nexus](#19-the-hall-of-the-crimson-king--persistent-multiversal-nexus)
- [20. The flagship integrated region and its first real journey](#20-the-flagship-integrated-region-and-its-first-real-journey)
- [21. Interface, sound, accessibility, and modding](#21-interface-sound-accessibility-and-modding)
- [22. Delivery sequence and dependency gates](#22-delivery-sequence-and-dependency-gates)
- [23. Performance, correctness, and visual evidence contract](#23-performance-correctness-and-visual-evidence-contract)
- [24. Acceptance catalog](#24-acceptance-catalog)
- [25. Required deliverables and session reporting](#25-required-deliverables-and-session-reporting)
- [26. Source context, precedence, and traceability](#26-source-context-precedence-and-traceability)

---

# 0. Product promise and non-negotiable decisions

## 0.1 The game

Build a cohesive standalone, editable 3D voxel sandbox combining the creative accessibility of heavily modded Minecraft, systemic material interactions and spell construction inspired by Noita, satisfying industrial automation, exploratory dungeons, expressive creatures, and a persistent inhabited world.

**The defining experience:** the player crosses a beautiful country, gradually recognizes the immense machine-city within its landscape, builds a worthwhile home among people with their own lives, and develops the knowledge, magic, industry, and vehicles to explore beyond it. Returning home remains valuable after discovering orbit and other worlds.

The setting is a deeply layered magitech ecumenopolis, buried, altered, recolonized, repaired, and partly lost across immense time and many regional or planetary resets. Not every ruin is hostile, every ancient device superior, every myth an AI malfunction, or every community waiting for the player to save it. Some mountains are simply mountains. There are prosperous new places, quiet orchards, competent craftspeople, warm workshops, dangerous tombs, uncanny realms, and truly exceptional megastructures.

A new player can already mine, build, store, grow, cook, explore, and use a simple spell without first completing a planetary simulation or a semiconductor factory. A veteran can develop sophisticated circuits, factories, living machinery, vehicles, rituals, orbital infrastructure, and powerful combinations without invalidating peaceful play.

## 0.2 Fixed constraints

1. Extend the **existing Windows-first Rust/raw-WGPU engine**. No Bevy, engine replacement, separate authoritative game simulation, mandatory Docker workflow, or web-demo substitute.
2. Preserve existing worlds, identities, construction, content, and working interactions. Migrate deliberately with backups and versioned fixtures. Never silently regenerate saved land or erase player modifications.
3. Use one canonical authority for consequential world facts and ownership. Meshes, screenshots, render proxies, predictions, and UI state are not independent truth.
4. Provide actual local physics on relevant clients, authoritative shared outcomes, and **verified deterministic command replication as the preferred normal mode for supported complete domains**. Keep explicit state-synchronized fallback and bounded recovery. No global slowest-client barrier.
5. Keep simulation, memory, streaming, input histories, derived work, and resource retirement bounded. Preserve already accepted consequences; reduce optional rendering before physical truth.
6. Keep art and gameplay unified: deliberate pixel-material language, painterly colour and silhouettes, modern spatial lighting, real working architecture, beautiful player construction, and authored original assets.
7. Ship useful end-to-end slices with native evidence. Compiling an enum or registering an item does not finish a feature.
8. Make homes, storage, ordinary tools, and basic automation approachable. No compulsory AFK, hostile-mob grinder, forced cruelty, or boss victory for basic comfort.
9. Build **one rich homeworld and a real orbital round trip before broad planetary multiplication**. Preserve the complete destination and realm design as a staged target, not a promise of simultaneous implementation.
10. First self-built crewed departure from the atmosphere belongs around **60% of the main capability progression**, with a reliable orbital return around 60–65%. This is pacing, not a runtime research-percentage barrier. Early limited gliding remains available.

## 0.3 Reconciliation register — these choices supersede conflicting drafts

| Conflict in earlier material | Decision in this directive |
|---|---|
| Global crimson space versus a parallel hell cosmos | The deep dark crimson cosmos defines **Crimson Mirror**. Material worlds retain coherent varied skies and astronomical space. |
| One material/hell pair versus many parallel worlds | Separate universe families, realms, bodies, frames, and explicit transit edges. Multiple parallel realm families are supported. |
| Crimson counterparts share a sky, so ships can fly between them | They are isolated planet-associated pockets. Local flight may work; ordinary flight and ordinary forged gates cannot connect different pockets. The Hall is the exceptional nexus. |
| Null Choir disables electricity | Superseded. Null Choir suppresses sustained magical bindings. Ordinary electricity, mechanics, and chemistry still work. A magically initiated ordinary fire persists while physical fuel permits it. |
| Early snapshot-only client physics versus later deterministic requirement | Prefer exact accepted-command replication after whole-domain replay and restore gates pass. Retain locally simulated state sync for incompatible/open domains; do not remove a working fallback prematurely. |
| Determinism means all peers must wait | False requirement. Authority seals and advances ticks without waiting for every peer; one slow client has bounded prediction/recovery. |
| GPU-driven rendering versus CPU-authoritative materials | Compatible responsibilities. Preserve the working GPU rendering path; canonical gameplay/collision remains reproducible. GPU visual computation need not be authoritative physics. |
| A new planet pipeline should replace existing terrain | No. Audit and extend current ownership/topology. Use migration or adapters only where proven necessary. |
| Fernwake, Windward Basin, Orchard, and Drydock as competing new starts | Preserve them as compatible existing/proposed regional sites and fixtures. Choose the verified starting fixture, link suitable locations, and never overwrite established coordinates to force a new name. |
| Gliders arrive near spaceflight | Later context explicitly favors an approachable early hang glider and one elastic movement tool. Neither supplies unlimited energy or indefinite effortless flight. |
| Every spacecraft requires wings | The introductory atmospheric shuttle does. Wings/control surfaces must actually work in air; vacuum-only craft need not carry decorative wings. |
| Airtight ship means every cavity must be pressurized | Only intended occupied/pressurized compartments require a pressure boundary. External assemblies and engine bays may be open. |
| A destination is visually loaded, so crossing is safe | It must be simulation/collision/ownership ready. A pending portal is visibly closed and blocks traversal; it is not a noncollidable opening into absent terrain. |
| Graph-connected means structurally strong | Connectivity and load-bearing strength are distinct. Both need explicit supported models. Unknown connectivity is not proof of detachment. |
| Expensive actions may lose damage when queues are full | Reserve supported consequences before irreversible activation. Never drop an admitted projectile's impact or duplicate/refund costs through visual reconciliation. |
| Concept dimensions, frame rates, voxel sizes, part counts are certified | Earlier numbers are proposals unless verified in the repository. Keep pacing intent; establish technical limits from real measurements. |
| Every impressive dungeon must resist all digging/flight | Preserve legitimate bypasses. Specific readable materials, fields, pressure, ownership, or mechanisms may impose costs; no blanket script-protection immunity. |
| More detailed art means photorealistic textures or more noise | Preserve painterly massing and authored pixel clusters. Geometry, material, lighting, and composition solve different problems. |
| Millions of years implies pristine intact labs everywhere | Ordinary fabric is often lost. Explain exceptional preservation through burial, durable materials, continuous repair, biological maintenance, or explicit magic. |
| A complete directive means all features are already implemented | Nothing here certifies source/runtime status. Audit, implement, test, and report each feature separately. |

Latest explicit user intent and correctness/ownership requirements take precedence. This consolidation resolves the known conflicts; a genuinely new contradiction requires a short recorded decision, not a silent deletion of a product promise. New illustrative place names remain design proposals until integrated with established canon.

---

# 1. Definition of a worthwhile playable game

## 1.1 Six valid player intentions

Support the builder/homesteader, explorer/archaeologist, industrial engineer, mage/experimenter, naturalist/cultivator, and pilot/expedition organizer. These are overlapping preferences, not mutually exclusive classes. Cooperative friends can specialize without a forced role composition.

The fundamental loop is: obtain information or materials; choose a personal objective; construct or experiment; go somewhere interesting; change or understand a real system; return with a capability, relationship, route, specimen, design, useful object, or experience. Commodity loot is only one reward type.

A quiet session gardening, organizing a workshop, visiting neighbours, or surveying a ridge is valid. An optional dangerous delve can be demanding. Do not convert every peaceful improvement into endlessly escalating raids or chores.

## 1.2 Familiar tool and construction completeness

Provide the familiar functional vocabulary expected from Minecraft-like play without copying protected assets or claiming a live exhaustive item-list match: mining pick, axe, shovel, hoe, sword, bow/crossbow, shield, shears, bucket, fishing tool, ignition/torch, ladder/rope, map/compass, chisel, repair wrench, basic instruments, and wand. Implement harvest tags, range, durability/repair where useful, material response, correct drops, and readable failure feedback.

Hotbar, inventory, recipes, workbench, furnace/smelter, containers, farming soil, basic food, safe lighting, doors, stairs, beds/rest, and creative placement must work together. Do not postpone ordinary convenience to advanced research. Surface feedback, controller state, selection ray, collision, and authoritative edit results must agree.

## 1.3 Progression and failure

Use a capability graph, not a mandatory single recipe staircase. Discovery, trade, salvage, commissioning, cultivation, and fabrication can provide alternate paths under explicit economy rules. Advanced control rewards understanding, but defaults make ordinary tools effective.

Food supports useful preparation and sustained activities rather than continual nuisance meters. Death/recovery rules must not create an unavoidable equipment-loss spiral. Threats have causes, warnings, counterplay, and refuges. Secured homes can stay secured. Offer explicit server/world difficulty and PvP settings rather than secretly changing rules per player.

A successful first 90-minute fixture supports several independent intentions, a useful shelter and storage, a real machine or spell, an optional physical mystery with alternatives, an attractive route, and a persistent improvement. It must not require completing a scripted main quest.

---

# 2. Mandatory repository audit and safe execution

## 2.1 First actions

Inspect actual root instructions, workspace manifests, branch and HEAD, working-tree changes, tracking documents, source ownership, save/generator versions, asset conversion/import, renderer passes, network transport, physics backend, and executable launch/capture tools. Locate existing tests and fixtures before creating replacements.

Produce a compact baseline table with rows for terrain/LOD/shadows, voxel edits/collision, materials, storage/crafting, machines/electricity, wands, vegetation/creatures, NPCs, world history, navigation, multiplayer, vehicles, portals, orbit, and asset production. Each row records real paths/symbols, authority/save owner, status, automated evidence, and native evidence. Status vocabulary: **reported only, inspected, implemented, automated-tested, native-observed, playtested, performance-validated, blocked**. These are not interchangeable.

Historical reports about generator/save versions, untracked crates, shader bugs, blocked suites, netcode completeness, or test totals are leads to investigate, never current facts. Reproduce reported freezes, horizon holes, black shadow patches, weak first-person models, and performance failures before asserting their cause.

## 2.2 Protect concurrent work

One writer per active hot path; coordinate shared shader/worldgen/authority interfaces. Use ownership-scoped worktrees where available. Never reset, clean, force-push, blanket-stage, or delete other agents' changes. Avoid unrelated formatting of shared files. Track all new source/assets required by the change; a clean checkout must not depend on invisible local files.

Use small reviewable changes. A necessary refactor must identify the old behavior preserved, the replacement boundary, migration cost, and tests. Do not introduce a second inventory, second world clock, second cargo ledger, or second physics authority for convenience.

## 2.3 Implementation quality

Prefer clear Rust ownership, typed identifiers/units, compact data, checked arithmetic, explicit error handling, stable serialization, meaningful APIs, and testable pure kernels. No giant god module, universal service locator, speculative framework for every possible feature, or unnecessary dependency takeover. Keep hot paths allocation-aware without sacrificing safety by default.

Inspect pinned dependencies and current primary documentation before version-specific changes. Verify licenses before reusing code/assets. Existing researched API versions are historical notes, not instructions to upgrade everything. Benchmark a proposed replacement against the actual workload.

Run formatting, targeted tests, appropriate checks/lints, shader validation when relevant, import validation, save/reload, and native interaction. Record full-workspace failures separately from the changed scope. Do not rewrite tests to conceal unrelated defects.

## 2.4 First deliverable, not first bureaucracy

After a concise audit, implement the smallest complete improvement that clears the highest-priority real blocker. Default-preset instability, corrupt saves, ownership duplication, unsafe collision, or a missing repeatable fixture outrank optional new realms. Once those foundations are sound, prioritize the integrated home/stream/gallery/workshop loop rather than another isolated tech demo.

Do not spend the whole implementation session authoring another enormous design document. This document is the design baseline; a short local design note is enough for the selected change.

---

# 3. Canonical world, ownership, frames, and persistence

## 3.1 Distinct identities and responsibilities

Map these responsibilities onto existing types; names below are conceptual, not a mandate for new wrappers everywhere:

- World identity and generation/content/simulation manifests.
- Universe family, realm instance, celestial body, local frame, region/brick address.
- Stable entity with generation; actor/account identity distinct from a transient connection.
- Vessel/assembly, compartment, material lot, inventory/container, consignment.
- Site, household, journey, institution, history event, knowledge observation.
- Operation/effect/transfer identity; authority session epoch; domain epoch and membership revision.
- Canonical terrain/material/topology revision; derived-product key and device generation.

A person, cargo lot, vessel, consequential fragment, or portal transfer has one persistent identity and one current authoritative owner. An object may have multiple views, never multiple spendable or damageable copies.

## 3.2 Coordinate contract

Keep exact hierarchical/dyadic coordinates for voxel ownership where suitable; high-precision body/frame transforms for orbital positions; bounded local coordinates for physics; camera-relative floats for GPU work. Subtract high-precision origins before converting to f32. A camera rebase is not a physics rebase.

Bodies, orbital parents, ships, rotating stations, portal endpoints, passengers, and local gravity domains use explicit frame relationships and a shared epoch. A shared solver-origin change is a versioned authority event. Preserve position, orientation, linear/angular velocity, attachments, and appropriate inertial effects across transitions. Do not reset motion on landing, docking, portal crossing, or streaming.

Local rotation terms matter. Tests must cover off-center points on spinning bodies, both endpoint motions, large coordinate magnitudes, and changing centers of mass. Graph cycles or nonfinite transforms fail validation explicitly.

## 3.3 Immutable baseline plus changing facts

Separate versioned immutable generation facts, authoritative mutable overlays/events, and disposable derived products. Persist macro geography, drainage, historical sites, route identities, and current edits. Visiting regions in another order must not reroll rivers, inhabitants, buildings, loot, or signatures.

Derived meshes, visibility summaries, render proxies, navigation accelerators, and cooked device caches carry precise source-projection versions. The renderer does not write canonical material. A mesh completion cannot resurrect an old wall or replace newer edits. Render simplification cannot change collision, tool reach, loot, pressure boundaries, or cover.

Track **known empty**, **known occupied**, **unknown**, and **stale** separately. Unknown geometry is not empty for a bullet and is not a valid airtight wall. Conservative acceleration may nominate possible collision, but the final interaction must use the appropriate ready canonical geometry.

## 3.4 Transaction and effect semantics

Use the existing authority command path for edits, crafting, transfers, repairs, batch stages, purchases, pickup, docking, portals, and cargo operations. Requests carry stable operation keys and actual dependency/read-set identities. Validate actor permissions, range, object generation, resources, capacity, and relevant revisions.

Distinguish Requested, Predicted, Admitted, Applied, Rejected/Corrected, and Durable. A transport ACK is not gameplay acceptance; Applied is not necessarily crash durability. Stable effect IDs prevent duplicated damage, costs, loot, sound, or particles during replay/retry. Reusing an operation ID with different content is invalid.

Deduplication survives reconnect and journal compaction. Expired reply detail cannot turn an ancient completed operation into a new purchase. Session epochs fence stale traffic, not reset persistent operation identity. Exact once is a logical ownership guarantee implemented with idempotency, atomic recorded transitions, and recovery, not a claim that networks never duplicate packets.

Multi-resource operations declare atomic versus staged behavior. Reserve outputs/inputs before irreversible stages. A failure may leave a documented partial process state, but must not silently lose input or deliver output twice. Creative creation is an explicitly privileged source, not an accidental survival loophole.

## 3.5 Saves, checkpoints, and clocks

Separate short rollback history, coherent join/recovery checkpoints, and durable saves. Durable snapshots cover a declared tick and ordered journal frontier, then a bounded suffix. Complete and validate a new durable recovery point before retiring the old one. Test interruption before/after mutation, journal persistence, and receipt publication.

Persist exact generation/simulation/content versions, local frames, portal graph, actor/cargo ownership, plant changes, factory partial batches, social facts, and realm progression. Migrations are backed up and tested; unsupported changes receive explicit compatibility handling. No silent reset because the renderer cache changed.

When exporting a baseline, atomically capture the snapshot/frontier and register retention of subsequent commands before serialization/compaction can race it. Candidate publication cannot move an installed frontier backward. Corrupt cache data is disposable; corrupt canonical saves require safe recovery, never silent fabrication.

Use one simulation clock with explicit scheduled events. Environmental rate modifiers do not change networking tick duration or duplicate clocks. Offscreen progression uses valid conserved coarse models; no free fuel, ignored hazards, restarted batches, or unbounded replay of every unseen frame. Pause policy is explicit for solo play versus active shared sessions.

---

# 4. Bounded work, responsiveness, and resource lifetime

## 4.1 Bound the whole action, not only its first function

An edit can induce collision, structural splitting, fluid/pressure changes, navigation, light, loot, persistence, networking, meshing, uploads, and retirement. Bound and reserve those consequences together before irreversible activation. Use compact spans/masks/strokes and affected regions; no entity/message/job chain per voxel.

The common path is intent validation → bounded footprint/dependency preflight → capacity reservation → preparation → read-set revalidation → authoritative activation at a declared tick → coherent publication → optional refinement → safe retirement.

Already launched projectiles, paid casts, and activated mechanisms retain their supported consequences. Large actions may use an explicitly authored staged propagation model with costs and interruption rules. Reject unsupported requests before activation rather than promise unlimited work and later drop damage.

Physical complexity includes contacts, constraints, active liquid neighbors, pressure solves, surface fragmentation, and topology output—not merely body or voxel count. Existing bodies can converge into an expensive pile without spawning new ones; the supported model must budget that possibility.

## 4.2 Fair scheduling and distinct clocks

Keep render, input, audio, networking, authority ticks, and derived worker scheduling separate. Fixed simulation work/iterations cannot depend on a local wall-clock timer or render FPS. Authority may schedule a not-yet-activated operation as a logged event; all replicas apply the same rule and tick.

Each queue has limits on bytes, estimated work, active jobs, oldest age, and maximum single task cost. Give old required work progress and interactive control reserved service. Coalescing may replace obsolete reconstructible products, never accepted gameplay history. Prevent repeated cancellation from starving the only current collider or mesh forever.

Long tasks must be resumable or split before running; checking time after a huge blocking call does not prevent a hitch. Bound all background consumers together, including generation, decompression, physics workers, pipelines, saves, and upload preparation. Avoid nested pools each taking every core.

After a burst, backlog must drain under continuing supported activity. Record queue age and authoritative state age, not only FPS. Persistent hidden lag is a failure. A finite queue can still starve a player indefinitely; fairness is part of correctness.

## 4.3 Whole-job resource leases

Derived jobs conceptually move through Requested → Reserved → Producing → Ready → Submitted → Resident → Retiring → Reclaimed. Cancellation changes intent; it does not free memory still read by CPU workers or the GPU.

Before production reserve conservative simultaneous additional backing for retained inputs, CPU scratch/output, ready-queue slots, staging, destination GPU allocation, replacement overlap, and completion bookkeeping. Transfer ownership/credits through stages without double counting. Shared data needs pin leases, not imaginary duplicate backing charges.

Stable logical handles include generation/device epoch. Publication rechecks exact read sets. Completion callbacks are short, nonblocking, lossless notifications; stale callbacks cannot reclaim newer reused handles. Retirement must make progress even when rendering is minimized or producers are stopped. Do not occupy worker slots waiting for upload space.

Handle canceled encoders, unsubmitted staging, window resize, minimized windows, device loss, and failed allocations explicitly. No infinite retry loop, wait for callbacks from a dead device, or freed range reused before its last submission completes. GPU recovery must not erase listen-host authoritative state.

## 4.4 Memory policy

Track total CPU and GPU process pressure, not only currently visible terrain. Include allocator backing/fragmentation, textures and mips, shadows, render targets, histories, portal views, buffers, staging, old/new overlap, snapshots/COW, queued products, and delayed retirement.

Use backend-supported native budget observation where available, plus a conservative tracked ledger and explicit fallback when telemetry is unavailable. WGPU buffer-size limits are not a free-VRAM report. Avoid counting the same UMA backing twice.

A starting experimental policy from prior work is soft pressure near 70% and a stop-new-growth admission ceiling near 80% of the relevant current process budget, with a user-lower cap. **These are tunable safety margins, not preallocation targets, guarantees, or validated capacities.** Reserve essential correct rendering and patch capacity before optional detail. Account for abrupt budget reductions and transition peaks.

Evict speculative or distant optional products first. Lower optional shadows/reflections/portal resolution/view detail using bounded transitions; never remove physical cover or change one client's solver fidelity. Dropping a logical handle, choosing a lower mip, or drawing fewer objects does not prove backing memory was reclaimed.

Reproduce the reported default-preset freeze. Memory pressure is a hypothesis until traces establish the cause. Diagnose allocation/lifetime leaks, sync waits, pipeline creation, excessive work, and driver/OS behavior separately. No claim of a universal zero-freeze engine.

---

# 5. Multiplayer: deterministic where proven, responsive everywhere supported

## 5.1 Authority and operating cost

Use a player-hosted listen authority and the same simulation core for optional community/user headless servers. Reuse a sound existing transport. No mandatory developer-run world-simulation fleet, peer consensus, or unsolicited transport rewrite. Direct connectivity is preferred when effective; relay fallback has explicit capacity, cost, authorization, observability, and failure behavior. Do not assume public relay bandwidth is unlimited or free.

A listen host remains privileged; hashes are not an anti-cheat proof. Validate clients' rates, resources, positions, permissions, and action consequences. PvP trust and moderation policies differ from friendly cooperative worlds and must be explicit.

A host departure initially means a documented save/session end or explicit reconnect policy, not an implied seamless host migration. Add migration only after durable state transfer, authority-epoch fencing, ownership, peer trust and interrupted-handoff tests exist. Never let two hosts continue authoritatively in a partition and merge conflicting inventories later by guesswork.

## 5.2 Three replication modes

**Exact command-replicated domain:** authority and subscribed clients step the same complete canonical domain from compatible state, applying accepted commands and declared external events. Healthy play uses commands, receipts, digests, and boundary events rather than routine pose streams for every body.

**State-synchronized, locally simulated domain:** required where compatibility, hidden information, coupling, or unsupported exact restoration prevents the first mode. Clients still simulate relevant local interaction and reconcile coherent state. Expose mode and reason; fallback is not an invisible excuse to abandon determinism.

**Coarse distant record/presentation:** entities and factories outside local interaction use valid authoritative aggregate/event models. Clients receive permitted summaries; no need to run every world locally.

Mode transitions have epochs, baselines, and activation ticks. State sync does not solve an oversized physical workload. Enforce a supported common model separately.

## 5.3 Define and prove exact domains

A domain is not a camera radius or arbitrary subset of one host solver. It includes all future-affecting state: relevant terrain and collision, bodies, contacts/caches, joints, controllers, attached cargo/passengers, materials, pressure/fluid state, RNG, fields, and boundary inputs. Reproduce the same canonical solver context and creation/removal order.

The compatibility manifest records simulation/physics/content/mod hashes, numerical rules, rounding/overflow, tick and fixed substep policy, shape construction, solver settings, RNG streams, stable ordering, and frame conventions. Test the entire application, not only a backend's determinism switch.

Use canonical event/reduction/query order and purpose-keyed RNG. Camera, worker completion order, unordered maps, render rate, and cosmetic effects cannot choose gameplay outcomes. GPU visual computation may differ; authoritative GPU material work requires its own validated deterministic or state-sync contract.

Prepare a merged domain before two-way contact, joints, fluid pressure, or other undeclared forces couple separate domains. Split only when dependencies permit, publishing complete new baselines. Use hysteresis and swept interaction bounds; reserve merged workload and snapshot peaks. Do not sever ropes, cargo, or fluid coupling at an interest boundary.

Keep sensitive strategic information server-side. Do not disclose hidden inventories/bases to every client merely to simplify a digest. Where undisclosed information changes local physics, publish an authorized event/state boundary or reveal it under a declared rule. Exact reconstruction cannot derive hidden inputs it was never given.

## 5.4 Ticks, input, and slow clients

Authority seals ticks with ordered accepted-command ranges and explicit completeness/no-event watermarks. It does not wait for all peers. Missing continuous controls use a bounded hold then neutral policy. Missing one-shot input never repeats the previous shot, trade, cast, or pickup.

Clients maintain distinct predicted and confirmed frontiers. A watermark beyond a hole does not fill missing accepted commands. Prediction is finite in ticks, bytes, dependency depth, and replay work. On exhaustion, keep UI/presentation responsive while that client repairs or loses interaction readiness; others continue.

Maintain clock mapping/drift uncertainty without changing the solver timestep or dumping obsolete movement after alt-tab. Domain membership and authoritative rebases use the shared clock; visual interpolation never becomes collision state.

## 5.5 Delivery, baselines, and recovery

Use a few bounded service lanes: urgent freshness-sensitive controls; ordered repairable accepted commands/outcomes; reliable membership/control/receipts; and credit-limited chunk/checkpoint/content streams. Separate bulk from control without assuming more sockets create bandwidth. Account for the actual transport's congestion, buffering, reliability, and relay head-of-line behavior.

A delta references a baseline the receiver reports installed and retained, not merely the last one sent. Candidate baseline installation includes the coherent snapshot, retained ordered suffix, collision, necessary assets, and correct minimal presentation. Fence stale transfers; atomically recheck epoch/frontier at publication. No old snapshot may overwrite newer installed edits.

Restore a manageable complete domain, not the whole planet. Full solver and external dependencies matter; position/velocity alone may not be enough. Bound COW pinning, checkpoints, hashes, resends, suffix length, decompression, and replay. If replay cannot catch the advancing authority, use a newer prepared baseline or a smaller genuinely closed initial domain; no endless restart or global pause.

## 5.6 Responsive terrain and dependent physics

Maintain confirmed state plus a bounded ordered speculative overlay. The initiator's next selection ray, character motion, affected crate contacts, attachments, and current visual patch must read the same predicted branch. A visible hole with an old solid collider is not responsive editing.

Each action includes target generation, relevant read sets, and dependency IDs. An unrelated change elsewhere in a chunk should not invalidate a legitimate stroke; replacement of its target with another object must. Continuous mining has shared cadence/progress and bounded swept sampling, independent of render FPS.

An Applied receipt does not remove the speculative patch until its canonical result is installed. This prevents a wall flashing back between confirmation and data arrival. Rejection removes/replays the affected dependency branch, never restores a stale whole chunk over other players' changes. Predicted loot is not spendable inventory.

Fast correct geometry must expose new surfaces, materials, depth, and collision; merely hiding old faces creates voids. Invalidate stale visibility/shadow histories. Expensive optimized meshes follow under revision checks. Complex collapse prediction is limited to supported coherent domains; otherwise present provisional feedback honestly while authority performs the scheduled topology change.

## 5.7 Combat and changing cover

Cooperative worlds may use current-state adjudication. For competitive play, use one explicitly configured bounded history policy with authority-selected current/target-observation ticks and actual cover revisions. A candidate policy checks both historical and current cover, with narrow exemptions for exact accepted prior break dependencies. This is a gameplay compromise to playtest, not a claim of perfect historical physics.

Never authorize a shot through rejected predicted cover. A new block at the same coordinate has a new generation and is not exempt because an older block was removed. Current muzzle, ammunition, cooldown, and accepted dependency order remain validated. Finite-speed projectiles step forward from their accepted firing tick with swept collision, not repeated target rewind at each contact.

History queries do not rewind inventories/factories. Missing history invokes an explicit authority-selected current-state fallback for the whole query; clients cannot independently choose a different result. Record compact accepted hit/damage events with stable IDs. Replay cannot duplicate costs or kills.

## 5.8 Networking proof before optimization claims

Start with two clients, one shared crate, placement/breaking, storage contention, a small mechanism, checkpoint continuation, and reconnect. Test at least two physical supported machines, worker-count variation, render-rate variation, reordered background completion, and long matching runs. Only then disable routine state packets for that validated domain.

Expand to four players: exploration, factory/fluid operation, combat/editing, and join/reconnect simultaneously. Include LAN and configured 40/80/160 ms RTT, asymmetric delay, jitter, 1–5% loss, burst loss, reordering, duplication, constrained upload, and forced relay as test conditions—not already achieved support claims. A paused client must not pause the others.

Measure full traffic including baselines, repairs, command redundancy, headers, and retransmission; prediction latency separately from authority confirmation and durability; correction magnitude/frequency; state age; and replay memory/CPU. A low ping with a growing installation queue fails.

# 6. Planetary geology, history, and coherent world generation

## 6.1 One planet across all scales

Use the existing viable sphere/region topology. Keep explicit mappings among strategic surface cells, body-local terrain, exact voxel addresses, local tangent patches, orbital coordinates, and map display. A flat map projected onto a sphere is not sufficient canonical spherical geography.

Pole neighborhoods, cube-face seams or exceptional graph cells need correct area/metric/adjacency, drainage, roads, ownership, shadows, and edits. Avoid assuming every strategic cell is identical or that an approximate parent polygon owns exact voxel coordinates. A world wrap is a display convention, not an impassable boundary.

Use a bounded, deterministic body manifest: radius and gravity model, rotation/axis, atmosphere, water/volatiles, geology, orbital parent, ephemeris parameters, seasons, and explicit magical exceptions. Compact planets may be deliberately fictional; surface gravity, orbital calculations, atmosphere, and flight must use the same declared model.

## 6.2 Causal generation pipeline

Implement inspectable stages with stable, independent seeded streams:

1. Global body topology and geological provinces: uplift, basins, faults, intrusive/extrusive bodies, lithology, resource origins.
2. Climate and weather statistics: latitude, elevation, moisture transport, rain shadows, seasonal behavior, local exceptional fields.
3. Connected drainage and geological aging: catchments, spill heights, river courses, endorheic basins where intentional, erosion/deposition, appropriate glacial/coastal histories.
4. Soils, karst/aquifers, ecological capacity, vegetation communities, habitat corridors.
5. Historical settlement suitability, transport modes, major civic/industrial infrastructure, and bounded consequential history.
6. Site/district plans, streets and circulation, foundations, public/private spaces, utilities, and human-scale architecture.
7. Collapse, burial, selective salvage, resettlement, later magical/technical alterations, present ecology and use.
8. Local volumetric refinement that respects established rivers, caves, strata, contacts, sites, roads, foundations, and shared boundary conditions.
9. Persistent canonical results and validation; derived near/far rendering products.

Coherent erosion/geology is a default-world requirement, not an expensive switch users must discover to avoid noise hills. Compute expensive aging in bounded generation/preparation, cache canonical outcomes, and refine locally; do not simulate millions of years whenever a chunk loads or rain falls.

Do not make later noise erase a river, reverse drainage arbitrarily, break roads at seams, or turn supports into ungrounded scenery. Do not create one independent "theme generator" for each valley.

## 6.3 Volumetric terrain and material identity

Generate actual arches, caves, overhangs, ravines, cliffs, strata, springs, sinkholes, basalt columns, exposed contacts, and root pockets where supported by regional causes. They require real empty space, collision, shadows, and editable material—not painted cavities or a heightfield pretending to contain tunnels.

Separate soil/grass/gravel/rock visually and physically. Geology determines ore association and concentration; arbitrary uniform ore grids are not the only discovery path. Include surface evidence, sediments, surveys, instruments, and old excavations. Materials retain properties through mining, crushing, smelting, and salvage.

Map/detail representations must describe the same edited world. A low-detail heightfield may represent suitable unedited distant ground, but not fill an edited canyon, inhabited overhang, bridge, flying structure, or newly opened cave whose topology matters. Preserve volumetric exceptions and source revision coverage.

## 6.4 A palimpsest, not neat dungeon layers

Founders, Choristers, Engineers, and present societies are overlapping historical/construction traditions, not mandatory species or depth tiers. History changes use: a coolant manifold becomes an instrument, then communal heat, then a shrine. Large resets vary in cause, area, and consequence. Most regions remember incomplete and conflicting accounts.

Bound historical simulation through meaningful events with prerequisites and outcomes: foundation, route change, flood, migration, invention, scarcity, separation, conflict, abandonment, repair. Persist important people/cohorts, institutions, ownership, route and facility changes. No need for a full simulated biography of every person who ever lived.

Maintain fact versus interpretation. An old map can be outdated and a chronicle biased, but the underlying event graph is consistent. Contradictions are deliberate investigable perspectives, not random lore generation. No runtime LLM dependency for canonical terrain, saving, or ordinary NPC dialogue. Authored bounded grammars and recorded facts are sufficient.

Leave true nature and uncertain origins. Not every god, folklore tradition, spirit, or magical event is secretly an ancient computer. Handle locally situated traditions with care and avoid turning living cultural beliefs into a checklist of interchangeable monsters.

## 6.5 Cities before monuments

Plan the transport skeleton and functioning precinct before dressing parcels: water access, market, homes, services, work, freight, bridges, quays, public space, and alternate circulation. Streets follow terrain and intended users. A cart road needs turning/grade clearance; specialized inhabitants need appropriate access without making every doorway universal.

Generate loops and useful shortcuts, not only a minimum spanning tree of chokepoints. Planned industrial extensions, contour-following streets, older markets, canal districts, and repaired terraces have different layouts. Reject unusable sliver parcels rather than filling every polygon with a building.

Rooms, stairs, roofs, entrances, drains, storage, and access must function. A convincing door is a promise: usable, visibly blocked, or meaningfully locked. Do not surround one usable showcase room with thousands of fraudulent doors.

Support intact and expanding towns as well as abandoned districts. Modern craft may improve ancient systems. Growth uses actual parcels/materials/access and never overwrites player land. Coarse simulation and nearby geometry refine the same identities; loading a city must not spawn duplicate households or cargo.

---

# 7. Art direction, assets, and inhabited visual identity

## 7.1 Style responsibilities

Use the supplied references to solve distinct problems:

| Inspiration family | Adopt | Do not import |
|---|---|---|
| Hyper Light Drifter/Breaker references | Graphic silhouettes, coherent colour masses, gesture, selective saturation, expressive contrast, striking negative space | Copied hero costume, exact characters/weapons/architecture, universal cyan-magenta grading, every inhabitant wearing the same scarf |
| Classic landscape composition | Foreground shelter, middle-distance choices, atmospheric depth, water/vegetation/ruins as one composition | Forced fixed camera, constant picturesque obstruction, empty travel masked by fog |
| Industrial spacecraft/city/hangar references | Mass organization, service access, purposeful gaps, cargo routes, human-scale interfaces, engineered support | Photorealistic texture noise, generic military grey, exposed pipes everywhere, inaccessible spectacle |
| Relic-world and layered-civilization concepts | Planetary archaeological depth, recognizable former functions, different societies interpreting inheritance | Every mountain is machinery, every ancient civilization perfect, everything abandoned and hostile |
| Earthsea-like development | Local knowledge, work, restraint, relationships, ordinary competence and moral complexity | Literal copied characters/plot, universal password names, a world serving only the protagonist |
| Technical/magical sandbox influences | Systems explain one another and enable invention | Separate incompatible visual packs or recipes copied without coherent use |

The target is **magi-2000**: deliberate sprite/pixel-material craftsmanship translated into a modern interactive 3D voxel world. Authored colour clusters and restrained material detail coexist with proper spatial shadows, atmosphere, motion, and lighting. Do not use global screen pixelation, random per-texel speckle, noisy Bayer coverage everywhere, or a generic low-poly kit as the style itself.

## 7.2 Form hierarchy

Design landscape mass → site silhouette/voids → usable architecture and movement → meaningful machinery → contact detail → effects. Review neutral clay and grayscale thumbnails before surface decoration. Distinguishable mountain profiles and grouped tree canopies matter more than filling every surface with detail.

At long range: landform, roof/canopy masses, major openings, towers, water, and weather. At walking range: routes, doors, buttresses, service areas, work and habitation. At hand range: fitted repairs, tools, material wear, seals, calibration, inscriptions.

Negative space is designed: an undercroft, opening between engine pods, suspended bridge, broad water surface, or clear workshop volume. Quiet fields and unadorned walls can be finished compositions. Every view need not contain another boss arena.

Validate ordinary player cameras from multiple approaches, back sides, roofs, interiors, first person, flight, and unexpected excavations. Never build one beautiful viewing angle surrounded by unfinished geometry.

## 7.3 Material and architectural families

**Founder:** broad pale ceramic/composite, exceptionally precise dark glass and ribs, deep recesses, large usable spans, rare active markings. Monumental civic purpose visible through connections.

**Chorister:** tuned intervals, paired chambers, fitted fins, calibrated instruments, suspended or bounded field-supported parts. Magical alterations fit existing structures rather than random crystals on every surface.

**Engineer:** understandable metal framing, exposed joints only where useful, rail/pipe/shaft connections, overhead handling, removable covers, real access and repair clearances.

**Present:** skilled regional timber, plaster, clay, cloth, stone, gardens, clean new industrial modules, and reclaimed fittings. New construction may look new. The player can build something attractive, warm, orderly, and competent.

Share useful proportions, interfaces, and material families across eras while making their intersections legible. Not every site needs all traditions. Distinct climates/cultures shape roofs, windows, drains, circulation, clothing, and plants.

## 7.4 Palette and light

Give each region a limited base palette, subordinate variants, and selective accents. Woodland jade/cream/warm roofs, snow blue-green/ivory/amber refuge, dry basins ochre/violet shade, Tideglass deep blue/sea glass, and Crimson oxblood/soot/bone/brass are proposed studies, not one universal palette lookup.

Painted cyan is not emission. Warm metal is not automatically hot. Magic, temperature, operational signals, cosmetic dyes, and allegiance cannot rely on colour alone. Lights and VFX correspond to actual state and remain readable under reduced-effects settings.

Surface value clustering does not require all lighting to snap between a few flat bands. Preserve stable contact/directional shadows, legible interiors, meaningful indirect/ambient contribution, and convincing night visibility. Review neutral daylight, sunset, overcast, artificial light, and torch/cave conditions. Do not fix a black-shadow bug by adding flat ambient everywhere or burying it in fog.

## 7.5 Vegetation and ecological detail

Use recognisable morphology: branching trees, arching fern rachis and pinnae, sedges/reeds, shrubs, flowering plants, moss, lichen, fungi, and vines. Compose habitat patches, age/species groups, clearings, wet boulders, leaf litter, and quiet ground. Density alone does not make a forest good.

One plant identity persists through near organs, middle-distance grouped foliage, and far canopy representation. Cutting, grafting, burning, harvesting, and growth survive LOD/save changes. Use bounded physical interaction where it matters and weighted GPU wind for presentation; no independent rigid body for every leaf.

Vegetation must not conceal all routes or carpet every machine. Growth follows moisture, light, shelter, substrate, season, and local history. Snow, wetness, sediment, and wear reveal construction rather than adding identical decoration everywhere.

## 7.6 Original equipment and animation

Deliver an in-engine proof gallery of six distinct well-designed tools/wands before expanding a huge catalog. Mature lineages may include twig/witchwood, forked conductor, rune key, bone needle, alchemical quill, cinder rod, shard cluster, grave artifact, filament, blackglass prism, and domestic tools. Avoid the repeated giant copper/cyan lantern silhouette.

A held item has a deliberate grip, coherent mass, readable casting end, useful size in the actual FOV, proper hand/finger placement, and meaningful idle/move/cast/recoil/sustain/failure motion. Gameplay muzzle and targeting agree with animation. First-person, dropped, other-player, and icon versions depict the same object without requiring identical geometry.

Creatures and characters have readable silhouette/gesture/functional anatomy, not randomized body-part piles. A handler and bound creature can visually communicate a real tactical connection. NPC species and occupations are not the same variable.

## 7.7 Actual asset pipeline

For authored hero props, mechanisms, and creatures where Blender is appropriate, actually execute the available Blender pipeline. Retain editable `.blend` or reproducible scripts, pixel/UV source, palette, scale, named parts/pivots/ports, collision representation, LODs, supported export, importer validation, and native captures. Inspect three distances and actual first-person use.

Procedural boxes/cylinders are acceptable blockouts, not finished assets. A rendered offline beauty image does not prove native import, correct interaction, pressure, or performance. When a tool is unavailable, identify it honestly and complete an executable source pipeline or a bounded placeholder without claiming the final art exists.

User-supplied reference images are inspiration/provenance, not automatically licensed shipped assets. Do not copy logos, named characters, promotional text, meshes, or paintings into the game. Maintain an original production-asset license/source manifest.

## 7.8 Player parity

Expose the same attractive construction vocabulary to players: roof profiles, frames, thin supports, canopy corners, pressure penetrations, hull transitions, wing roots, doors, stairs, lamps, pipes, service cabinets, cargo openings, and multiblock interfaces. Corners and junctions are first-class assets.

A small coherent compatible kit is better than hundreds of unrelated pieces. Provide symmetry, snapping, rotations, useful substitutions, and clearance previews without secretly replacing the design with a valid prefab. Generated sites may use composed combinations, not inaccessible exclusive beauty parts.

---

# 8. Renderer, LOD continuity, streaming, and loading

## 8.1 Preserve the working architecture and diagnose first

Audit packed geometry, draw metadata, material indexing, indirect counts, pool allocation/lifetime, render/compute shader boundaries, uniform layouts, depth conventions, shadow passes, and adapter features. Previously described sparse-slot/dense-count or pool exhaustion problems are hypotheses until reproduced.

Keep a correct native baseline while optimizing. Compare compact indexed geometry, packed quads/vertex pulling, culling, and targeted sparse traversal only on representative scenes. Do not replace world storage/netcode/physics to fit a fashionable mesher or promise ray tracing as a universal solution. Keep CPU reference/fallback paths where they serve validation/capability coverage without casually regressing the established production GPU path.

## 8.2 Multiresolution coverage is a publication contract

At coarse/fine boundaries, account for actual fine-cell face coverage, dyadic/integer addresses, edges/corners, and shape/material rules. A small tunnel cannot become a sealed coarse face; a fine railing cannot vanish as cover because of visual distance.

Use a coverage-preserving hierarchy frontier. A parent is replaced only when required children, explicit empty results, and seam/transition data are ready. Controlled overlaps have explicit masks, short duration, and reserved memory. Keep a valid parent when a visual refinement budget is exhausted; never truncate draws into holes.

Edited volumetric exceptions need exact or correctly conservative representations appropriate to their task. Unknown data cannot become arbitrary filled land or transparent sky. Coarse summaries must reflect persistent destruction. Do not resurrect intact distant towers when memory pressure coarsens the scene.

Remove depth-sink/radial-discard crack-hiding only after actual seam coverage is correct. Use geometric stitching/morphing only for compatible representations and proven shared samples; not a universal label covering incompatible heightfields and volume topology.

## 8.3 Shadows, visibility, and materials

Diagnose near/distant/combined shadow paths separately. Check depth convention, sampler comparison, cascade mapping, bias, normals, shadow coverage, and caster bounds. Foliage, arches, hulls, moving fragments, and distant structures must cast appropriate shadows without phantom black fields.

Conservative occlusion must not hide a newly exposed opponent. Invalidate previous-depth history after edits/teleports/moving occluders, and prefer visibility when uncertain. Match HZB min/max reduction to actual depth convention and projected query, including non-power-of-two resolutions and near-plane cases. Compare to an unculled reference.

Separate opaque, alpha-tested, transparent, water, atmosphere, and portal work. Texture sampling/atlases need stable detail, correct borders/mips, consistent scale, and motion review. Temporal history has object/view/revision validity; it cannot smear removed walls back into existence. Cap lights/shadow allocations/transparency work with explicit correct overflow behavior.

Keep outline/contact treatments selective. Do not outline every microvoxel or turn detailed machinery into a noisy black grid. Important silhouettes and room lighting must remain legible with bloom and optional particles off.

## 8.4 Fast safe loading

Measure menu response, new-world macro generation, warm saved-world entry, cold resume, first join, cached join, reconnect, and uncached region arrival separately. First playable means correct local facts, current edits, complete relevant domain, collision, necessary assets, minimal truthful visuals, and responsive input—not a loading camera above absent ground.

Persist canonical macro generation/history once. Use indexed regional snapshots and bounded journal tails. Cook source assets, mips, collision helpers, and known shader variants before runtime where possible. Use independently readable compressed pages rather than thousands of tiny synchronous files or one archive inflated in full.

Key caches by world/generator/content/math versions, address/level, relevant source projections, product version, and device/backend identity where necessary. Corrupt/mismatched derived data is refetched/rebuilt without resetting a save.

Stream by time to interaction, not only distance: falling/fast-moving actors, projectiles, vehicles/passengers, portal exits, edited cover, then optional detail. Include worst supported acceleration and full request-to-collision installation latency. Declare a supported travel envelope; unlimited instantaneous flight through uncached geometry is not a valid requirement.

Arrival transitions reserve and prepare destination state before activation. Readiness identifies installed epoch/baseline/command frontier and permits bounded ongoing changes; requiring an unchanged busy destination forever would deadlock arrival. No global pause, loading invulnerability, invisible invented floor, or fall through missing ground.

## 8.5 Main-thread and GPU discipline

No synchronous full-world reads, giant rebuild/serialization, blocking worker joins, whole-device waits, unbounded callbacks, or bulk destruction on the interaction path. Bound installation cost as carefully as worker cost. Large GPU dispatches and uploads also consume time; asynchronous naming does not make them free.

Keep GPU-produced draw counts on the GPU where the supported path permits; validate capabilities and bounded fallbacks. Shared pools use generations and safe last-use retirement. Avoid copying the whole scene table after one edit. Refine only geometry likely to survive long enough to repay its cost.

Measure full pipeline CPU/GPU timing, allocations, copied bytes, upload age, wasted refinement, native memory commitment, and state age. Faster meshing with slower rendering or larger upload stalls is not a success.

---

# 9. Coupled 3D materials, structural building, and destruction

## 9.1 One material ontology, several appropriate solvers

Use typed composition/phase/quantity/temperature and relevant physical traits across terrain, objects, fluids, processes, biology, and magic. Different representations have explicit transitions; they are not duplicate resources.

| Material/system | Canonical representation and behavior |
|---|---|
| Terrain and coherent structures | Sparse volume plus meaningful bonds/attachments; persistent damage; compound rigid aggregates on supported separation |
| Loose grains/rubble | Bounded consequential parcels/aggregates with quantity, friction, motion, and settling; no body per grain as default |
| Stored/bulk liquid | Conserved reservoirs/pipe graph with levels, composition, enthalpy, and matched flux |
| Active free liquid | One selected bounded 3D solver where free flow, pressure, buoyancy, and moving interaction matter |
| Gas/heat/reactions | Compartments/network state plus local finite-volume fields when gradients affect play |
| Ropes/hinges/folding/deformable objects | Bounded constraints/cages with force, compliance, tearing, motor and collision rules |
| Cosmetic chips/spray/smoke | Presentation only; cannot also become collectible mass, damage, or hidden physical cover |

Keep the existing appropriate rigid backend. Evaluate specialized reuse or an original solver behind a narrow interface; no automatic second solver ecosystem. A CPU-authoritative coupled pilot is the first proof; optional GPU solvers must earn compatibility, coupling, recovery, and total-frame benefits.

## 9.2 Conservation and conversions

Each material amount has one owner. Terrain-to-fragment, reservoir-to-flow, liquid-to-ice, cargo-to-machine, salvage-to-inventory, and magic conversion transfer quantity/composition/momentum/thermal state with declared rounding and tolerances. No duplicate solid still left in the terrain after creating its debris.

Record external fictional sources/sinks explicitly. Numerical drift, deliberate dissipation, heat transfer, and magical work are different categories. Freezing accounts for latent heat and collision; melting reverses ownership. Settling/reseeding/LOD cannot mint water or destroy recoverable material. Merging moving aggregates must account for angular momentum and lost relative kinetic energy under the chosen model, not blindly average velocities.

## 9.3 Structures and constraints

Connectivity locates components; material strength/load paths determine failure in the supported structural model. Use local components and hierarchical interbrick bonds/anchors where useful. Arbitrary deletion is not solved by merge-only union-find. Preserve resumable frontiers and exact dependency versions; unknown unloaded support cannot be classified as gone.

Reserve detachment output separately from connectivity search: component enumeration, mass/inertia, colliders, module/cargo ownership, compartments, and contact growth. Meaningful slabs, wings, machinery racks, and bridge sections detach as compounds. Tiny residue may become conserved salvage/deposit records; important moving cover cannot become purely cosmetic under low settings.

Splits use the parent's accepted point velocities and angular motion. Changing center of mass must not create an impulse. Collision/topology/module/cargo/pressure changes publish coherently. Support failure, crushing, bending, rope tension, jamming, and motor limits have visible feedback and counterplay.

## 9.4 Liquid and gas proof

Build a heated-tank breach fixture with a floating crate, a powered hinge, a pipe supply, and active/inactive boundary. Choose one bounded real 3D liquid path after comparing complete coupling and minimum-hardware costs. APIC/PIC/FLIP or particle pressure methods are candidates, not a requirement to implement all of them. A transport scheme alone is not a complete pressure solver.

Bound neighbors, particles/cells, iterations, scratch, checkpoints, and reconstruction. Never truncate required neighbors silently or change one peer's solver iterations by FPS. Define compatible substeps/stability limits and residual tests.

Each boundary declares sealed/open/inlet/outlet/coupled behavior. Process shared flux once with equal transfers. An unloaded neighbor is not an infinite drain or a wall. Coupled pressure may need a merged solve or validated boundary formulation. Coarsening is allowed only while its reduced model remains valid—not when a camera turns away from an active jet.

Fluid-solid forces have one owner and equal/opposite impulse with consistent moment arms. Do not apply buoyancy twice if pressure coupling already supplies it. Moving boundaries use the same pose/substep as rigid contacts. Track composition, heat, positivity, pressure, and phase changes across all levels.

Gas/steam ventilation and room pressure use a declared bulk relation and conservative transport. Combustion consumes represented fuel and oxidizer, generates products/heat, and affects pressure. A low-speed flow solver is not a shock solver; blasts need their own bounded physically motivated model rather than smoke presented as pressure physics.

## 9.5 Readable systemic examples

A magical jackhammer chips real material, applies supported impulses, and routes its one yield to debris, inventory, or a declared conversion—not all three. A folding spell lifts existing terrain panels using hinges/motors, leaving donor voids and respecting occupants. A hot-vessel breach changes flow area, moves fluid and heat, pushes a crate, and loads a door. A frozen crossing incorporates actual water, a cable, load, and later melting.

Demonstrate these on the same shared material system used by factories, spacecraft, terrain, and combat. Four unrelated scripted visual effects do not satisfy coupled physics.

---

# 10. Construction, inventory, crafting, and early convenience

## 10.1 Build and inspect

Support placement/breaking across existing voxel scales, preview rotation/shape/material, snapping and free placement where supported, mirrored building, blueprints, accessible repair, and consistent primary/secondary actions. Fine construction is not a license to refine an entire planet densely.

A ghost explains fit, occupied volume, support, seal, resources, access, and permissions. A failed operation states the actual missing condition. Creative exposes searchable functional families, including **Spacecraft & Space Building**, magic, industry, ecology, mechanisms, and ordinary architecture. Creative controls explicitly label infinite supplies/flight/orbit placement; tests using them do not prove survival progression.

Blueprints store components, geometry, settings, named interfaces, minimum clearances, and permitted substitutions. Survival assembly consumes real inventory atomically/staged as declared. Undo cannot refund committed material now owned by another player or recreate transferred cargo.

## 10.2 Unified inventory and useful storage

Implement one canonical item/lot stack model with clear compatibility keys, quality bands, volume/mass where relevant, ownership, and stable container revisions. Avoid unique floating properties per pebble. Stack caps are tunable item-family data; equipment need not stack like tiny components. Never silently change existing save quantities.

Provide quick-stack into nearby accessible containers, inventory sort/consolidate, search/filter, favourites/locked slots, sensible bulk transfer, material drawers, fluid storage, and overflow diagnostics early. Show locked/inaccessible/full destinations clearly. Sorting changes organization, not identity/quantity.

Add a configurable **local storage scanner/terminal** inspired by connected workshop access: discover permitted nearby containers through actual connectivity/range rules, maintain an incremental index, and withdraw/deposit through atomic authority transactions. It does not grant unlimited storage, bypass ownership, extract from sealed private ships, or magically connect every planet. A starter range such as 16 m is a balancing proposal, not a hard architectural constant.

Index insert/remove/rename/permission/container-move events; resync bounded snapshots after missed events. Simultaneous last-item requests have one winner. Moving a chest into a vehicle or across a portal preserves its contents and updates eligibility once. Nested-container capacity/weight rules cannot recurse indefinitely.

## 10.3 Crafting and process ownership

Recipes reference capabilities, typed ingredients and substitutions, meaningful conditions, energy/work, outputs/byproducts, and failure/recovery behavior. Aggregate duplicate ingredient lines before validation. Check overflow and partial capacities. Reserve outputs before irreversible stages; power loss pauses or creates defined intermediate material rather than deleting input.

Crafting from a storage terminal uses the same transaction model as manual crafting. No separate optimistic client crafting ledger. Tool repairs, salvage and recycling preserve declared fractions/inputs rather than arbitrary output multipliers.

---

# 11. Science, machinery, energy, and player-designed electronics

## 11.1 Products before recipe complexity

Each new process enables something desirable: lighting, a pump, useful material, better tool, radio, controller, vehicle bearing, greenhouse, wand component, repair part, or expedition capability. Demonstrate the product and a competent preset before requiring elaborate infrastructure.

Allow alternative routes: mechanical, electrical, recovered machinery, discrete electronics, cultivation, or magitech. Do not require every player to master every industrial branch. Factories make scale/convenience/control rewarding; they do not monopolize basic shelter or storage.

The functional catalog eventually covers mining/recovery, crushing/separation, metallurgy/alloys/heat treatment, glass/ceramics/polymers, chemical operations, pumps/gases/vacuum, energy conversion/storage, mechanical automation, circuits/controllers/chips, agriculture/forestry/biotech, logistics/rail, and high-end reactors. Create original machines and coherent roles rather than copying mod item names.

## 11.2 Materials and processing

Lots carry practical grade/composition/contamination/provenance classes. Crushing and refining improve recovery, purity, throughput, or byproduct capture; they do not multiply an element's mass without an explicit additional input. Tailings support fill, reprocessing, sale, storage, or automated disposal; no compulsory manual trash chore.

Use reusable process capabilities such as HeatTreat, Separate, GrowCrystal, Deposit, Pattern, Etch, Dope, Inspect, and Package. Time and tolerance are intentionally game-scaled. Teach with diagrams and conditions, not real-world hazardous laboratory instructions or claims of faithful fab elapsed time.

Machines expose physical typed ports, modes, gauges, heat, pressure, real moving parts where useful, access, output capacity, and a clear bottleneck report. Factory statistics show input/output rates, utilization, energy per product, loss, quality, and blocked time.

## 11.3 Electrical and mechanical systems

Preserve real connections and units: stored energy versus power, voltage/current, finite resistance, source/current limits, charging limits, heat, and network topology. Closed DC circuits require actual source/return paths or an explicit declared reference model. Do not replace a functioning circuit model with a universal wireless power pool.

Use a bounded suitable nodal/MNA/RC model for supported circuits, with cached topology/factorization only when coefficients permit. Deterministic pivots, iteration bounds, singular-circuit handling, saturation, and numerical diagnostics matter. Cable loss scales with the implemented current/resistance model; heat is accounted. Capacitance retains charge/voltage behavior rather than pretending to be a constant-voltage battery.

DC/DC converters, rectifiers, inverters, and transformers have distinct roles. The agreed AC abstraction uses RMS/real-power behavior with phase ignored; do not call it complete waveform/reactive-power simulation. Machinery can also use shafts, belts, steam, and pressure under explicit conversion/loss rules.

Audit before changing balance reported or proposed in earlier context: induction storage shells around 3–9 m, 64 kWh cell modules, 256 kW provider modules, configured input/output ports, a roughly 5 kW basic biomass burner, and useful machine footprints such as 1×2×2 m. These are retained tuning leads, not permission to overwrite current values. Capacity, provider count, conductor/port limits, and occupied module space jointly determine actual performance; a large empty shell has no free capacity.

Thermal, capacitor, battery, flywheel, fuel-cell, pneumatic, and other stores serve different jobs. Closed conversion loops lose or explicitly receive energy. Cooling needs a sink; refrigeration expends work; vacuum is not an infinite cooling source. Renewable/local power remains useful after advanced energy appears. Fission/fusion and exotic generation are bounded fictional game systems with readable startup, heat, fuel, safety, and net-output concerns—not engineering instructions for real equipment.

## 11.4 Logistics and unattended factories

Compile stable belt/pipe/rail/electrical networks and update affected topology, not every block every frame. Represent belt flows/stack segments compactly until physical interaction requires objects. Preserve order, capacity, blockage, picking, transfer ownership, and broken-link behavior.

Sleeping networks subscribe to real changes. Canonically merge wake requests, guard against reused IDs, cancel stale scheduled events, and bound retained dead subscriptions. Validate coarse or closed-form production against a tick-stepped oracle across interruptions and thresholds. Do not award products through a destroyed belt or run a blocked factory while unloaded.

Byproducts, buffers, interlocks, hysteresis, setpoints, and alternative routing support player ingenuity. Simple presets should work. Advanced controllers improve an actual process, not an unexplained global efficiency aura.

## 11.5 Integrated-circuit and controller design

Separate mechanical fabrication, PCB assembly, IC design, fabrication process, and packaging. The player creates **functional multilayer macro-module layouts**: inputs/outputs, computation, registers/memory, routing, clocks/timers, sensors, power distribution, and interfaces. Provide known-good starter designs, test benches, diagrams, and saveable versioned patterns.

Compile the design into a bounded executable controller/macrodevice. Connectivity, routing length/congestion, latency class, parallelism, memory, IO, power/heat, and manufacturability influence actual workload capability. A sensor-heavy controller, memory-heavy scheduler, and parallel processor solve different problems. A filled grid is not an automatic speed bonus.

Smaller process nodes allow denser usable modules in a fixed die abstraction, with appropriate fabrication, yield, routing and power tradeoffs. Do not scale every attribute upward without cost or equate a modern commercial node name with a literal universal cell size.

Process path can include silicon purification/crystal/wafer preparation, clean handling, resist/pattern exposure, etch/deposition/doping, repeated interconnect layers, inspection/test/binning, and packaging. Historical discrete/transistor/controller routes remain useful. Compress waiting and abstract timing; no full transistor solver for every chip, no nanosecond clicking, no real-time fab queues.

A chip's benchmark must change real machine/controller behavior under fixed resources. Versioned manufactured lots preserve their design and process results. Reloading cannot reroll yield. Copies require materials and process capability. Basic avionics and early useful controllers do not require the final fabrication node.

# 12. Magic: expressive, physical, powerful, and understandable

## 12.1 One interacting world, not a second spell-only reality

Magic acts on the same material, thermal, biological, electrical, structural, motion, sensory, and spatial systems as tools and machines. The setting may call its deeper manipulable order **the Work**. This is a thematic/implementation continuity term, not proof that all reality or every mystery is one programmable machine.

A shared bounded command IR may support spells, controllers, and ritual instructions where useful. It does not replace every specialized physical solver, turn plant biology into bytecode, or require the player to program before casting a basic spell. Different traditions expose distinct grammars, costs, access, presentation, and social meanings.

Provide immediately useful noncombat magic—light, drying/heating, shaping, limited freezing, repair, movement, growth, sensing—alongside projectiles, traps, fields, transformations, and ritual apparatus. Effects should be remarkable through interaction, not just larger bloom.

## 12.2 Wand/deck model

Preserve useful concepts: order or explicit shuffle, capacity, draw count, cast delay, recovery/recharge, resource pool and recovery, spread, multicast, trigger/timer payloads, damage/type, homing/bounce/orbit, material transformation, growth, status, and environmental modifiers. Keep clear tooltips and a cast preview/test space. A powerful build can be understandable without explaining an entire compiler.

Compile and validate bounded effect graphs: maximum simultaneous projectiles, child expansion, recursion depth, affected volume, duration, recurrence, resource sources, and worst supported topology/physics work. Detect cycles and provide interpretable errors. Static bounds and runtime authority limits work together. Large effects can deliberately charge/propagate in stages; no hidden frame-dependent weakening.

Costs, outputs, hit events, and resource recovery are once-owned. Cosmetic replay cannot recharge mana, award experience, spawn a second projectile, or duplicate loot. Different forms of shields have physical/semantic differences—kinetic impulse distribution, electrical conduction paths, biological repair, ward boundaries—not five colors of the same opaque bubble.

## 12.3 Required expressive examples

**Earthfold:** select a bounded ground area, create articulated panels from actual donor material, leave matching recesses, and move them by constrained supported force. Begin with a simple box-like cage and one payload such as Kiln Seed or Frost Stitch. Define closure from real geometry/state, not a hidden timer. It can trap, lift, brace, block, fail under load, or jam; it cannot teleport panels through occupants. Later shape grammar is an expansion, not a prerequisite.

**Hammerhand:** sustained magical jackhammer with visible cadence, material-sensitive impacts, recoil and supported impulse, incremental excavation, and configurable exclusive yield destination.

**Spindle Lance:** a narrow penetrating/drilling spell whose progression depends on material, budget, angle, and available work; not a visual beam deleting unlimited rock behind any wall.

**Lens Knot:** a bounded field that redirects/focuses supported projectiles or measurements through clear geometry and cost; distinguish visual refraction from actual collision/path changes.

**Quarry Toll:** an explicitly funded extraction/conversion arrangement for bulk work. Ejection, direct storage, transmutation, experience conversion, and discard are mutually accounted routes with real losses and permission. Modifier-added heads perform paid additional work rather than multiplying the same output.

These names are design candidates retained from prior context. Preserve better-established compatible spell identities in the repository and map the behavior explicitly.

## 12.4 Living magic and cultivation

Players must be able eventually to cultivate or summon actual supported creatures, fungi, and organisms—not merely place a mushroom-looking terrain voxel or heal a nearby enemy. A summoned/cultivated being has an identity, behavior, owner/relationship where appropriate, lifecycle, ecological requirements, and bounded population costs.

Growth/size augmentation changes the relevant anatomy, mass/energy source or explicit magical compensation, movement, colliders, reach, and habitat requirements. It cannot only scale the render mesh. Define size limits, fit checks, attachment changes, and safe failure. A small usable first creature is better than an uncontrolled procedural creature factory.

Allow ethical food/biomass/reagent routes, breeding/grafting/cultivation, symbiotic automation, familiar relationships, and ecological observation. Hostile farming/capture may be possible through learned supported mechanics; it should not be a compulsory resource exploit for ordinary progression.

## 12.5 Equipment, perks, enchantment, and character development

Support modular tool construction with meaningful heads/shafts/grips/bindings, material roles, repairability, weight, efficiency and special actions. Enchantments and perks alter supported mechanics, resource/control tradeoffs or available verbs rather than adding unbounded hidden multipliers. Keep equipment identity through reforging and salvage.

Use a readable status/condition vocabulary for injury, temperature, wetness, electrical exposure, poison or curse, protection, resistance and recovery. Distinguish physical mitigation, active defense, immunity to a narrowly defined effect, and a ward that can be disrupted. Avoid a combinatorial trigger burden that makes ordinary combat unreadable. Explain stacking and caps explicitly.

JRPG/tabletop and action-adventure influences should create interesting builds, timing, preparation, exploration choices and cooperative combinations, not copy a full ruleset or impose rigid classes. Character development unlocks options and mastery; engineering, clever observation and good equipment can still overcome a numerical disadvantage. Preserve a useful starter kit and experimental freedom.

## 12.6 Naming and restricted knowledge

Names express precise understanding and relationships, not omnipotent passwords. A powerful binding or targeted repair also needs a valid link/proximity, technique, energy, resistance handling, and applicable realm rules. Knowledge can improve efficiency/control and reveal options without making every target obey remotely.

Restrictions have causes: safety, coercion, consent, unstable consequences, institutional monopoly, or political control. Forbidden is not always evil; licensed is not always humane. Players can learn through training, experiments, trade, research, salvage, or illicit access with actual consequences.

## 12.7 Dark magic families

| Technique | Real capability | Consequence and counterplay |
|---|---|---|
| Borrowed Breath | Transfers supported living reserves into casting work | Consent/target resistance, link interruption, fatigue/injury limits, no infinite healing-energy loop |
| Dead Hand | Binds an eligible surviving identity to a construct or ward | A willing guardian differs from enslavement; find vessel/anchor and release or negotiate |
| Unnaming | Disrupts a specific magical identity/binding | Can free or harm; precise target knowledge, bounded effect, counter-wards and recovery |
| Redirection | Changes a supported linked transfer's destination | Logistics/rescue or theft; verify permissions/physical link, conservation, readable routing |
| Hollow Harvest | Draws biological reserves into immediate production | Depletes future fertility/recovery; measurable ecology, alternatives and repair |
| Casting Off | Severs a caster from a binding consequence | A severed responsible identity may become an Unreturned; not merely an evil-score increment |

Anchor, vessel, tether, participant, or affected process is discoverable and interactable when presented as counterplay. Dark mages need preparation, supplies, assistants, positions, and dependencies. They cannot ignore the player's relevant rules whenever it would defeat them.

## 12.8 The Unreturned

Implement an original personal adversary inspired by the theme of confronting consequences, not a copy of Ged or his story. Casting Off may be used for understandable reasons; its risk follows what the operation does, not a simplistic moral meter.

Its knowledge is bounded to observations and accessible traces. It may learn a few cast patterns, paths, trap arrangements, or habits it actually encountered. Never read the entire player's inventory and instantly choose an unbeatable counter. Adaptations have visible evidence, limits, cost, and weaknesses.

Progress from uncertain sightings/evidence to a constrained pursuer and an investigable origin. Repeated defeats provide substantial real respite, remove an anchor, or reveal useful information; not immediate fake victory respawns. Resolution can include reintegration, negotiated independence, binding, or ending its sustaining mechanism where the story/system supports it.

Maintain dependable wards/refuges, opt-in or explicitly shared co-op involvement, persistent cooldown/anchor state, and offline safety. No impersonating real player chat, deleting storage off-screen, punishing uninvolved friends, or making every home unsafe. The horror is a familiar gesture or trace in an impossible-but-accountable place, not constant black fog and invisible attacks.

---

# 13. Ecology, inhabitants, institutions, and a useful economy

## 13.1 Friendly and mythic populations

Build recognizable ecological and social roles: livestock/pack animals, birds, fish, pollinators, amphibians, small scavengers, fungi, predators, neutral territorial creatures, friendly mythic inhabitants, custodial machines, traders, scholars, and specialists. Preserve already implemented satyrs where verified. Candidates such as centaurs, dryads, kobold-like small folk, aquatic peoples, gargoyle custodians, or ancient artificial persons need original designs and real habitat/access roles rather than a skin roster.

Species/body plan, culture, job, faction, morality, and technology are separate. Mixed settlements can share institutions with adapted furniture/routes. Mythology can remain ambiguous, local, and lived rather than every unusual being existing to drop ingredients.

Functional anatomy matters for AI/combat/cultivation: sensors, locomotion, feeding, armor, power, attachments, and vulnerabilities. A creature's size and form affect navigation. Use authored families and meaningful variation, not arbitrary mesh assemblages.

## 13.2 Life across simulation scales

Near actors use appropriate sensing/navigation/animation and simple work/rest/social routines. Regional records preserve journeys, needs, relationships, cargo, and consequences without full per-frame brains. Materialization refines the same record once and suspends overlapping abstract work. Dematerialization transfers result state back once.

Ordinary settlements maintain themselves with available supplies and reasonable defaults. Shortages produce warnings and gradual consequences, not instant extinction because one ingredient disappears. Buffers, substitutions, alternate routes, and migration support resilience.

Navigation reads actual geometry and changed routes. It does not let enemies shoot through rebuilt cover, residents walk through missing bridges, or caravans deliver twice. Local fixes invalidate affected routes rather than every path on the planet.

## 13.3 Social and economic play

Households, facilities, productive capacity, inventory, agreements, claims, and transport routes have persistent identities. Trade prices and availability reflect useful bounded conditions; no requirement to model every economic variable. Cargo is real custody, not a second abstract stock pile that spawns goods when convenient.

Recruitment can follow hospitality, employment, rescue, shared work, migration, or persuasion—not a workstation spawning a generic person. Player inns, gardens, warehouses, workshops, bridges, and ports affect actual access/capacity/services. A remote cabin or animal sanctuary remains valid; governing a city is optional.

Avoid punitive upkeep spirals and inevitable raid escalation for prosperity. Threats have origins and available responses: negotiation, avoidance, defense, relocation, containment, or conflict. Reputation/claims should not make every minor mistake permanently destroy a playthrough.

## 13.4 Wizard compounds as institutions

**College of Reed and Copper:** coastal/waterside civic magic, weatherwork, repair, precision instruments, gardens, lodging, apprentices, waterworks. Restricted naming/large-area work has real containment and institutional access rules. Service/trade/research and infiltration are distinct approaches.

**Black Kiln:** legitimate high-quality fabrication concealing coerced memory-bound labor. Owners, technicians, scholars, clients, and wardens have dependencies. Players can expose, sabotage, replace its method, negotiate, rescue, or seize infrastructure; consequences affect actual production and people.

**House Beneath the Rain:** a protective/weather institution whose burden-transfer technique may harm communities excluded from its priorities. Investigate evidence and outcomes rather than unlock a stereotypically evil purple basement.

Provide kitchens, public work, private accommodation, records, maintenance, control rooms, and restricted spaces. An alarm changes guard/door/work behavior, not instant mission failure. Restricted rooms have physical access/containment; permission is social, capability mechanical. Independent research can reproduce suitable techniques.

---

# 14. Exploration, navigation, dungeons, enemies, and bosses

## 14.1 Compose journeys

A major approach has foreground shelter, a middle-distance choice, and a distant question. Use silhouette, water, wind, route structure, sound, habitation, and occlusion to guide curiosity. Alternate approaches reveal different evidence. Do not require the intended camera or entrance for the world to make sense.

The player notices, hypothesizes, chooses an approach, observes, acts or simply understands, and returns differently informed. A region can also reward beauty, friendship, scientific knowledge, a safe camp, or a useful building location without a combat/repair task.

## 14.2 Atlas and Primer

Distinguish Observed, Reported, Inferred, and Tested knowledge with source/time/confidence. Let players pin routes, rename camps, store diagrams/sketches/captures where supported, and compare old maps with current terrain. A consistent repeated structure may imply a missing segment; label it a hypothesis, not guaranteed loot.

Instruments answer different questions with range, resolution, conditions, and uncertainty. Geological probe, electrical meter, thermal viewer, pressure sensor, spectrometer, radio, radar/acoustic sensor, and magical perception are not one omniscient scan button. Automation may perform legitimate observation when equipped; no arbitrary human-only discovery flag.

Offer optional unassisted, subtle, and structured help. Important acoustic/colour clues have alternate accessible signals. Maps/radio report actual events and schedules known to their sources, not unrestricted global secrets.

## 14.3 Traversal and earned convenience

Walking, climbing/building, ropes, boats/carts, pack creatures, early gliding, elastic tools, vehicles, aircraft, and spacecraft each change what is practical. The early glider has bounded lift/energy and environmental constraints; elastic boots/sling have accounted charge or stored elastic work, not infinite bounce.

Let powerful tools solve old inconveniences. A repaired lift, automated freight route, drill, survey drone, or landing site may bypass the original path. Sustain later exploration through hidden interiors, relationships, mechanisms, spatial conditions, and larger questions, not arbitrary no-fly fields over every ruin.

Some places genuinely restrict access through heat, pressure, dangerous material, local wards, faction control, or environmental conditions. Their source, scope, counters, and costs are readable. A field emitter can be damaged, bypassed, supplied, negotiated around, or studied where its defined behavior permits. Not every challenge must permit every solution, but important obstacles should support distinct supported approaches.

## 14.4 Dungeons and systemic puzzles

Generate room/route graphs before decorative stamping. Vary ceilings, light, shafts, galleries, vistas, bridges, pits, hidden passages, chambers, and loops. Include cisterns, lush caverns, spider habitats, tombs, undead caverns, dripping service galleries, astral rooms, observatories, and industrial ruins with original identity.

Use Zork/older-D&D-inspired physical observation, Zelda-like readable spatial changes, and immersive-sim alternatives without copying exact rooms. A false wall is actual material; a phantom floor has explicit illusion/collision rules; a pressure puzzle moves conserved fluid; a gravitational room affects actors and objects, not just the camera.

For each important obstacle document physical state/owner, at least two legitimate methods, observable cost/hazard, evidence, and what digging/building/flight changes. Local antimagic fields can constrain spell use but not serve as a blanket excuse for indestructible dungeon walls. Solve in-world problems, not three glowing-key sockets repeated forever.

Major secrets use several independent clue channels so ordinary destruction does not irrecoverably erase all understanding. Never require every player to read a long log to see obvious machinery. Failure can produce informative experimental results.

## 14.5 Enemy and boss quality

Enemies have roles, readable preparation/recovery, movement, target selection, animation, material interaction, original sound, and functional vulnerabilities. Avoid simply spawning larger variants with more HP. Include ground melee, ranged/throwing, fliers, support/control, ambush, and environmental specialists as the existing combat vocabulary allows.

A boss must be interesting alone before its farming loop exists. Design an arena/habitat whose routes, cover, verticality, hazards, and resources support several builds. Damage changes surroundings and tactics. Attacks track within declared limits rather than reading player input; phase transitions do not erase arbitrary player construction or teleport trapped bosses through walls silently.

Physical strategies, traps, counterspells, engineering, captures/diversion, and nonlethal options can work where designed. Provide visible robustness/costs rather than sudden immunity when a player is clever. Solo and co-op adjustments change pressure/opportunities with bounded state, not only health inflation.

Boss spawning/recovery/farming has discoverable ecological, ritual, or industrial conditions, costs, cooldowns, and population/resource provenance. Mana/experience collection is useful and automatable where supported, but does not duplicate rewards through respawn/reload or create mandatory cruelty for basic resources.

A giant remnant may stay dead. A large creature may be a neutral migrating ecosystem. Not every impressive silhouette needs a health bar.

## 14.6 Region briefs retained for expansion

| Place | Core experience and systems |
|---|---|
| Fernwake / Windward Basin | Comfortable valley, mill, spring/karst, distinctive strata/arch/ferns, optional physical oddity, early glider, useful first factory; exact placement follows repository canon |
| Orchard Above the Sea Gates | Inland remains of a former sea-gate complex organize canals/terraces, college, homes, waterworks and exploration |
| Orchard Drydock | One attractive real construction bay and an older service level; player-built shuttle belongs to the workshop landscape |
| Redwater Reach | Mineral/biological red basin with inhabited service terraces; colour does not alone define toxicity; restoration improves a specific water issue without erasing ecology |
| Pass of Three Needles | Distinct mountain silhouettes, freight/ridge/climb approaches, shelter and weather knowledge, later useful flight; real geological grandeur |
| Broken Procession | Traceable annular freight/guidance infrastructure, pattern learning, missing segments, later occupation; rings are not automatically portals |
| Fallen Keeper | Dead civic construct in a working/reused water or weather chamber; alternate entries, contested salvage/memorial meaning, not obligatory boss awakening |
| Hanging Parish / Veyr Below | Coherent local gravity, counterweights, water/choir machinery, a small first passage/court/return loop; expand only after real spatial physics |
| Salt Walker | Moving ecological wonder, migration, shed materials, care, pilgrimage, optional conflict; not an endless kill-for-drops source |

Retain compatible older regional story webs—Lowwater Compact, Sound That Remained, Unclaimed Freight, Garden of Local Winters, Corrected Sky—as authored content candidates rather than mandatory global quest chains. Do not reset the current world to install every example.

---

# 15. Cosmology, parallel dimensions, and realm rules

## 15.1 Topology before visual presentation

A universe family contains material astronomical space and associated parallel realm instances. A body is not a dimension; a local frame is not another world; a portal is an explicit authorized edge, not a global teleport permission.

Realm rules are versioned data governing local gravity/fields, atmosphere/pressure, material/thermal effects, magic, ecology, and transit constraints. Keep the common simulation clock. Any rate modifier applies to defined processes, not the length of a network tick. Different laws must have measured/observable consequences, not arbitrary palette swaps or silent exceptions.

Not every planet needs a full instance of every realm. Distribution should reflect history and create different economic/exploratory opportunities. A planetary system is not a mandatory checklist of all archetypes.

## 15.2 Realm profiles

**Material worlds:** coherent physical geography, rotation/orbits, ordinary local laws plus explicit magitech. Flight and legal portals connect supported destinations. Familiar landscapes remain the home and long-term return point.

**Crimson Mirror:** each eligible material planet has a corresponding distorted hell pocket beneath a shared deep red-black cosmos. Parent landmarks remain semantically recognizable through authored inversions: drowned/ash/petrified/glass/brine/ember ecologies, altered architecture and dependencies. This is not literal sign-inversion of geometry or negative density. Safe arrival volumes and access are validated.

Crimson pockets do not occupy one traversable interplanetary flight volume. Ordinary local portals and paired material returns cannot be forged into arbitrary pocket-to-pocket routes. Local vehicles can work under the pocket's rules. The Hall authorizes exceptional discovered cross-pocket/family edges. Explain the sky's unreachable correspondences rather than drawing a reachable planet and adding an invisible arbitrary wall after hours of flight.

**Underwritten City:** displaced/decommissioned districts remain real, wearing, inhabited spaces joined by surviving addresses/anchors. Not time travel or duplicated historical loot. Restore freight, investigate drainage/records, survey connections, or dispute ownership. Unauthorized address changes can isolate people. Connections remain specific; the realm is not a loophole connecting every planet cheaply.

**Green Continuance:** a society's infrastructure became inheritable biology. Living filters, conductive tissues, valve roots, carrier organisms and self-repairing structures require nutrients, energy, habitat and time. Cultivation and ecological restoration change actual processes. Coercive growth has consequences; benign routes are effective.

**Tideglass:** ocean/pressure-bound infrastructure with engineered or magical interfaces between fluid regimes. Pressure, heat, buoyancy, flow, containment, equalization, and transport create choices. Portals into pressure volumes need supported containment/flow policy; not surprise automatic base deletion.

**Null Choir:** sustained enchantment loses coherence. Ordinary mechanics, electricity and chemistry work. A material flame remains a material flame; an enchanted support needs structural replacement. Refugees, escaped bound beings, engineers, and containment institutions inhabit it. Coherent magical pockets require explicit supported sources and costs.

**High Reaches:** inhabited aerial archipelago organized around stable, readable gravity anchors, weather routes and trade. Functional air transport, field maintenance, wind knowledge, and shelter matter. No random arbitrary gravity roulette. Field changes wake relevant bodies and affect existing routes.

**Last Hearth:** communities of anchored memory/identity. Anchors may be places, objects, or continuing relationships. Preserve uncertainty about their ultimate nature; do not claim the game proves a real afterlife. Consent, preservation, testimony, recovered expertise, and coercion matter. Persons are not simply fuel items.

**Manyfold and constructed domains:** bounded spatial/liminal/gothic regions, unusual topology and local rules; some belonging to powerful institutions/wizards. Use original mystery and atmosphere, not endless repeated hallways. Preserve contextually meaningful silence, safe exits, and actual geometry/portals. A new realm must introduce a different interaction/history/relationship, not only a colour.

## 15.3 Realm interfaces

At crossings, evaluate permission and rule effects authoritatively. A machine does not secretly run unchanged where its sustaining magic fails; UI identifies the actual condition. Preserve material/identity/accounting during transformation. Temporary visual adaptation does not reroll original items or grant free inventory.

Use common tags/interfaces to avoid a hardcoded exception for every machine on every planet. Still author realm-specific content; an all-purpose scalar multiplier cannot express a genuinely different world.

---

# 16. True immersive portals and cross-frame transfer

## 16.1 Required progression of proofs

Prove a static local paired aperture; then arbitrary orientation/gravity; then moving endpoints; then a remote body; then material/Crimson routing; then co-op viewing/crossing; then compatible loose objects/assemblies; finally vehicles, coupled fields, and optional fluid/network continuity where explicitly implemented. The architectural foundations begin early; broad content waits for working crossings.

A portal must look like an actual opening with correct parallax, visible remote actors, terrain changes, occlusion, and continuity while walking through. A screenshot texture, black fade, hidden teleport corridor, or loading pop is not a passing implementation.

## 16.2 Render and streaming contract

Each portal view owns destination frame/origin, camera, aperture/exit clipping, relevant sky/sun/fog/exposure, shadow context, and temporal history. Use the engine's actual WGPU depth convention and supported clip/compositing path; do not paste an OpenGL oblique-projection formula unverified.

Use depth/stencil/scissor or equivalent verified composition. Correctly clip source/destination halves of actors and geometry at the aperture. Near-plane crossing must not slice the frame or show a black frame. World-space effect trails, particles, water, reflections, and exposure require explicit behavior; do not claim full light transport merely because the view is visible.

Budget projected visible area, render-target pool, recursion depth, refresh rate of noncritical distant views, and total visible destination detail. Begin with recursion one; a bounded second level follows evidence. A recursion cap affects rendering, not legal transport topology. Never require full destination-planet residency for a small view.

Destination streaming includes the view cone and a safety/interaction envelope near the exit. Private/locked destinations are not transmitted just for preview. A destination not ready remains visibly inactive and collision-blocking. Crossing activation checks a coherent progressing baseline/frontier, not a timeless visual-cache flag.

## 16.3 Moving-frame mapping

Use a configured proper rotation between endpoint local frames, commonly a half-turn around the chosen aperture-local up axis for opposing faces. Test handedness rather than accidentally reflecting the world.

Conceptually, for source/destination endpoint poses `A`, `B` and through-aperture local rotation `H`:

```
M = B * H * inverse(A)
p_out = M * p_in
R = rotation(B) * H * inverse(rotation(A))
v_out = v_B_at(p_out) + R * (v_in - v_A_at(p_in))
w_out = w_B + R * (w_in - w_A)
```

`v_endpoint_at(p)` includes endpoint translational velocity plus angular velocity crossed with the point's offset from the endpoint origin, evaluated in compatible coordinates. Orientation, camera basis, attached items, and relevant gravity/reference frame follow the same mapping. These are integration relationships to validate against the actual math conventions, not drop-in source code.

Preserving portal-relative motion does not guarantee conserved inertial kinetic/potential energy across separated/moving endpoints. Any change attributable to the portal is explicit fictional work with a power/cost policy, preventing an accidental free-energy machine unless that behavior is deliberately designed and balanced.

## 16.4 Crossing and ownership

Detect swept hull/capsule/assembly crossing, aperture fit, destination obstruction, and crossing direction. Support objects partly through where the collision/render model promises it. Use separation-based hysteresis against duplicate trigger contacts without preventing a genuine immediate return crossing.

Transfer the authorized closure of owned attachments exactly once: actor, equipped items, inventory, carried objects, vehicle modules, compartments, and crew as applicable. Define handling for ropes, very large straddling assemblies, active connections, and collision constraints; do not silently cut or duplicate them. Unsupported cases visibly decline before committing.

Stage destination state, reserve resources, validate permission and source revision, commit realm/frame ownership at a declared tick, and record a replay-safe transfer receipt. Handle opposite-direction simultaneous travellers, disconnect, duplicate messages, and stale endpoints. A mid-transfer save/restart resolves to one valid ownership state.

Closing a portal with occupants or spanning assemblies follows a published grace/safety policy. It cannot delete bodies, seal an actor inside nonexistent terrain, or clone cargo. Choose the smallest correct initial policy, such as delaying closure until clear, before designing dramatic hazardous closures.

Fluid/gas/heat/electrical/mechanical continuity through portals is separately versioned and tested. Initially a deliberate containment membrane may prevent fluid transfer; label it honestly. When coupling is supported, matched flux, domain boundaries, pressure, latency and conservation apply. A visual waterfall alone does not prove transported water.

# 17. Player-built voxel spacecraft, flight, and orbital construction

## 17.1 A real construction category

Creative includes **Spacecraft & Space Building**, with search/filter and editable starter blueprints. Subcategories cover training craft, hull/pressure, cockpits/control, wings/surfaces, propulsion/propellants, power/air/heat, cargo/landing/docking, and station construction. These are functional parts and ordinary editable shapes, not purchased immutable vehicle skins.

Survival obtains the same parts through appropriate processes, salvage, purchase, repair, or commission. Use a construction cradle/boundary that distinguishes the vessel from its hangar, terrain, and scaffold. Launch preserves the same voxel geometry, modules, paint, damage, inventory, and identity. No hidden replacement vehicle, duplicate static hull, or accidental launch of an attached mountain.

Support real body structure plus optional exterior fairings. Pressure boundaries, structural supports, visual skins, aerodynamic patches, and collider geometry can have different roles, but their relationship must be explicit and truthful.

## 17.2 Simplest suggested design: Kestrel-1

Begin with a single-pilot winged shuttle for a short orbital expedition and return. Approximate initial art proportions are 9 m length, 3 m fuselage width, and 7 m wingspan; they are **unvalidated layout targets**, not certified flight design.

Layout: shaped nose/canopy → sealed cockpit and hatch → compact service package and secured small cargo space → propellant assembly → main engine. Add two modest wings with independent elevons, fin/rudder, landing contacts, protected reentry-facing surfaces, and a small useful manoeuvring-thruster arrangement.

Minimum functional systems:

- Real editable pressure cockpit with seat, instruments, canopy, frame, panels, and hatch; not a seat projecting invisible atmosphere.
- Connected load-bearing assembly, suitable shape, functional wings/control actuators for the atmospheric mission, and accessible repair.
- Rocket with finite fuel and oxidizer supply, controllable thrust/gimbal where installed, and actual resource connections.
- Attitude/translation thrusters or other installed supported controls for vacuum.
- A convenient service package combining separate battery, cabin reserves, circulation/scrubbing, and thermal functions behind a simple interface.
- Heat rejection and thermal protection appropriate to the flight profile; landing gear and a cargo locker.

Do not begin with reactors, bedrooms, turrets, full industrial workshops, or capital-ship logistics inside the first shuttle. Packaged convenience is legitimate; unlimited hidden power/fuel/air is not.

One guided detachable launch booster is an acceptable starter configuration when the declared planet/engine model needs it. A single-stage craft is permitted only if the actual mass/propulsion/atmosphere/thermal model supports it. Do not falsely establish spaceflight by injecting orbital speed through an unlabeled debug command.

The beginner blueprint remains editable. Provide clear diagnoses for a missing seal, disconnected actuator, inadequate steering, blocked exhaust, inaccessible hatch, insufficient reserve, obstructed radiator, poor load distribution, or an unreachable planned trajectory. Do not rely on a vague "ship invalid" score.

## 17.3 Airtightness and habitability

Determine pressure regions from actual free-space connectivity and configured impermeable boundaries, including slopes, thin surfaces, partial voxels, windows, hatches, fine/coarse seams, and service feedthroughs. An occupied cabin requires a valid pressure boundary; open wings and engines do not.

Keep sealed, pressurizable, supplied, and habitable as distinct statuses. Track supported gas quantities/species, volume, thermal energy, pressure relation, contaminant removal, and relevant physiology. A high oxygen fraction at insufficient total pressure is not automatically safe. Outside may be atmosphere, water, gas, or vacuum.

Expose a leak overlay with the actual opening, boundary path, current loss, and isolation option. Unknown neighboring topology cannot certify sealed safety. Compartment merge/split after edits conserves gas and heat under defined volume/displacement rules. Closing a door does not refill a room. Destroying a window changes leak area and pressure at the accepted state transition.

Use finite intercompartment/exterior flow. Handle equalization, venting/recovery, interlocks, and physical access. The smallest craft may have one hatch and require suit protection before opening in vacuum. A larger airlock is a useful upgrade, not a mandatory corridor in every beginner vehicle.

A breach creates a recoverable situation, not automatic deletion or engine shutdown. A suited player may isolate a compartment or return for repair. Optional difficult modes can make severe cases harsher, but ordinary play gives warnings and clear actions.

## 17.4 Flight, shape, and orbit

Use mass/inertia, actual thrust and lever arms, finite tanks, environmental-relative velocity, atmosphere density/wind, supported lift/drag, steering limits, heat, and changing cargo/fuel distribution. Implement bounded aerodynamic panels/patches rather than per-voxel CFD.

A wing hidden inside the body is not fully effective. A disconnected rudder cannot steer. Atmospheric surfaces lose authority as usable air vanishes; a vacuum tug does not need token wings. Shape influences performance without requiring one approved aesthetic.

Flight assistance commands installed actuators; it cannot add free force or infinite stability. Provide useful pitch/roll/yaw assistance, guidance, predicted path, reserve estimates, target-relative docking, and manual overrides. Explain inadequate authority instead of secretly correcting any design.

Use coherent orbital state/ephemerides and gravity from the same body parameters. Distinguish a high-altitude returning arc from a stable orbit. No atmosphere boundary that turns gravity off, fake skybox-space transition, or world-speed reset. Analytic celestial ephemerides and a bounded vehicle dynamics model are acceptable; unrestricted n-body simulation is not required.

Launch, insertion, coast, rendezvous, approach, docking, deorbit, reentry, and landing are distinct supported actions. Atmospheric rotation and relative winds matter to the implemented model. Time acceleration in multiplayer cannot independently advance one participant's shared factories and vehicles; use explicit common policy or bounded coarse noninteracting travel where valid.

Heat generation/storage/rejection, propulsion work, life support, and supplies persist during travel. Magic engines and artificial gravity are allowed only as explicit later fictional capabilities with inputs/limits, not hidden corrections to unfinished physics.

## 17.5 Moving assemblies and multiplayer interiors

One vessel is a coherent assembly with submodules and a small supported articulated graph, not a body per voxel. Compile colliders, mass properties, compartments, power/fluids, control channels, structural bonds, and render products incrementally from source revisions. Removal of an engine updates thrust, mass, structure, pressure, and networks coherently.

Seated flight is an early valid milestone. Claim walkable co-op interiors only after passengers can move during translation, rotation, acceleration, docking, and network correction without falling through, being left behind, or receiving deck velocity twice. Frame-local locomotion and actual contact must share a coherent physics time with the ship.

Define zero gravity, acceleration-induced apparent weight, rotating habitats, landed support, and explicit artificial gravity separately. Do not fake a stationary flat floor while the exterior rotates independently. Passenger/vehicle attachments, cargo restraints, and dropped objects preserve appropriate point velocities when attaching/detaching.

Fuel burn, moving cargo, damage, and split topology update mass/inertia. Critical data cannot wait for the decorative render mesh. Strong stress fractures produce meaningful conserved fragments; no free salvage by launching/docking/splitting the same piece repeatedly.

## 17.6 Stations and docking

Stations use the same hull, hatch, atmosphere, power, thermal, cargo, and construction systems. Survival construction above a planet does not automatically become a stable orbit; creative may offer an explicitly labeled orbit-placement tool.

Docking validates relative pose/speed, port compatibility, clearance, authority/permissions, and supported contact capture. Establish constraints/service connections deliberately. Equalize pressure before opening. Preserve distinct vessel identities rather than merging everything irreversibly into one anonymous block pile.

A friend can arrive with their own craft, dock, unload real cargo, refill supplies, repair, and leave in the same craft. Save/reload and disconnect at any stage preserve exactly one copy of every person, component, and item.

Portals transport a craft only when the entire supported assembly fits and the transfer closure is valid. Preserve compartments, modules, crew, cargo, fuel, and movement. Ordinary ships cannot bypass Crimson topology.

## 17.7 Progression and destination scope

Maintain capability bands: comfortable workshop/regional tools; precise fabrication and atmospheric craft; first self-built crewed atmosphere exit near 60%; reliable orbital return near 60–65%; local space industry; interplanetary specialization; later exotic transit and megastructures. Purchased passage or a restored vehicle can offer deliberate earlier access without making self-built ascent trivial.

Early limited gliding belongs before sophisticated atmospheric vehicles. Do not require the final IC process node or advanced reactor to build useful avionics. A small rugged controller and sensible service package should suffice for the initial validated mission.

Prove one worthwhile orbital service-platform round trip and one additional distinct world before mass expansion. Keep a later archetype catalog: airless moon, thin-atmosphere arid world, ocean/archipelago world, tidally locked world, volcanic moon, ice-shell ocean moon, cold hydrocarbon world, hot dense-atmosphere world, demanding-gravity rocky world, gas-giant system, and remote ice-giant region. Existing proposed names include Cairn, Rook, Thalassa, Vesper, Cinder, Nacre, Sable, Aureole, Ferrum, Orison, and Pale Reach; retain verified canon instead of forcing them into every seed.

Each completed destination needs distinct travel/engineering decisions, multiple regional environments, inhabitants/ecology/history appropriate to its conditions, useful first-arrival activity, scientific observation, several reward types, and a meaningful return. No one-exclusive-ore planets or ordinary ground planes under gas giants. Distant rings/asteroids may use bounded summaries; any interacted object needs persistent identity and appropriate geometry.

---

# 18. Meridian infrastructure, radio, skyhooks, and real cargo

## 18.1 Distinguish the structures

A surface elevator, rotating skyhook, launch/guidance ring corridor, station, radio relay, mass driver, orbital port, and portal nexus are different systems. Their roles and silhouettes must not collapse into a generic giant glowing obelisk.

Choose infrastructure according to the world's gravity, rotation, orbit availability, terrain, material/field support, and intended transport model. Do not stamp a synchronous elevator on every moon regardless of its feasible orbital geometry. Fictional support is acceptable when explicit and observable.

A rotating skyhook has an ephemeris, reachable transfer envelope, rendezvous conditions, clearance and operation schedule, cargo capture/release, and supported momentum/energy effects. A slow dramatic descent is not evidence a tether can physically scoop any arbitrary moving city. Treat extreme city-lift events as separate later authored mechanisms requiring their own load/attachment/safety model.

## 18.2 Radio and map information

Radio stations announce real permitted schedules, changes, distress, weather, shipments, and rumors according to their knowledge. The map may display an observed/reported landing/drop zone and expected window with uncertainty and provenance. Missed routine windows recur or have alternatives; a missed appointment must not permanently lock ordinary progression.

A shipment has one consignment ID, source stock, custody, carrier, destination, quantity, and recorded transfer events. Coarse transit and nearby pods are two levels of the same cargo. No spawning rewards separately in the radio event, pod, and warehouse.

Drops respect configured eligibility, clearances, ownership, existing builds, and safe interaction policy. Deliberate danger is telegraphed; routine supply does not destroy a player's home because a random spawn chose the roof. Recoverable failures can create salvage/rescue opportunities without resetting the same free supply repeatedly.

## 18.3 Why a base benefits

A functioning outport, warehouse, crane, receiver pad, radio, surveyed route, and manufacturing capacity improve actual logistics. Buyers/sellers/recruits can use the added capacity. A player may automate deliveries or manually meet an arrival for enjoyment.

Tie the regional land network to orbital cargo gradually. First prove loading a real container at a local dock and receiving that same consignment elsewhere; then a scheduled orbital exchange. Spectacular infrastructure should support this everyday utility.

---

# 19. The Hall of the Crimson King — persistent multiversal nexus

## 19.1 Identity and geography

Build an original terrifying hanging palace/machine-city within the Crimson topology. It is not a generic Nether fortress, a copied final arena, or a flat megabox with repeated tiny rooms.

Use enormous dark masses, long voids, pale structural surfaces, hanging causeways, deep arches, weathered brass, old navigation machinery, restrained furnace/stellar light, and a throne/nexus recognizable from several approaches. Human-sized work, habitation remnants, stairs, shelter and equipment establish scale. Do not cover everything in red emission or spikes.

Separate original mega-scale ambition from initial playable volume. Begin with a convincing approach, interconnected precinct, and two complete wardens; grow the actual structure without an arbitrary kilometer target forcing empty content. All remote silhouettes that promise reachable structure need a compatible eventual physical representation.

## 19.2 Routes and occupation

Three distinct route families should emerge: foundry/service infrastructure; vertical choir/astronomical machinery; and cistern/ossuary/underworks. They differ in hazards, mechanisms, movement, inhabitants, and information, not merely corridor colour. Preserve loops, shortcuts, safer staging points, and consequences of player construction/destruction.

Traps, wards, machines, pressure, gravity, and ecology use shared systems. Some sectors can be partially reclaimed. Dangerous inhabitants and surviving maintenance traditions make it more than an empty boss sequence. Optional exploration remains worthwhile before and after victory.

## 19.3 Warden progression

| Warden | Infrastructure and encounter identity | Example supported alternatives |
|---|---|---|
| Furnace Saint | Heat, pressure, furnace routes, moving shielding, constrained high-output attacks | Cooling/venting, redirecting flow, severing supply, physical combat through readable openings |
| Chain Astronomer | Tethers, alignments, machinery loads, local gravity or orbital instruments | Rebalance/cut/retime mechanisms, use geometry and movement, interrupt exposed control phases |
| Drowned Archivist | Cistern pressures, records/identity vessels, flooded routes | Isolate/drain/equalize, negotiate or disrupt anchors where appropriate, fight using changed environment |
| Ash Choir | Distributed resonators, propagation, shared ritual/control apparatus | Dampen/separate/retune channels, use cover and timing, break connected sources |

The names establish original content directions, not permission to fake functionality with four counters. Each warden needs readable attacks, recovery, habitat/arena, failure feedback, and at least a conventional combat resolution plus meaningful systemic interference.

Defeating or equivalently resolving **any two of four** enables the King's challenge. Remaining wardens influence the encounter through visible functioning systems. Do not mandate four copied pillars or a fixed completion order. Joining players, difficulty adjustments and persistent world/party progression have explicit authority rules.

## 19.4 King, extraction, and return

The King is an original encounter using the throne/nexus machinery, portal apertures, weighted bridges, spatial changes, and realm conditions. Fair telegraphs, physical hit volumes, supported counters, safe recovery openings, and state persistence matter more than giant HP. Players' engineering and spells remain relevant.

Victory initiates a playable realignment/extraction with multiple viable routes and team rescue choices. Do not copy another game's exact boss, timing, arena, or gear-removal sequence. Keep a valid return route before progression and avoid trapping disconnected players permanently.

Successful extraction persistently enables **charted routing to discovered/authorized destinations**, including exceptional other pockets and eventually other universe families. Unlocking is not omniscient discovery or a global bypass of permissions. Server/world or explicit shared-group progression owns the capability; a client-local flag cannot forge it.

The Hall remains revisitable, partially usable, dangerous, and potentially farmable through disclosed ritual/resource conditions. Repeated rewards need real reset cost/state and transaction identity. A win does not permanently delete every exploration opportunity or require wiping player storage.

---

# 20. The flagship integrated region and its first real journey

## 20.1 Preserve continuity

Locate the verified existing Fernwake/Windward fixture and the current Orchard/Sea Gates/Drydock plans. Preserve their historical identities and edits. When feasible, make them parts of a connected regional demonstration; if the repository establishes different locations, connect by legitimate routes rather than overwriting coordinates.

The central image is a player-built useful workshop beneath ancient infrastructure, with orchards/water/woodland outside, friendly activity, an investigable service level, and a credible place to build the first craft later. The planet's enormous past supplies wonder; contemporary work gives it value.

## 20.2 First integrated expedition

Start at a comfortable small home/workshop with good storage and a useful device. Residents work competently; the college/waterworks has a visible local concern rather than an exposition dump. Follow the stream through grouped forest into an old canal/embankment. Ceramic repetition and altered banks reveal history before text explains it.

Provide at least three feasible approach styles across the whole loop: maintained route, water/service access, and climb/build/dig or another supported alternative. They reveal different evidence, not the same room three times. A local mechanism affects a supported flow, route, or service; repair, bypass, or negotiation leads to a persistent consequence.

An upper view reveals a larger infrastructure pattern and a mountain route. A quiet scenic discovery offers relief from instrumental tasks. The return reveals changed knowledge/state: a working crossing, reliable water, improved access, a relationship, or a recovered useful component.

Do not require the player to solve the concern before living in the region. It is one invitation among building, farming, travel, trade, magic, and observation.

## 20.3 One complete drydock bay

Create one well-composed bay with real support, cargo/service access, a working lift/handling interaction, and sufficient clearance for the editable starter shuttle. Native before/after views include the approach, under-hull/work level, gantry view to landscape, rear/service side, and interior.

An older lower gallery shows original function, a later alteration, and present reuse. One large surviving fitting hints at lost scale without requiring a working capital ship. Do not enlarge this into a district of empty hangars before the first bay works.

First launch and orbital return later reuse this same place and its inventory/manufacturing, not a separate vehicle test map disguised as integration. Creative fixtures may accelerate tests but the survival loop must eventually be playtested without privileged resources.

---

# 21. Interface, sound, accessibility, and modding

## 21.1 Interface identity

A clean Minecraft/Noita-like immediate interaction layer, refined for this world's pixel-material craft: readable hotbar, inventory/storage, concise contextual inspection, restrained magitech framing, consistent spacing, crisp scalable typography, and useful diagrams. Avoid generic dashboard cards, tiny gold text, oversized decorative borders, and menus full of unimplemented buttons.

A wand editor shows actual execution order/cost; machine UI actual bottlenecks; storage actual permissions/capacity; atlas knowledge provenance; cockpit actual pressure/power/propellant/trajectory; portal readiness actual reason. Debug telemetry is available to developers, not dumped onto ordinary players as unexplained scheduler jargon.

Support mouse/keyboard first according to current platform, rebindable controls, readable controller accommodation where present, scalable UI, colorblind-safe signals, reduced flashes/motion, subtitles/visual sound equivalents, and clear interaction focus. First-person equipment must not cover the most important aiming/inspection area.

## 21.2 Sound and presence

Sound supports navigation and state: water within walls, real machinery cycles, weather exposure, changes in enclosure, footsteps/material, distinct tools, creature movement, distant inhabited activity, and rare magical motifs. Signals correspond to real sources and obey the supported acoustic model.

Keep quiet stretches. Constant epic music, fake mystery sounds, and nonstop idle dialogue weaken the landscape. Sound through portals or pressure boundaries needs an explicit limited model; don't imply complete acoustic physics while playing the same unoccluded loop everywhere.

## 21.3 Extensible content without unsafe magic hooks

Use versioned registries and typed stable IDs for materials, species/body plans, items, recipes, machines, spell operators, biomes, structures, realms, instruments, events, audio, and localization. Validate data at load with actionable errors and dependency checks. Hot reload should not silently mutate shared canonical state; preview, migration, and session restart rules are explicit.

Deterministic scripts receive bounded fuel/allocation, stable time/RNG/query APIs, and no uncontrolled OS/network/thread access. Server-only nondeterministic decisions enter as authoritative logged events; presentation mods cannot change collision or resources. A deterministic library does not make arbitrary native plugins compatible.

Keep content iteration easy: schema examples, editor/test fixtures, searchable spawn/inspect tools, reproducible site generation, and automated graph/clearance validation. These support real content creation, not replacement of asset authorship with random procedural noise.

---

# 22. Delivery sequence and dependency gates

This is an implementation program, not a calendar estimate. Audit may establish that some gates already pass. Reuse demonstrated work, repair the weakest blocking edge, and record genuine completion. Broad product scope does not justify a branch containing dozens of disconnected half-features.

| Gate | Complete integrated result | Required proof before expansion |
|---|---|---|
| P0 — Audit and reproducible baseline | Existing engine builds or blockers isolated; ownership/saves/fixtures known; actual failing scene captured | Clean reproduction instructions, source/status table, known failures, baseline traces/captures |
| P1 — Stability and coherent world edits | Default resource lifecycle bounded; safe current collision/visual edits; save/ownership correctness | Allocation/cancellation/resize/minimize tests, border edits, restart, no stale wall or ignored damage |
| P2 — Responsive two-client core | Shared crate, mine/place, storage contention, real local physics, semantic receipts, coherent reconnect | Two-client impaired-network run, no duplication, no slow-peer barrier; exact replay prototype with full restore |
| P3 — Home and reference-region quality | Useful home/storage/crafting, geological valley/stream/cave, attractive kit/vegetation/wand, one real regional loop | Native multi-angle/motion/day-night review, 90-minute multi-intention playthrough, persistence and alternate approaches |
| P4 — Industry and material pilot | Useful pump/controller chain plus heated tank/hinge/crate coupling, observable energy/material ledger | Interrupted batches, fluid/solid/heat conservation, minimum-hardware workload and co-op editing |
| P5 — Magic, ecology and meaningful dungeon | Useful composable wand, Earthfold or equivalent real physical spell, one cultivated creature, inhabited compound or dungeon | Counterplay, transformations and collisions, noncombat alternative, anatomy/navigation and bounded effects |
| P6 — Deterministic domain delivery | Proven compatible closed-domain command replication with fallbacks and efficient snapshots | Cross-machine/worker/render-rate replay, checkpoint continuation, boundary merge/split, healthy low-repair traffic |
| P7 — Spatial portals and first mirror | Local then remote/moving portals, material-to-Crimson pocket, authoritative rules | Native crossing/occlusion, two players/items, loading/readiness, save/reconnect, rejected forbidden routes |
| P8 — Atmospheric construction and orbital round trip | Editable airtight Kestrel and optional booster, guidance, actual orbit/return, real platform/cargo | Survival capability path, pressure/flight diagnostics, fuel/heat, frame/ownership, preserved home/workshop |
| P9 — Inhabitants, routes and orbital utility | Regional trades/journeys/recruits, station docking, scheduled same-cargo transfer, useful logistics | Coarse/detail identity, simultaneous users, missed schedule/recovery, economy without compulsory chores |
| P10 — Rich second world and new realm behavior | Distinct useful destination and one additional realm's real rule difference | World distinctness, society/ecology/engineering, portal/frame/privacy conservation, return value |
| P11 — Hall first precinct | Original approach, route families, two complete wardens and associated machinery | Fair combat/systemic alternatives, co-op, native atmosphere/silhouette, useful revisits |
| P12 — Hall progression and Unreturned | Four-warden/two-required King/extraction/nexus loop; bounded personal shadow storyline | Flexible order, durable shared unlock, extraction rescue/recovery, anti-dupe farm, safe homes and player-specific responsibility |
| P13 — Scale and production content | More systems/regions/vehicles/chip depth built from proven interfaces, original authored assets and broader modding | Full mixed-world performance/correctness suite, licensing/import/repro builds, measured capability envelope |

**Important overlap:** authority, deterministic ordering, save compatibility, interfaces, and two-client smoke tests are present from early gates. P6 is the evidence gate for the richer command-replicated path, not permission to bolt multiplayer on after completing the game. Portal and spacecraft interfaces begin before all their art/content. Art validation occurs in every player-visible gate, not as a final coat of paint.

## 22.1 First implementation session

Read local instructions and perform the bounded audit. Reproduce a current failure or missing end-to-end path. Choose the earliest unmet gate, identify touched ownership boundaries, and implement one vertical slice with code, assets if relevant, tests, native evidence, and a clear handoff.

A likely priority is default memory/resource lifecycle plus current-edit continuity, or responsive two-client mining/storage if stability already passes. If these already work, complete the existing stream/gallery/workshop loop rather than invent a new test island. Do not assume the highest-priority issue from old chat remains present.

For each change use: hypothesis → failing fixture → minimal integration → automated tests → native test → profiling/visual review → keep or revise. A classifier/reviewer may flag risky edits and select tests, but cannot certify correctness in place of execution. Agent-generated self-praise is not an acceptance criterion.

## 22.2 Scope control

Long-range catalogs are not immediate obligations to make every named item. No dummy registered realms, empty boss arenas, placeholder ship classes, or unimplemented UI buttons counted as completed scope. Complete one representative of each new interaction family, validate extensibility, then scale content.

Do not ask for confirmation of requirements already settled here. When a detail is genuinely open, make the smallest reversible consistent choice, identify it as provisional, and record evidence needed to settle it. Escalate only destructive/incompatible decisions or genuinely missing authority; continue safe independent work.

# 23. Performance, correctness, and visual evidence contract

## 23.1 Establish comparable measurements

Record build/commit, world/save/seed and content manifest, CPU/RAM/GPU/VRAM, storage, OS/driver/backend, resolution, presets, cache state, player count, transport route, and impairment settings. Record listen-host rendering and simulation together; headless results alone do not establish hosting performance on a player's PC.

The user's RTX 4090/24 GB machine is a useful high-end profile, not the minimum supported platform. A low-end i3/8 GB machine is only a valid exploratory target when its actual GPU/storage are known and tested. Do not promise support from a model name. Keep correct conservative baseline rendering on declared supported hardware.

Measure cold launch/menu, new-world global preparation, cold/warm saved entry, first/cached join, reconnect, destination readiness, and current-edit response separately. Measure whole-frame/tick wall-clock critical paths and GPU passes. Individual subsystem percentiles cannot be added to form a real frame percentile.

Report p50/p95/p99 and worst frame/tick, stall counts over explicit thresholds, state age, queue bytes/work/oldest age, post-burst drain time, pipeline/cache misses, useful versus wasted rebuilds, copies/uploads, total memory and reservations/retirement, deterministic mismatches, correction frequency/magnitude, and total network bytes including recovery. Add p99.9 when run length makes it meaningful.

## 23.2 Initial experimental targets, not claims

Use the existing validated fixed simulation rate; 60 Hz is a candidate if the measured critical path leaves headroom. For a ready ordinary edit, aim for first locally submitted feedback within one presentation frame at p95 and predicted physical change at its next eligible simulation tick. This is not input-to-photon latency and does not remove authored mining duration or remote confirmation delay.

Before an optimization, set both relative and absolute acceptance values for the actual problematic scene. Prior suggested experiments were at least 30% lower p99 frame time and 50% lower median warm time to first safe interaction. These are candidate objectives, not predicted outcomes; an already fast scene should not be destabilized to pursue a percentage. A 30% improvement from an unusable baseline can still fail the absolute target.

Keep declared memory/admission/operation limits visible to developers and admins. Run increasing edit/surface/contact tiers to establish an honest supported envelope; do not label every 1,000/10,000/100,000-cell destruction test supported in advance. Fine checkerboards, railings, transparent foliage, and material-fragmented surfaces may cost far more than equal-volume solid blocks.

## 23.3 Required mixed workload

Use one saved inhabited region with an operating factory, water/pressure interaction, vegetation, a detailed building interior, and persistent edits. In four-player runs: one explores/streams, one builds/operates machines, one uses combat/destruction, and one joins/reconnects. Repeat with direct and forced-relay connections, upload constraints, and one deliberately stalled client.

Later expand the same harness to a moving vessel/passengers, visible portal destination, planetary seam, and orbital arrival. Test rapid return to evicted regions, long edit sessions, view reversals, resolution changes/minimization, shader-cache invalidation, and controlled memory-budget pressure. Check that performance improvements do not delete inhabitants, disable fluids, shrink gameplay, remove shadow casters, or silently simplify collision only on one client.

No empty flyover, paused simulation, static screenshot, localhost ping, backend microbenchmark, or standalone fluid demo substitutes for the integrated game workload.

## 23.4 Visual and experience reviews

Capture native before/after with matched seed, pose, FOV, time/weather, settings, and exposure, plus short traversals. Include ground-level, contact detail, middle distance, distant skyline, reverse approach, interior, night, motion, edits, and lower-detail settings. A generated concept image never counts as engine evidence.

Review silhouette, colour masses, material distinction, functional architecture, route comprehension, player-kit parity, first-person equipment, vegetation morphology, lighting, seam continuity, and actual fun. A test suite cannot certify those visually. A beautiful still does not certify motion, editability, physics, or native performance.

Review several seeds early, then a broader deterministic seed set when generator scale expands. Verify that procedural variations still preserve access, habitat, clearances, meaningful terrain, and aesthetic organization. Hand-fixing one camera fixture does not finish the generator.

---

# 24. Acceptance catalog

Every case below is a required implementation test/review contract, **not an executed test result**. Initial status is NOT_RUN. The companion `AXIOM_ACCEPTANCE_MATRIX.json` is generated from this same catalog for tracking, not a second source of game requirements.

Automate semantic invariants where possible; retain native visual and interactive reviews where necessary. For a pass, attach exact build, command or manual procedure, fixture/save/seed, hardware where relevant, output/capture/trace, and observed result. A failed or unavailable check is FAIL or BLOCKED/NOT_RUN, never a pass inferred from compilation.

## AUD — Repository, execution, and evidence

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| AUD-01 | Reproduce the current native baseline | The actual repository, branch, dirty files, executable, seed/save, dependencies, and source ownership are recorded; prior reported status is not substituted for inspection. |
| AUD-02 | Clean checkout and asset dependencies | All required owned source and original asset inputs are tracked/reproducible; unrelated work is untouched and no hidden local file is required. |
| AUD-03 | Existing save migration and rollback safety | Backups and migrations preserve canonical entities, construction, quantities, and versions; an incompatible migration fails safely without silent regeneration. |
| AUD-04 | Authored asset to native interaction | An original authored item retains editable source, scale, pivots, materials, collider/ports and import provenance, and is inspected while used in the native game. |
| AUD-05 | Completion report truthfulness | Implemented, unit-tested, native-observed, playtested and performance-validated are reported separately; failed/unavailable checks and placeholders remain visible. |

## WLD — Canonical world, generation, and persistence

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| WLD-01 | Purpose-isolated deterministic generation | Same manifest produces the same canonical results across visit orders and worker schedules; adding cosmetic decoration does not move rivers, settlements or deposits. |
| WLD-02 | Planetary seam and pole traversal | Terrain, gravity, ownership, roads, drainage and saved edits remain continuous at all supported region-face seams and exceptional topology locations. |
| WLD-03 | Shared map, terrain and orbit facts | All representations use the same body parameters and canonical geography; changing camera scale does not create another mutable world. |
| WLD-04 | Coherent geology and connected water | The default fixture contains explained lithology, catchments, outlets, strata, springs and actual volumetric openings; refinement preserves macro boundaries. |
| WLD-05 | High-precision frame round trip | At large system coordinates, frame transforms preserve declared player-scale tolerance, rotation and point velocity without early f32 truncation. |
| WLD-06 | Operation identity through retries and compaction | Duplicate requests, reconnect, journal compaction and expired receipt detail never repeat a successful cost, reward, transfer or damage event. |
| WLD-07 | Crash injection at transaction boundaries | Interrupt before/after activation, persistence and receipt; restore one coherent allowed state and correct Applied-versus-Durable semantics. |
| WLD-08 | Stale derived publication | Old mesh, collision, navigation, light or proxy results cannot overwrite newer relevant source projections; unrelated inventory changes do not force full rebuilds. |
| WLD-09 | Mutable overlays and changed generators | A world upgrade preserves installed geography, sites, identities, recorded knowledge and player edits; caches can be rebuilt without rewriting canon. |
| WLD-10 | Indexed resume after long play | Saved entry reads a current indexed state plus bounded suffix, not full history or global erosion; genuine collision-ready first interaction is measured. |

## REN — Rendering, memory, assets, and streaming

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| REN-01 | Default-preset freeze reproduction | Capture the actual failing timeline and isolate CPU work, GPU work, memory pressure, waits, pipeline compilation or driver causes before claiming a fix. |
| REN-02 | Whole-job peak admission | Saturate producers and stall GPU completion; descriptors, ready outputs, staging and retirements plateau within declared credits without deadlock. |
| REN-03 | Cancellation and stale completion | Cancel before/after encoding, supersede a region repeatedly, and reuse handles; no premature reclaim, leaked staging, dropped completion or double release. |
| REN-04 | Minimize, resize and device epoch | Rapid resize/minimize and simulated device recovery keep authority state intact; transitions are bounded, zero-size surfaces handled, and cleanup makes progress. |
| REN-05 | Native budget pressure and pool backing | Lower available budget during replacement; optional admission stops, real backing or reuse is demonstrated, and essential correct geometry remains available. |
| REN-06 | Coarse-fine seams and edited holes | Traverse edge/corner transitions with caves, fine railings, bridges and destroyed landmarks; no sky leaks, filled tunnels, overlap flicker or restored distant tower. |
| REN-07 | LOD publication frontier | Delay arbitrary child uploads and empty results; the published frontier keeps complete valid coverage, bounded transitions and no unbounded neighbor-refinement cascade. |
| REN-08 | Occlusion after cover removal | Remove a wall, move occluders and turn rapidly at non-power-of-two resolutions; compare with unculled output and detect no false hidden actor or stale cover. |
| REN-09 | Shadow and depth correctness | Near/far casters, moving fragments, foliage and interiors use correct depth/bias/material behavior without broad black patches or missing geometry. |
| REN-10 | Art identity at actual camera scales | Matched native day/night/motion captures show coherent silhouettes, pixel clusters, distinct materials, grouped plants, quiet spaces and accessible original architecture. |
| REN-11 | Fast travel and turnaround | Move at supported speed toward an uncached site and reverse; collision/readiness deadlines, upload age and memory remain bounded without missing ground. |
| REN-12 | Persistent surface complexity | Compare solid, hollow, checkerboard, perforated and material-fragmented regions at equal voxel count; measure real output/collision/memory costs and preserve supported geometry. |

## NET — Authority, local physics, prediction, and deterministic domains

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| NET-01 | Cross-machine command replay | On the declared compatibility profile, accepted commands reproduce complete canonical state across machines, worker counts and render rates without routine corrective state traffic. |
| NET-02 | Complete checkpoint continuation | Restore terrain, solver/cache, controller, RNG, material and attachment state; continued hashes match an uninterrupted run, not merely object poses. |
| NET-03 | A deliberately stalled client | Host and other peers continue; the stalled peer exhausts bounded prediction then recovers or receives an explicit readiness result, without global waiting. |
| NET-04 | Missing commands and sealed ticks | Reorder/drop accepted frames and no-event ranges; confirmed time never passes a gap, axes follow bounded hold policy, and one-shot actions never repeat. |
| NET-05 | Immediate break and dependent movement | A predicted hole, selection ray, local motion and crate contacts use the same branch; authority rejection restores coherent dependencies without overwriting others edits. |
| NET-06 | Target generation contention | Two players mine/replace one cell; only one gets original loot and a late request cannot destroy a newly placed unrelated machine at the same coordinate. |
| NET-07 | Receipt before canonical patch | Receipt/data arrival orders never flash a wall back, spend provisional loot, duplicate sound, or discard unresolved dependent actions. |
| NET-08 | Baseline capture versus compaction | Race snapshot publication and suffix pinning with edits/compaction; every committed event remains represented exactly once. |
| NET-09 | Late baseline versus newer live state | Old epochs and same-epoch stale candidates cannot regress the installed frontier or overwrite newer geometry. |
| NET-10 | Bulk transfer saturation | First join, repair and content transfer under limited upload do not starve controls/receipts; queued bytes, age and processing cost are bounded. |
| NET-11 | Domain merge, split and mode transition | Before physical coupling, install a coherent merged baseline; splitting and fallback preserve contacts, momentum, frames, caches and ownership under tested rules. |
| NET-12 | Hidden information boundary | Portal interest and domain membership reveal only authorized facts; exact-mode convenience does not expose secret inventories or undiscovered private structures. |
| NET-13 | Changed-cover combat policy | Test accepted/rejected break-then-shot, replacement blocks, current new cover, moving doors, exhausted history and duplicate fire; authority selects one damage/cost outcome. |
| NET-14 | Impairment and full traffic accounting | Configured RTT, asymmetry, loss, burst loss, reorder, duplication and relay migration converge within declared limits; report complete traffic, state age, corrections and replay costs. |

## PHY — Coupled physical materials and bounded destruction

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| PHY-01 | One material owner through fracture | Cut a structure and collect fragments; original terrain, debris, salvage and inventory never own the same quantity twice. |
| PHY-02 | Load versus connectivity | Connected but overloaded construction can fail under the defined model; unknown unloaded support is not automatically detached or a certified anchor. |
| PHY-03 | Support deletion with cyclic and cross-brick bonds | Repeated cuts and moving attachments yield correct bounded component work, mass/inertia, point velocities, cargo and collider publication. |
| PHY-04 | Heated tank, powered hinge and floating crate | Breach the tank while inlet and obstacle move; real flow, pressure, heat and two-way impulses operate once within declared numerical tolerances. |
| PHY-05 | Fluid boundary activation/deactivation | Across active/inactive or adjacent patches, preserve mass/composition/heat/motion as supported; no invisible drain/wall, duplicated flux or camera-dependent disappearance. |
| PHY-06 | Freeze, cable support and melt | Freeze flowing water around a cable, load/cut it and melt it; latent heat, fluid/solid ownership, tension and collision remain coherent. |
| PHY-07 | Grains on belt, settling and reload | Transport, settle, save, reopen and excavate material without a body per grain, duplicated quantity or reset provenance. |
| PHY-08 | Fast projectile and rotating thin cover | Swept contacts use the correct revised geometry for both moving participants; render approximation does not grant tunneling or phantom obstruction. |
| PHY-09 | Dense contact and wake amplification | Converge existing bodies, alter gravity near sleepers and drive joint loops; common physics stays correct with measured contacts/constraints and no private accuracy reduction. |
| PHY-10 | Already admitted impact under pressure | After projectile activation, saturate derived queues; reserved effects and costs occur once at the defined stage/tick rather than disappearing or being indefinitely delayed. |

## KIT — Ordinary tools, building and storage

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| KIT-01 | Tool-family functional loop | Actual mine/build/hoe/harvest/bucket/shear/fish/ignite/map/repair affordances use correct tags, feedback and authoritative results, not inventory icons only. |
| KIT-02 | Mixed-resolution construction | Place, rotate, remove and repair supported fine shapes on large cells; collision/selection/quantity and seam behavior agree. |
| KIT-03 | Blueprint, mirror and creative distinctions | Ghosts show real fit/resources; survival consumes once, creative sources are labeled, undo respects ownership, and construction does not steal neighbouring structures. |
| KIT-04 | Stack, sort and consolidate | Repeated sorting/stacking preserves lot compatibility, quality, identity/quantity and locked/favourite slots across saves and concurrent edits. |
| KIT-05 | Storage scanner permissions and movement | Incremental index follows container moves, locks, range/connections and destruction without full-world scans or access to private containers. |
| KIT-06 | Concurrent last-item crafting/withdrawal | Exactly one valid transaction acquires the final input; duplicate ingredients/overflow/partial capacity are handled without partial unrecorded consumption. |
| KIT-07 | Comfort without late-game gates | A fresh survival fixture provides useful storage, shelter, food/light and simple work without compulsory boss kills, slaughter, AFK or advanced-chip fabrication. |

## IND — Scientific industry, electricity and control

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| IND-01 | Material recovery and process lots | Alternate ore routes change recovery/purity/byproducts while conserving represented feedstock and inputs; reload does not reroll yield. |
| IND-02 | Circuit and storage limits | Closed DC network, resistance/heating, source/provider/port limits and capacitor behavior agree with the declared model; large capacity does not grant unlimited power. |
| IND-03 | Conversion and thermal loops | Converters, electrolysis/fuel-cell or equivalent loop, pumps, heat exchange and cooling cannot create free energy through missing losses or erased heat. |
| IND-04 | Interrupted or blocked production | Stop power, fill output, change a valve and save mid-stage; quantities and partial state remain coherent with useful player-facing diagnoses. |
| IND-05 | Deterministic wake scheduling | Two networks wake one consumer, reused entities and canceled events; no duplicate activation/transfer and no unbounded stale scheduler storage. |
| IND-06 | Dormant factory oracle comparison | Coarse/batched advancement matches the defined tick model through thresholds and interruptions; broken logistics and blocked outputs do not produce offline goods. |
| IND-07 | Functional multilayer IC | Two distinct layouts show different real IO/memory/control/throughput behavior with test-bench diagnostics; design save, manufacture and installation preserve function. |
| IND-08 | Complete useful product chain | Prospect or salvage, process, fabricate a controller, power/automate a pump and improve a place; statistics expose actual throughput, losses, quality and bottlenecks. |

## MAG — Magic, living transformations and personal threat

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| MAG-01 | Wand compiler limits and order | Deck order/shuffle, triggers, cost, delay and modifier composition behave as declared; recursive expansion is bounded and errors are understandable. |
| MAG-02 | Earthfold physical closure | Donor material becomes constrained panels with actual voids/collision, force limits and occupant handling; payload activation follows real supported closure state. |
| MAG-03 | Mining spell exclusive yield destination | Hammerhand/additional heads and output modifiers perform paid work; debris/store/transmute/XP paths cannot duplicate the same resource. |
| MAG-04 | Cultivated creature and size change | An actual organism gains supported behavior, anatomy/collider/mass or explicit magic-source changes and persistent identity, not only a scaled visual. |
| MAG-05 | Dark spell anchor and alternatives | A restricted technique uses discoverable dependencies and effective counters; consent and legitimate alternatives matter, not just a purple damage type. |
| MAG-06 | Unreturned bounded knowledge | Only allowed observations/anchors influence behavior; reload and networking preserve limits, respite and ownership without omniscient inventory counters. |
| MAG-07 | Unreturned resolution and safe home | Supported investigative/resolution path works; wards and uninvolved co-op players remain protected, with no fake chat or offscreen storage deletion. |
| MAG-08 | First-person equipment quality | Original held wand/tool has useful grip, scale, animation, cast origin and coherent world/icon appearance; native use is reviewed rather than inferred from Blender output. |

## LIFE — People, ecology, institutions and real consignments

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| LIFE-01 | Coarse-to-detailed resident and cargo | Repeated activation of adjacent districts refines one household/journey/consignment, never duplicates stock or overlaps abstract and physical work. |
| LIFE-02 | Friendly daily life and habitat | Different supported body plans can reach home/work/services; organisms follow habitat and ordinary routines without meal-by-meal micromanagement. |
| LIFE-03 | Wizard compound alternate access | Public work, restricted rooms, service routes and actual containment exist; research/trade/service/infiltration have distinct consequences and alarms do not instantly fail the site. |
| LIFE-04 | City growth and player ownership | New construction respects plots, resources, access and player edits; no overnight overwrite or visit-order reroll. |
| LIFE-05 | Radio and scheduled delivery | Broadcast/map information has a real source/window/uncertainty and authorized destination; missed windows have supported alternatives rather than permanent ordinary lockout. |
| LIFE-06 | Consignment custody through transport | Warehouse, carrier, coarse travel, pod, dock and recipient transfer one recorded cargo identity; disconnect/reload cannot award it twice. |
| LIFE-07 | Peaceful settlement viability | Hospitality, trade, restoration, gardens and specialist work create real benefits; prosperity does not mandate endless escalating raids or city-government chores. |

## EXP — Exploration, regional composition and systemic dungeons

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| EXP-01 | Complete first journey and return | Stream/embankment/gallery loop offers visible invitation, several approaches, evidence, optional action/understanding, quiet discovery and a meaningful return. |
| EXP-02 | Player-created alternate route | Digging, building, gliding or another supported bypass discovers a coherent site and valid clues rather than broken scripting or blanket immunity. |
| EXP-03 | Physical puzzle consistency | At least one pressure/gravity/false-wall/fold interaction uses real state and multiple solutions with legible consequences, not hidden matching-key flags. |
| EXP-04 | Knowledge provenance and reconstruction | Observed/reported/inferred/tested records retain source/time; clues and old maps do not become omniscient exact markers or reroll on visit order. |
| EXP-05 | Quiet exploration and accessible guidance | A view/person/specimen is worthwhile without obligatory loot/combat; optional guidance supports players who cannot rely on subtle sound or color. |
| EXP-06 | Boss and enemy readability | Distinct roles, telegraphs, recovery, paths and actual counterplay work alone and in co-op; damage persists and no instant input-reading defeats clever methods. |
| EXP-07 | Farming and restoration persistence | Supported ecosystem/ritual resets have real costs/population/provenance; repaired routes and player changes remain useful without duplicate experience/material. |

## POR — Realm topology and immersive portal correctness

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| POR-01 | Multi-family realm policy | Universe family, realm, body and frame remain separate; direct requests cannot forge discovery/private permissions or restricted cross-family travel. |
| POR-02 | Crimson pocket isolation | Local travel and valid paired return work; ordinary flight or ordinary forged portals cannot connect different Crimson planet pockets. |
| POR-03 | Null Choir revised rule | An ordinary electrical machine continues functioning while a sustained magical binding attenuates; material fire and unsupported magical structures follow distinct rules. |
| POR-04 | Static arbitrary portal orientation | Position, direction, orientation and handedness map correctly for nonparallel apertures and different gravity frames. |
| POR-05 | Moving endpoints and point velocity | Rotating/translating source and destination preserve intended portal-relative motion, angular state and explicit energy accounting without giant accidental orbital impulses. |
| POR-06 | Native visible doorway and mid-crossing view | Parallax, near-plane behavior, occlusion and clipped actors work from oblique angles and halfway through, with no black frame, fake screenshot or source fog contamination. |
| POR-07 | Destination not ready or unauthorized | Pending aperture is visibly blocked; preview sends no private data and traversal waits only for that valid transfer rather than pausing every player. |
| POR-08 | Opposite crossings, disconnect and replay | Simultaneous travelers and retries at the exact transfer tick produce one entity/cargo owner each and a consistent save/reload result. |
| POR-09 | Nested portal and coupled-effect limits | Recursion/render-target/interest budgets are bounded; closed-while-occupied and unsupported ropes/fluid/networks follow explicit policies rather than deletion or hidden fake continuity. |
| POR-10 | Remote edits and players through a portal | Two clients see actual destination movement and current terrain; source/destination view histories and authoritative interest remain coherent during editing and recovery. |

## SHP — Voxel spacecraft, pressure, orbit and space building

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| SHP-01 | Creative category and editable starter | Spacecraft & Space Building contains functional parts/blueprints; Kestrel can be edited and built from the same survival-compatible kit. |
| SHP-02 | Assembly boundary and stable identity | Launching/staging/landing never swaps to an unrelated prefab, carries the hangar, duplicates hulls or loses module/cargo ownership. |
| SHP-03 | Airtightness actual geometry | Thin walls, corners, windows, partial shapes and fine/coarse joints produce correct seal/leak diagnostics; a missing canopy corner cannot be concealed by a seat bubble. |
| SHP-04 | Compartment split, merge and equalization | Hatches, breaches, docking and moving volumes conserve gas/enthalpy under the model; unknown topology never certifies safety and closing a door does not refill a room. |
| SHP-05 | Life support and thermal state | Separate pressure/composition, scrubber, power, heat and supplies persist during travel/unload; breach is repairable and a suit enables declared contingency behavior. |
| SHP-06 | Aero, actuator and load diagnostics | Wings require usable airflow/exposure, actuators and finite power; mass/fuel/cargo changes affect steering; vacuum-only craft need no decorative wing blocks. |
| SHP-07 | Real propulsion and guidance | Installed engines consume finite actual supplies and guidance uses available control authority; insufficient capability is diagnosed rather than corrected by free forces. |
| SHP-08 | Altitude versus orbit and round trip | A returning arc and stable orbit are distinguished; launch, insertion, rendezvous, deorbit, entry and landing use one consistent configured model without unlabeled cheats. |
| SHP-09 | Walking passengers on moving ship | Clients can walk/drop objects during translation, rotation, acceleration and correction with coherent collision time and no double deck velocity or lost passengers. |
| SHP-10 | Dock, service, disconnect and depart | Compatible capture/permissions/seals/equalization precede opening; both vessel identities and all inventory survive repeat docking and restart. |
| SHP-11 | Station construction and skyhook fit | A built station needs supported orbital state; tether/elevator/transfer geometry has explicit eligibility, load/clearance and schedule rather than a generic hovering prop. |
| SHP-12 | First useful orbital return and vehicle portal | A real platform mission returns useful cargo to the same workshop; a supported fitting ship can transfer through a legal portal with crew/compartments intact and no Crimson loophole. |

## HALL — Crimson Hall, wardens and enduring progression

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| HALL-01 | Original hanging machine-city composition | Approach/interior/reverse/distant native views establish scale, voids, original structures and useful space; no generic castle or copied finale geometry. |
| HALL-02 | Independent route families | Foundry, vertical machinery/choir and underworks offer distinct routes and systemic choices; damaged paths and player construction remain coherent. |
| HALL-03 | Warden functional counterplay | Each completed warden has readable combat and at least one real environmental/engineering intervention tied to functioning machinery, not just a switch counter. |
| HALL-04 | Any two of four progression | All valid orders and co-op joins persist correctly; remaining wardens affect the King through declared systems and no client can forge the shared state. |
| HALL-05 | King combat and realignment | Fair telegraphs, actual hit volumes, dynamic routes, player-built interventions and persistence work without arbitrary immunity or deleting the players tools. |
| HALL-06 | Extraction and reconnect | Multiple supported exit/rescue paths preserve participants through disconnect and failure; pre-victory return is available and ordinary possessions are not wiped. |
| HALL-07 | Charted nexus and replay-safe farming | Successful extraction unlocks only authorized discovered edges; return visits/reset costs/rewards preserve identity, avoid duplicate loot and do not erase the whole location. |

## MIX — Whole-game acceptance and release evidence

| ID | Fixture / review | Acceptance result required |
|---|---|---|
| MIX-01 | Four simultaneous player intentions | Explorer, factory operator, fighter/builder and joining player share the actual inhabited world under impairment and forced relay with bounded state age and queues. |
| MIX-02 | Long-run memory and recovery | Travel, edits, plants, scripts, saves, frames, portals and vehicles stop growing retained state after expected steady demand; post-burst debt drains under continued normal use. |
| MIX-03 | Reduced visual quality and accessibility | Lower effects/resolution/detail preserve collision, hazards, clues, cover, material distinction and operable UI; color/sound-only dependencies have alternatives. |
| MIX-04 | Current world at every representation | Globe, far skyline, near terrain, interior, vehicle frame, portal view and reload agree on consequential edits and identities through extended play. |
| MIX-05 | Honest finished-slice delivery | Provide original code/assets, executable native procedures, observed failures, measured comparisons and remaining scope; no synthetic screenshot or unrun test presented as proof. |


# 25. Required deliverables and session reporting

## 25.1 Native implementation package

For the selected milestone, deliver actual modified source in the existing engine, original editable assets and reproducible imports where applicable, validated data definitions, focused automated tests, integration fixtures, save compatibility/migration evidence, and reproducible native interaction/capture/profiling procedures.

Prefer existing tracking locations. Suitable records, when equivalent files do not already exist, are a short implementation status, decision log, test/evidence manifest, and measured benchmark report. Do not create a bureaucracy of nearly identical roadmaps. Keep one requirement-to-code-to-evidence mapping.

An evidence record should contain:

```
requirement_or_test_id:
status: NOT_RUN | BLOCKED | FAIL | PASS
build_commit_and_owned_dirty_changes:
world_content_simulation_manifest:
fixture_seed_save_and_initial_state:
hardware_backend_settings_network_conditions:
command_or_manual_steps:
expected_result:
observed_result:
trace_capture_log_paths:
measurement_scope_and_cache_state:
remaining_limits_or_followup:
```

Every milestone report separates:

- **Implemented:** real changed behavior and source/asset paths.
- **Verified:** tests actually executed, native actions actually performed, and evidence locations.
- **Measured:** before/after values with matching conditions and the measurement's scope.
- **Not verified / blocked / incomplete:** unrun native features, environmental issues, known failures, stubs, and later content.
- **Next dependency:** the specific next unfinished gate, not a new grand redesign.

No fabricated screenshots, test counts, Git commits, profiler traces, benchmarks, discovered source APIs, or claims that the whole game is finished. When a compiler/display/tool is unavailable, check that condition and report it; do not infer it from an earlier session. Work on the safe executable portions that are available.

## 25.2 Quality gate against superficial completion

A new type is not a system. A registered recipe is not a useful production chain. A generated chamber is not an interesting dungeon. A button is not working UI. A transform interpolation is not relevant local physics. A sky sphere is not a planet. A viewport texture is not a traversable portal. A rigid box with a cockpit icon is not an editable airtight spacecraft. A static crane prop is not working logistics. A text description of a wand is not an authored viewmodel.

Require the smallest meaningful player journey through the feature, including a failure case, persistence, and an interaction with an existing system. Preserve performance and presentation during that journey, not only at its start.

Where a target is not yet achievable in the current slice, leave a clear bounded unfinished status rather than false implementation. Keep only interfaces genuinely required by the next dependency; do not pre-fill the game with dozens of promised empty systems.

## 25.3 Execution instruction

Begin now with the real repository audit and reproduction. Reuse verified systems, preserve unrelated work, and implement the earliest blocked complete slice. Validate it in the native game and with the relevant acceptance cases. Maintain the unified authority, conservation, visual style, and region continuity throughout. Advance to the next gate only after evidence supports the current one.

**The end product is a living editable world where a player can build a warm home, invent useful machines and magic, investigate layered places, construct and fly the same real spacecraft, and return to a landscape that remembers what they did. Every engine improvement must help that game exist.**

---

# 26. Source context, precedence, and traceability

## 26.1 What was consolidated

This directive synthesizes the full visible conversation, its supplied visual references and attached specifications, the relevant earlier project context retrieved for this task, and the design requirements in the source groups below. It is not an audit of the game repository and does not claim access to every conversation or every prior file in the user's account.

| Source group | Role in this consolidation | Status / treatment |
|---|---|---|
| Current conversation: planets, parallel Crimson universe, Hall, dimensions, Earthsea-style magic, visual references, spacecraft, exploration | Explicit product intent and latest refinements | Incorporated throughout; user intentions distinguished from assistant-proposed illustrative content |
| `AXIOM_INHABITED_PALIMPSEST_ART_DIRECTION.md` | Core visual identity, historical layers, regional composition, player construction, Orchard fixture | Full relevant document reviewed; integrated in sections 6–8 and 20 |
| `AXIOM_ART_DIRECTION_REFINEMENT_02.md` | Engineered civic scale, ships, docks, functional industry | Full relevant document reviewed; integrated in sections 7, 11, 17, 20 |
| `AXIOM_ART_DIRECTION_REFINEMENT_03_EXPLORATION.md` | Expedition landscapes, knowledge, regional examples, return journey | Full relevant document reviewed; integrated in sections 13–15 and 20 |
| `AXIOM_MODULAR_SPACECRAFT_SPEC.md` | Editable vehicles/category, smallest design, pressure, flight, progression, docking | Full relevant document reviewed; controls section 17 with explicit earlier-glider reconciliation |
| Crimson prototype `ARCHITECTURE.md`, `IMPLEMENTATION_PROMPT.md`, `ACCEPTANCE_TESTS.md` | Original topology, moving portal, Hall progression and prototype tests | Reviewed as design references; old Null Choir/eager planet scope/readiness wording superseded; prototype execution is not asserted |
| `V8_COMBINED_MASTER.md` | Broader product, canonical systems, biology, science, mystery, foundations, previous status leads | Reviewed; preserved compatible scope, superseded older networking execution policy |
| `README_CANON_AND_PRIORITY.md` (V8.1 context) | Default geology, early glider/elastic tool, homeworld-first emphasis and compatible fixture continuity | Reviewed; folded into decision register and world/traversal delivery |
| `AXIOM_Deterministic_Voxel_Engine_Optimization_Prompt.txt` | Latest technical requirements for actual client physics, exact closed domains, prediction, receipts, loading, CPU/GPU work, memory safety and evidence | Reviewed; controlling technical refinement, not proof of achieved performance; external API/version statements not copied as current guarantees |
| `AXIOM_Reference_Worldgen_And_Bounded_Physics_Prompt.txt` | Default geography, local physics, physical material coupling, action admission and bounded destruction | Relevant substantive sections reviewed and consolidated; no claim of reproducing its cited research experiments |
| `AXIOM_Planetary_Simulation_And_Coop_Architecture.txt` | Spherical ownership, seeded histories, cities, institutions, conserved journeys and scalable residents | Relevant substantive sections reviewed and consolidated; existing engine topology preferred over a dependency-driven rewrite |
| `AXIOM_Science_Spaceflight_And_Planetary_Expansion.txt` | Capability graph, meaningful process choices, functional chips, planetary catalog, return-home value | Relevant substantive sections reviewed and consolidated; catalog explicitly later content, not initial deliverable count |
| Relevant earlier project context on storage, mining/fold spells, coding workflow and performance | Workshop usability, exclusive yield handling, original spell examples, source ownership and verification | Retained as design constraints/proposals; unverified source status remains a lead, not a fact |

## 26.2 Reference-image use

The companion handoff package includes the supplied visual references under `reference_art/`, with an `INDEX.md` and a content-hash manifest. Identical uploads are stored once, with their original aliases retained in the manifest. These are reference/analysis materials, not licensed production game assets. Do not copy their protected characters, logos, textures, or geometry into the game.

The supplied image sets include graphic caped-traveller/equipment compositions, bright forest/water, snowy architecture, immense ruined civic forms, engineered spacecraft/docks/cities, red-water excavation landscapes, mountain expeditions, facility/ridge views, and receding rings beneath extraordinary celestial scenery.

Use them as an art-direction board with responsibilities described in section 7, not as images to paste into production. When the repository includes the original uploaded filenames or a reference folder, map assets by their actual identity and retain a local reference manifest. Do not invent external artwork licenses or assume a filename exists in the repository because it was present in this chat's workspace.

## 26.3 Authority of this document

For future implementation sessions this master is the consolidated baseline. Older design files remain useful provenance; they do not revive explicitly superseded behavior. Keep future user changes as short versioned amendments with a traceable decision and affected tests. Do not generate another contradictory master by concatenating every historical draft verbatim.

Technical values, class names, example place names, part proportions, performance objectives, and proposed method choices are adjustable where labeled or where a better compatible existing implementation is verified. Ownership, correctness, useful gameplay, native evidence, and the explicit settled product decisions are not optional just because they are expensive.

**Prepared outcome:** a complete implementation directive and an unexecuted acceptance catalog. No game source was modified or engine benchmark performed while preparing this document.
