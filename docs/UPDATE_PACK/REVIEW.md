# Critical review and decisions — R1

**Scope:** Editorial, gameplay, architecture, execution and document-consistency review of the supplied discussion and retrieved design sources. This is not a source-code audit or a completed playtest. The package deliberately retains unresolved hypotheses instead of declaring every problem solved.

## 1. What was actually reviewed

The full 1,494-line `AXIOM_COMPLETE_IMPLEMENTATION_PROMPT.md` consolidated directive was read, including its 122 acceptance clauses. It is the most specific cross-chat baseline retrieved for this task. The entire 355-line V8.4 universe addendum was read. Relevant V8.3 geological/hydrological/ecological/guidance passages, the visible conversation and its uploaded shader text, and the latest supplied screenshot informed the review. Earlier V8–V8.2 ideas were considered through this conversation and their consolidation; this is **not** a claim that every original attachment or every past chat was independently reopened.

The latest screenshot is retained as `reference/FAILURE_REFERENCE.png`. It documents visual symptoms, not the code producing them or the target quality. The actual game checkout, current shaders, executable, performance and native tools were not inspected or run during this design task.

Source-derived requirements and new decisions are distinguished below. `reference/SOURCES.md` identifies technical reference documents checked during the review. The inherited acceptance text is preserved unchanged in `reference/upstream_cases.json` and `ACCEPTANCE.json`; mapping a clause into a later task does not claim it already works.

## 2. The central editorial correction

Earlier drafts accumulated responsibilities faster than they established choices. “A groundbreaking voxel world with all of these influences” is an ambition, not a useful implementation order. A coder asked to be botanist, physicist, animator, writer, renderer author, network architect and critic simultaneously is likely to implement thin approximations of all of them.

The revised identity is narrower than the feature list: **a persistent world that is enjoyable to inhabit, understand and alter**. The product retains industry, spellcraft, ecology, archaeology, celestial travel and unsettling exceptional places. But the execution unit is one useful proof in the existing game. Source fidelity, readable art and basic pleasure in moving/building are not postponed until the simulation is complete.

No new grand location catalog, invented civilization, named cosmic material or new mandatory starting region was added. Old names remain compatible content candidates or existing repository facts to verify. The package is a reviewed production reference, not another fictional universe replacing the previous one.

## 3. Design flaws corrected or explicitly constrained

| Review ID | Weakness in earlier advice or recurring temptation | R1 decision and reason |
|---|---|---|
| R01 | “Nothing can ever be boring or repetitive.” | Reject as an acceptance promise. Familiarity, dependable routines and repeated motifs support mastery and attachment. Test stale decisions and coercive grind instead of maximizing surprise. |
| R02 | Every session must produce three stories; every site three routes. | Use as playtest questions, not quotas. A peaceful building session and a simple useful place are valid. Routes should arise from topology and capabilities. |
| R03 | A spectacular landmark is the remedy for a dull world. | Improve ordinary ground, movement, habitat, construction and social use first. Exceptional sites remain authored exceptions, not uniform scatter. |
| R04 | Every interesting valley conceals a failed ancient machine. | Preserve nature, mundane history, competent present societies, cultural disagreement and genuine unknowns. The Work does not explain away every tradition or spirit. |
| R05 | Serious tone requires permanent decline and misery. | Consequences coexist with comfort, rebuilding, play, thriving communities and understated humor. No requirement that every inhabitant need rescue. |
| R06 | Scientific fidelity requires the player to do technical homework. | Useful presets and instruments reveal cause/effect. Advanced engineering is optional depth; fabrication time and electronic timing are deliberately abstracted. |
| R07 | Exploration must equal a factory's ingots/hour. | Preserve factory throughput. Give exploration new possibilities and reasonable ordinary supplies. No explorer-only currency or arbitrary human-only observation flag. |
| R08 | Machines may be automated, but they stop when the player leaves. | Use validated offscreen production with real inputs, output limits and interruptions. AFK proximity should not outperform doing something enjoyable elsewhere. |
| R09 | Punishment proves survival depth. | Food, exposure and death policies are explicit trials. No default maximum-health death spiral, late-game storage gate, continuous sprint tax or unavoidable loss of long-term tools. |
| R10 | Complete settlement simulation is prerequisite to a believable villager. | Start with an actual service, home, route and supply record. Summarize distant activity. Do not simulate every historical person or require a full market equilibrium. |
| R11 | Humane play should receive virtue points. | Provide practical non-cruel routes and useful living creatures. No universal morality meter or golem slaughter as ordinary metal necessity. |
| R12 | Regional biomass is conserved as a closed scalar. | Plants use explicit environmental inputs and produce outputs under the declared model. Solar energy, water, carbon/nutrients, harvest and respiration cannot be conflated. Promotion itself must not create stock. |
| R13 | More anatomy fields automatically make better creatures. | Author readable roles and bodies; add properties when an interaction consumes them. Avoid random body-part assembly and simulation invisible to the player. |
| R14 | One huge materials matrix with a prescribed reaction percentage ensures depth. | Sparse resolved rules plus a deliberate inert default. Measure useful interactions, not nonzero table entries. |
| R15 | Millions of spell sequences prove Noita-quality depth. | Validate order, resource costs, observable behavior diversity, readable combinations and useful environmental applications. Combinatorial counts are not a fun metric. |
| R16 | One VM should simulate every physical process. | A shared bounded command vocabulary can connect devices and magic. Specialized solvers own fluid, electrical, biological and mechanical state. Separate player interfaces retain personality. |
| R17 | Every challenge must resist mining, flight and powerful spells. | Let mastery solve old obstacles. Particular constraints need readable sources and costs. No universal anti-dig/anti-flight field to protect a scripted route. |
| R18 | More worlds cure repetition. | A useful homeworld and real orbital return precede expansion. Each new destination changes existing decisions and has internal variety, not one mandatory ore. |
| R19 | Complete physical laws require only real sources and no fictional ones. | Explicit magical sources are legitimate. Accounting distinguishes physical supply, numerical losses, game abstraction and intentional magical work. Don't dress an exception as measured science. |
| R20 | Removing external tutorials means hiding every recipe in mysteries. | Common essentials and device diagnostics are accessible in game. Optional rare interactions can reward investigation. Guidance describes known methods, not hidden generator truth. |

These changes are design judgments, not empirical proof of universal player preference. Trial balance must be tested in the actual game and changed when evidence contradicts it.

## 4. Engineering contradictions resolved

| Review ID | Dangerous shortcut | Corrected contract |
|---|---|---|
| R21 | “Every ray at the horizon must hit terrain.” | Rays through real caves, arches and sky must remain open. Coverage means an appropriate representation wherever authoritative geometry should be visible, not filling the background. |
| R22 | Inflate or raise fallback terrain to conceal gaps. | Preserve valid published geometry; repair ownership, topology, seams and revisions. An uncertainty bound is not permission to create material or close a tunnel. |
| R23 | A black shadow diagnostic proves missing terrain. | Separate geometry/depth, CSM, horizon occlusion and final radiance. Zero direct-sun visibility can be legitimate; a single still cannot diagnose its cause. |
| R24 | Replace `min(CSM, horizon)` with `max`, averaging or confidence weights. | Define caster/receiver coverage and uncertainty first. `min` can be valid for combined occlusion; `max` can remove real shadows. No unexplained “confidence-aware” blend fixes incompatible geometry. |
| R25 | Transvoxel is a universal stitching solution. | Its sampled-isosurface 2:1 case is specific. Cubic face ownership, heightfields and other surfaces require compatible boundary construction. Benchmark only applicable methods. |
| R26 | A 2D resident column bit owns all 3D space. | Heightfield domains may use it; overlapping bridge/cave/floating/editable volumes need correct interval/volume ownership. |
| R27 | Fade thin orbital cables to sky color while retaining opaque depth. | Physical subpixel coverage must compose against the actual scene/background; widened support cannot erase land, clouds or objects. Pipeline choice is a measured integration task. |
| R28 | Latest shader complexity equals latest visual quality. | Fix shapes, classification, source data, sampling and lighting domains first. The old shader's material-family collisions are investigation leads, not facts about the current checkout. |
| R29 | Every fern leaflet needs fine canonical voxels or a rigid body. | Stable organ/gameplay state with reusable render geometry, bounded physical proxies and explicit conversion when interaction requires it. |
| R30 | All CPU/GPU state must be bit-identical and float-free. | Prove deterministic generation and selected complete domains under a declared compatibility profile. Keep the supported floating numerical model and explicit state-sync fallback elsewhere. |
| R31 | Deterministic physics means all players wait for the slowest. | Host authority advances accepted ticks. A slow client exhausts bounded prediction then repairs independently. Replicas must receive all required inputs and full relevant state. |
| R32 | Shared seed is secure hidden-world synchronization. | Seed reconstruction saves bandwidth but permits datamining. Player-facing discovery and private mutable data protection are different contracts. |
| R33 | Positions/velocities are a complete physics checkpoint. | Include future-affecting contacts, caches, RNG, controllers, topology, solver parameters, attachments and boundary inputs as required by the backend/domain. Prove continuation. |
| R34 | A large action can reserve all future causal consequences exactly. | Reserve a conservative supported immediate footprint and staged work model. Validate subsequent expansions before activation; don't silently lose paid effects. Truly unlimited aftermath cannot be guaranteed. |
| R35 | Fixed finite queues guarantee stable responsiveness. | Also bound task size, installation/retirement, service fairness and oldest state age. Show recovery under ongoing activity and after bursts. |
| R36 | Rendering fewer objects frees VRAM. | Track actual backing, pools, staging, overlap and retirement. Account for native budgets when available without inventing a portable free-memory API. |
| R37 | A heat pump producing more heat than electrical input violates conservation. | Account for heat extracted from its environment plus electrical work. The closed conversion-loop test must include every source and sink. |
| R38 | Connectivity proves structural strength. | Connectivity and a declared load/strength model are separate. Unknown unloaded support is neither detached nor a certified anchor. |
| R39 | A flow transport algorithm is a complete fluid/pressure model. | Specify behavior and coupling; do not claim pressure/buoyancy/blast fidelity from an unrelated transport kernel or smoke effect. |
| R40 | Fluid boundaries can freeze when offscreen without consequence. | Keep matched flux/control volumes or a validated reduced model. Unloaded neighbors are not infinite drains or automatic walls. |
| R41 | Saving a long journal makes loading safe. | Indexed coherent snapshots plus bounded retained suffix; test compaction, crash boundaries and late network snapshot installation. |
| R42 | A portal's visible destination is ready for crossing. | Collision, ownership, permission, state frontier and required assets must be ready. Pending destinations remain visibly blocked, without pausing unrelated players. |
| R43 | A portal conserves energy merely by rotating velocity. | Moving/separated endpoints can change inertial energy. Account for explicit portal work and point-frame velocity. |
| R44 | A shuttle with wings and a high altitude has reached orbit. | Use one gravity/air/propulsion model, distinguish ballistic altitude from orbit, require an actual useful return. Blueprint dimensions are not proven flight configurations. |

### Two corrections applied to the task graph during this review

An encounter does not need a newly implemented hang glider before its attacks can become enjoyable. A moving vehicle does not need the entire advanced liquid solver before a seated rigid-assembly proof. Those overbroad dependencies were removed. The selected proof still needs real collision, ownership and the specific physical interfaces it uses.

Meridian's thin-tether **visual** proof and its **cargo-service** proof now have separate identifiers. A successful render cannot unlock a “second world with real transport” milestone. The same distinction separates a moving assembly from an orbital-return proof. This prevents partial milestones from silently satisfying full-system gates.

## 5. Codex execution flaws removed

| Review ID | Earlier prompt habit | Replacement |
|---|---|---|
| R45 | Reload all old bibles before every edit. | Short durable instructions, concise session state, a section router and one current work card. Sources remain available without occupying every session. |
| R46 | Give the agent a dozen expert roles and demand the best game ever. | Name the selected player benefit, actual paths, allowed boundaries, evidence and stop condition. No adjective is accepted as an implementation mechanism. |
| R47 | “Never stop until everything is complete.” | Finish/revise the selected proof; stop at its acceptance boundary or an honest blocker. An endless task is not responsible autonomy. |
| R48 | Spend every turn generating a new master plan. | The master is stable; necessary changes are short explicit amendments. No renamed starting valley or repeated architecture rewrite. |
| R49 | Independent critic means the author's own praise. | Record actual adversarial questions and executed checks. A separate reviewer is optional when available; no invented independent review. Human visual/gameplay judgement remains necessary. |
| R50 | Repeated full tests always increase confidence. | Run relevant regression checks and broader suites when warranted. Repeat after meaningful changes or contradictory evidence, not automatically on identical bytes. |
| R51 | Strict lines-of-code/public-API/page quotas equal quality. | Review cohesion, interfaces, code ownership, measured cost and tests. A number can focus scope but cannot certify design. |
| R52 | Documentation can assume Windows/Blender/game access. | Verify tools and actual commands. Preserve executable source and precise blocked status when native evidence is unavailable. |
| R53 | One failing unrelated suite authorizes cleanup of another agent's code. | Classify ownership and whether the failure invalidates the selected proof. Do not overwrite shared work to manufacture a green report. |
| R54 | Choosing a model name implies known limits and special flags. | Respect the user's selected Codex/Astra Light configuration; no invented context size, switches or performance guarantees. |

The package follows official Astra/Codex guidance on progressive disclosure and concise persistent instructions, but the content is a project-specific proposal. It has not been A/B tested against other prompts on the user's actual selected Light configuration.

## 6. Scope and canon preserved rather than silently rewritten

The newest retrieved directive preserves actual local physics on relevant clients and favors command replication for proven complete domains. Older snapshot-only wording is therefore not used to erase that objective. State synchronization remains an explicit safety/compatibility mode, not a claim that exact replication is already achieved.

The approximately 60% self-built atmosphere-exit pacing goal remains an **authoring hypothesis**. In an open capability graph it cannot be a meaningful universal runtime research percentage. Players may buy passage or repair technology earlier where designed. Early gliding is retained.

Crimson planet-associated pocket isolation, the exceptional Hall routing, sustained-magic-only Null Choir behavior, multiple realm families, meaningful ship pressure/flight/docking, working Meridian cargo, and the eventual any-two-of-four Hall challenge are preserved. Their names do not authorize placeholder registries, mandatory early completion, or a rewrite of existing world coordinates.

Unified accessible storage, same-owner inventories/materials, practical presets, and user-designed electronics remain. The latest simplified AC/RMS abstraction with phase ignored takes precedence over the older draft proposing a richer phase/reactive model. A more advanced electrical solver would be a deliberate later decision, not implied by calling a block a transformer.

The declared scientific abstractions do not negate magitech. A fictional energy source or supernatural relationship can be legitimate when its operating rules, costs and consequences are clear. Cultural meaning should not be reduced to an instrument reading merely because some effects are measurable.

## 7. What still cannot be settled by proofreading

**Visual quality:** The forest, ground, roofs, mountains and held-item framing need actual assets and captures. A language model cannot confer “professional quality” on an unrendered mesh. Review silhouettes and materials at walking scale as well as aerial vistas.

**Gameplay balance:** Trial food, recovery, movement, storage range/stack size, resource availability and manufacturing cadence need playtests. They are deliberately not immutable code constants.

**Performance:** Minimum hardware, native working set, supported destruction/contact complexity, region streaming latency and exact-domain cost are unknown until measured. No promise of 120 FPS or superiority over Teardown follows from this document.

**Algorithm choice:** Current storage, meshing, shaders, rigid backend, transport and importer may already solve portions well. The prompt cannot prescribe a replacement before inspecting the repository.

**Determinism:** Cross-machine command replay, checkpoint restoration, boundary merges and hidden inputs must be proven per compatibility profile. Backend marketing or unit tests on an isolated kernel are insufficient.

**Economy and ecology:** Even a conserved simulation can be tedious, unstable or unfair. Remote operation must not become mandatory maintenance; abundance must not erase curiosity; self-sustaining communities must not require the player to babysit them.

**Production scope:** The long-term design is large. Completion of one strong home-to-expedition loop is evidence to expand, not proof that all named planets/realms/encounters are affordable. Stop or cut a feature when its pilot fails its stated player benefit.

## 8. Verification performed by this package

`tools/validate_spec.py` checks document sections, task/proof references, dependency cycles, retention of all 122 source clauses, acceptance status discipline, and local links. `tools/test_validate_spec.py` tests the checker against intentionally invalid small examples. File hashes and ZIP integrity protect the delivered artifact bytes.

These are **document/package checks**. They do not compile Rust, validate the game's WGSL, run Blender, simulate physics, launch the game, measure FPS, prove visual quality, or establish player enjoyment. All game acceptance cases remain `NOT_RUN` in this handoff. A source clause retained verbatim is traceable, not automatically endorsed as already achieved or mathematically sufficient.
