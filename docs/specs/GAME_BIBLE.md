# THE GAME BIBLE AND PLAN-GENERATION PROTOCOL

This file does two things the previous specs did not. Part A is the design bible: the world, its cosmology, its technologies and magics, and the explicit, testable claims for how this game is better than its references. Part B is the protocol by which the agent turns the bible plus the engine specs (`ENGINE_CORE_PROMPT_v2.md`, `MOD_ADDENDUM.md`, `ADDENDUM_B_HARDWARE.md`) into implementation plans that are verified before any code is written. A spec cannot contain a finished architecture for a game this size; what it can contain is the vision, the constraints, and a procedure that forces the architecture to be produced, reviewed, and proven piece by piece.

---

# PART A — DESIGN BIBLE

## A.0 Working title and premise

**Working title:** *Underkingdom* (placeholder; the agent proposes five alternatives with the plan).

**Premise.** The world is a single planet whose crust is a palimpsest: every civilization that rose on it dug deeper than the last, and each left its machines, its magics, and its mistakes in the strata. The surface is young and green. The deep is old and remembers. The player is a *delver*: someone who goes down for salvage and comes back up changed. Technology and magic are two surviving dialects of one older discipline that the deepest civilization called simply *the Work*. Learning either dialect is learning half of a language; fluency comes from both. The planet's core is not rock. What it is, the player discovers.

The game's thesis, which every system should express: **matter has rules, rules can be learned, and learning them is power.** There is no mana you cannot explain and no machine that works by fiat. Secrets are *physical*: hidden in reaction tables, in geology, in the way light falls through a crystal at a particular season.

## A.1 Reference analysis — what each source does that we must match or beat

Each reference below has a *lesson* (what we take), a *flaw* (what we fix), and *claims* (testable statements of superiority that go into the acceptance suite). Claims are the contract.

### Heavily modded Minecraft (tech + magic + Tinkers)
Lesson: the joy of an endless recipe graph; machines as spatial objects; magic with its own progression; building as self-expression; the modpack as a world.
Flaw: systems don't talk to each other (a Thaumcraft node ignores a Mekanism reactor); physics is absent so nothing has consequences; mods duplicate (five ore doublers); tooltips and books are the game; balance is arbitrary (RF numbers).
Claims:
- C-MC1: Every energy form converts into every other through physical devices with losses (heat → kinetic → electric → mana → heat); a scenario builds a closed loop and measures efficiency < 100% by conservation test.
- C-MC2: Any machine can be damaged by any physical hazard (fire, flood, collapse, overvoltage, radiation, acid) and the damage is visible and repairable; a scenario applies each hazard to each machine class.
- C-MC3: Tool/armor properties are derived from materials, not authored per item; adding a new material with 10 properties produces usable tools, weapons, and armor with no new code (test: a RON-only PR adds "orichalcum" and the validator proves full coverage).
- C-MC4: There is exactly one implementation per concept (ore processing, storage, transport, power, teleport); the feature matrix maps all mod features to the shared mechanisms in `MOD_ADDENDUM.md` A.15, and the lint fails on a second implementation.
- C-MC5: A new player can reach the first automated resource in 2 hours of play without reading external docs; measured by the text-layer playtest bot following only in-game hints.

### Noita (lore, worldbuilding, magic, secrets)
Lesson: the world is a chemistry set; the lore is told by the world, not by text dumps; gods and secrets are *mechanisms* (the Work, the alchemy recipes rolled per seed, the sky, the moon, the fungal caves); every material interacts; spells compose; death teaches; curiosity is rewarded with things nobody told you to look for.
Flaw: 2D; secrets are mostly fixed-position trivia; late game is wand-stat optimization; no building, no automation, no long-term base.
Claims:
- C-N1: Per-seed secret recipes exist for at least three transmutations (e.g., lively concoction / alchemic precursor analogues) and are discoverable by experimentation with in-world hints (texts, murals, NPC rumours) that are generated from the seed, not hand-placed. A scenario proves a bot can discover one from hints alone in bounded time.
- C-N2: Every material pair has a defined interaction (including "nothing") and the matrix is complete; the validator enumerates all pairs and fails on undefined ones; ≥ 30% of pairs have non-trivial reactions.
- C-N3: The cosmology has at least six *physical* secrets (a place, a time, a condition) whose discovery changes world state (unlocks a dimension layer, changes weather, wakes a thing), each implemented as a rule, each findable without a wiki by at least one in-world hint chain, none placed at a fixed coordinate.
- C-N4: Wand/spell composition supports ≥ 10^6 distinct meaningful combinations (count of card sequences up to length 6 whose effect differs by a defined metric); emergent spell effects appear in a fuzz test that discovers ≥ 20 unauthored useful behaviours.
- C-N5: 3D destruction and fluid behaviour match Noita's feel at 60 tps on H1 hardware: a side-by-side capture of the same scenario (acid eating a floor, oil fire spreading, water pressure bursting) is reviewed each milestone.

### FTL
Lesson: a vessel as a system of systems; crew with jobs; resource scarcity; events with choices; the run structure; tension from information (what's behind the next jump).
Flaw: abstract space; no persistence; no building beyond upgrades.
Claims:
- C-F1: Vessels (airships, ships, rovers, rockets, submarines, trains) are rigid bodies with subsystems (power, life support, propulsion, hull, sensors, weapons, shields) that fail independently and are crewed by mobs with jobs; a scenario breaches a hull and the crew responds (seal, repair, evacuate) via utility AI with no scripting.
- C-F2: "Jumps" exist as an exploration structure: expedition mode where a vessel moves between generated sites (deep caves, sky islands, orbit, other worlds) with event rules (encounters, hazards, trades) generated from world state and seed; choices alter world state.
- C-F3: A permadeath "delve" run mode exists, built entirely from the persistent world's rules, with meta-progression recorded in the codex and the world (your previous bodies are found, your previous bases are ruins).

### Zork (worldbuilding, not text interaction)
Lesson: the Great Underground Empire — a dead civilization whose ruins have *internal logic*: a currency (zorkmids), a religion, a bureaucracy, comedy, an economy you reconstruct from fragments; places that are memorable because they are specific (the Flood Control Dam #3, the Loud Room, the Round Room); puzzles whose solutions are the world's physics.
Flaw: puzzles with single solutions; parser as the barrier.
Claims:
- C-Z1: At least three deep civilizations exist, each with a generated-but-coherent culture: naming language (phonotactic generator), architecture templates, technology/magic dialect, religion with a *mechanism* (what their temples actually do), economy (what they valued and why), downfall (a physical event that is still observable in the strata).
- C-Z2: Named places are generated with specificity: landmarks get names, histories, and a functional role (a dam that still holds water; a loud room that is acoustically loud because of geometry — reverb from nav summaries; a round room that is a mechanism hub). A scenario asserts ≥ 1 named landmark with functional role per 500 m of underground travel.
- C-Z3: Puzzles are physical and multi-solution: every "locked" progression gate (flooded passage, collapsed hall, heat barrier, sealed vault) can be solved by at least two of {tech, magic, brute physics}; a test suite solves each gate three ways.
- C-Z4: The text layer renders any place as a Zork-quality room description generated from planes and lore tags (light, sound, smell, temperature, materials, landmarks, history); reviewed for quality per milestone against a corpus of hand-written exemplars.

### Neal Stephenson (Snow Crash, The Diamond Age, Cryptonomicon, Anathem, Seveneves, Fall; the Baroque Cycle's natural philosophy)
Lesson: ideas as the spectacle; technologies with social consequences; the *matter compiler* and the Feed (nanotech as infrastructure); the Primer (an adaptive book that teaches); the Metaverse (a built world inside the world); language as code and code as magic (Snow Crash's nam-shub, the Sumerian "me"); Anathem's concents (knowledge preserved across millennia by discipline), the Hylaean Theoric World (mathematics as a real place), causal domains; Seveneves' orbital engineering with real physics and deep time; Cryptonomicon's cryptography and information as treasure; the Baroque Cycle's alchemy becoming chemistry.
Flaw: novels, not games — but the lesson is that *systems with intellectual content* can be the attraction.
Claims:
- C-S1 (**The Primer**): the education profile and the codex are one in-world artifact: an adaptive book found early that teaches the Work by posing experiments in the player's own surroundings, tracks what the player has demonstrated (not read), and unlocks deeper chapters on demonstrated mastery. Lessons are scenario-asserted (the book knows you built a working siphon because the sim says so). This is the single most important feature for the student path and for onboarding.
- C-S2 (**Matter compilers / the Feed**): the endgame of the tech dialect is *programmable matter*: compilers that assemble blocks/items from elemental feedstock delivered by a Feed network (a transport type), with throughput limited by energy and by the information content of the object (bits per block from a compression estimate of the template) — making design, not grinding, the late-game constraint. Illicit compilers, Feed theft, and Seed-style distributed compilers are gameplay.
- C-S3 (**Language is power**): the magic dialect's deepest tier is a *spoken/written language of the Work* whose grammar is the controller VM's instruction set exposed as glyph sequences (Hexcasting-style); runes are literally programs; "nam-shubs" are programs that affect mobs' utility weights; linguistic viruses are a late-game hazard and weapon. Tech and magic therefore literally share a compiler.
- C-S4 (**Concents / deep time**): knowledge persistence is a system: libraries, archives, and monastic orders (NPC factions) preserve recipes, maps, and lore across world "ages"; a catastrophe mechanic (eruption, impact, flood) can end an age and the player's knowledge survives only if archived physically; new-game-plus inherits archives found in the new world's strata.
- C-S5 (**Orbital engineering**): the space layer uses real-enough orbital mechanics (patched conics, integer elements, transfer windows) and structural engineering (tether stress, spin gravity in the body model, radiation and thermal balance); building a stable habitat is an engineering problem with failure modes, not a checklist.
- C-S6 (**Cryptography as a mechanic**): the signal network supports encrypted channels, keys as items, codebreaking as a machine/spell process with real (toy-but-honest) ciphers; lore fragments and faction communications are encrypted; the Primer teaches the basics.
- C-S7 (**Causal domains / instanced worlds**): pocket worlds, the Metaverse analogue (a built virtual space inside the world running on in-world computers), and dimensions all share one mechanism: a world with its own physics constants and a defined interface to the parent; nested at least two levels deep in a test.

## A.2 Cosmology and lore spine (what is literally true in the world)

Keep this fixed; generators elaborate within it. Lore is delivered by *things*, not text: strata, machines, murals, the Primer's experiments, NPC rumour, archives.

1. **The Work.** The underlying discipline: matter obeys rules; rules can be written. The oldest civilization (the *Founders*, a placeholder name the naming generator replaces per seed with a phonotactic family) could write rules directly into matter. Their artifacts are *rule-bearing materials*: cells whose reaction rows are nonstandard. Finding one is finding a new verb.
2. **Three deep civilizations**, from shallow to deep, each a stratum, each with a generated name, culture, dialect, religion-as-mechanism, economy, and downfall:
   - **The Engineers (upper strata, ~200–600 m):** industrial, steam-and-steel, built rail, dams, foundries, cities in caverns. Downfall: a cascade failure — their deepest mine broke into something. Their ruins are *working infrastructure* if you restore power.
   - **The Choristers (middle strata, ~600–1500 m):** the magic dialect; cathedral-caves with acoustics that matter (resonance is their mana source: standing waves in crystal geometry — the sim's pressure/vibration field); runes as liturgy; their golems still patrol. Downfall: a schism that weaponized resonance; shattered strata (ravines) are their war.
   - **The Founders (deep, > 1500 m):** the Work itself; matter compilers, the Feed, archives in crystalline memory (physical data storage the player can read with the right device). Downfall unknown; the deepest layer is sealed. The core is theirs.
3. **The surface peoples** (villages, trade, monastic archives — the concents) are descendants who remember fragments. Factions: Salvagers (tech), Cantors (magic), Archivists (knowledge; neutral; the Primer is theirs), the Crown (surface power; taxes delves), and a cult that worships the deep.
4. **The sky.** Day/night, moon phases, seasons, meteor seasons, and a visible orbital ring (Founder-built) that can be reached; the ring is the space layer's first destination and contains the Founders' records of the planet before the Work.
5. **Secrets (C-N3 minimum six)**, each a rule, not a coordinate; examples the generator realizes per seed:
   - A transmutation only possible under a specific moon phase and temperature band at a leyline well.
   - A door in the Chorister strata that opens to a sustained chord (a signal pattern on resonance cells).
   - A dam that, if drained, exposes a Founder antechamber (the dam is a real body of water in a real structure).
   - A plant that grows only in Founder light (a specific emissive material) and whose fruit teaches a verb.
   - A planet-wide event (eclipse) during which the ring is reachable by a cheaper path.
   - The core: what it is and how to reach it, gated by all three dialects.
6. **Ages.** Catastrophes end ages; archives persist; strata grow. New-game-plus places your previous world's ruins in the new world's upper strata (C-S4).

## A.3 The two dialects and their shared compiler

- **Tech dialect**: energy forms, machines, networks, logistics, chemistry, nuclear, orbital, programmable matter. Progression by *demonstration* (the Primer) and by *salvage* (Engineer/Founder artifacts).
- **Magic dialect**: mana (resonance-derived), aspects, wands, runes, rituals, constructs, transmutation, language of the Work. Progression by *experiment* (Noita), *salvage* (Chorister/Founder), and *attunement* (physical conditions).
- **Shared compiler**: the controller VM. Rune circuits, machine programs, drone behaviours, spell glyph chains, and Founder artifacts all compile to the same bytecode. The player sees two notations (glyphs vs. ladder/text) for one semantics; the reveal that they are the same is a mid-game story beat, after which cross-compilation (cast a machine program; wire a spell) unlocks the hybrid path. Design consequence: the VM is a core system, built in M1–M2, not a late add-on.

## A.4 Gameplay modes and the run structure

- **Persistent world** (default): survival/creative/education as specified.
- **Delve runs** (FTL/Noita): descend from a surface base with a vessel or on foot; permadeath for the character, persistence for the world; events generated from world state; meta-progression via the codex and physical ruins.
- **Expeditions** (FTL): a vessel jumps between generated sites; crew, subsystems, scarcity; integrates the space layer.
- **The Primer** (education): in-world, adaptive, demonstration-gated; also the tutorial for everyone.
- **Adventure modules**: Zork-style authored sites placed into generated worlds via the structure system; the text layer renders them.

## A.5 Beat chart (the intended first 40 hours; each beat maps to systems and claims)

1. Surface: needs, tools, first fire that needs oxygen (C-MC2 feel), first Primer experiment (siphon or lever) (C-S1).
2. First descent: an Engineer rail stub; restore a water wheel; first machine; first collapse teaches support (C-Z3).
3. Chorister echo: a resonant chamber; first wand; the Primer explains resonance as physics (C-N1 hint chain starts).
4. Base and automation: belts, furnace, farm; first trade with the Crown; first archive visit (C-S4).
5. The dam (Zork beat): flooded passage solvable by pump, by freezing, or by breaching (C-Z3 three ways).
6. Engineer city: restore power; drones; the mine that broke into something (foreshadow).
7. Chorister cathedral: rune liturgy; golems; the schism's ravines; the chord door (secret 2).
8. Vessel: build an airship or submarine; expedition structure opens (C-F1/F2).
9. The reveal: rune program and machine program decompile to the same bytecode (A.3); hybrid path unlocks.
10. Founder antechamber (behind the dam): matter compiler fragment; the Feed; language of the Work (C-S2/S3).
11. Ring: launch; orbital engineering; records of the planet (C-S5).
12. Age-ending threat: the thing in the mine; archive or lose; the core (C-S4; secret 6).

## A.6 Better-than scorecard (acceptance suite entry per claim)

Every claim C-* above becomes a scenario or a review item in `assets/scenarios/claims/`; `just claims` runs them and prints a scorecard. A milestone is not done until its assigned claims pass. The plan assigns each claim to a milestone and a system owner.

---

# PART B — PLAN-GENERATION PROTOCOL

The failure mode of long specs is that they produce long plans that are still vague. This protocol fixes that with structure, review, and proof. The agent does not write code until a system's plan passes the rubric.

## B.1 Inputs and outputs

Inputs: this bible; `ENGINE_CORE_PROMPT_v2.md`; `MOD_ADDENDUM.md`; `ADDENDUM_B_HARDWARE.md`; the repo (`CLAUDE.md`, `docs/`, ADRs); `docs/research/*` (produce first).

Outputs, in order:
1. `docs/research/` — one file per reference topic and one per mod family, each ≤ 1 page, each ending in "what we adopt / reject / improve."
2. `docs/MASTER_PLAN.md` — the system inventory (B.2), dependency graph, milestone assignment of every claim and every matrix feature, and the session schedule.
3. `docs/systems/<system>.md` — one *system design document* per system, in the fixed format (B.3), reviewed by the rubric (B.4) until it passes.
4. `docs/decisions/` — ADRs for every decision with > 1-week consequence.
5. Only then: implementation sessions per system, each with the exit criteria in the SDD.

## B.2 System inventory (minimum; the agent extends, never merges)

Core: det/fixed-point, coords/scale, brick storage/pool/awake, CA kernel framework, powder, liquid, gas, thermal, reactions/fire, support/collapse, light, weather/seasons, growth/biology, planes manager, actions/replay/desync, save/paging, budgets/governor, profiles/caps, overlay/telemetry, jobs/threads, audio.
Bodies: table, creation (floodfill), integration, contacts, resolve, fracture, fuse, joints, controllers (vessel/missile/quarry/train/drone/rover/airship/golem).
Mobs: table, movement, nav summaries, HPA, senses, utility AI, LOD, factions/reputation, villagers/jobs, taming/breeding, bosses, crew.
Machines: table, buckets, inventories, multiblocks, transport cells (belts/pipes/tubes/conduits), inserters, networks (electric, kinetic, signal, link, heat/steam/mana/neutron/radiation planes), generators, processing chains, augments/modifiers, digital storage, autocraft, builder/quarry, field emitters, forcefields, pocket worlds, matter compiler/Feed.
Magic: mana/aspects planes, wands/cards, projectiles, runes, rituals, alchemy/secret recipes, enchanting, constructs, language of the Work.
VM: bytecode, sandbox, I/O, glyph front end, text/node front ends, cryptography primitives.
World: worldgen stages (from WORLDGEN.md), civilizations generator (names, culture, architecture, religion-mechanism, economy, downfall), strata, structures, POIs, secrets generator, ages/catastrophes/archives, dimensions/physics constants, space layer/orbital, ring.
Player/game: player, needs, inventory, tools/parts/traits, crafting, cooking, farming, building variants/schematics, combat, vehicles/vessels, trade, advancements/quests, run modes (delve, expedition), Primer, codex, text layer/parser/descriptions, UI schemas, tooltips (style guide), accessibility, localization, modding overlays.
Rendering: GPU meshing, indirect/culling, shadows, volumes, water, sky/weather, light/emissive, particles, bodies instanced, far LOD, upscaling, post, materials (procedural/atlas), UI rendering, map/preview.
Tooling: scaffolds, validators, scenario recorder, replay bug reports, benches, class profiles, CI matrix, docs lint.

## B.3 System design document format (every section mandatory; "N/A" must be justified)

```
# <System>
1. Purpose — one paragraph; which pillars and which claims (C-*) it serves.
2. Scope — in/out; what other systems it depends on and which depend on it (link).
3. Data — every struct with field types, bit widths, byte size, GPU/CPU residency,
   lifetime, ownership; RON schemas with an example file; plane usage.
4. Algorithms — pseudocode for each operation; complexity; per-tick cost model
   (N awake × cost); determinism notes (hash use, ordering); failure modes.
5. API — the crate's public items (≤ 40) with signatures; Action variants emitted/consumed;
   events; what the text layer, UI, and VM can query.
6. Tick integration — which stage(s); budget fields; degradation behaviour by distance;
   sleep conditions.
7. Hardware — cost on H0/H1/H3 (estimated ms/tick, memory); ladder knobs it owns;
   GL/no-compute path.
8. Content — what designers write in RON to add things; scaffold command; validator rules.
9. Tests — unit, property, CPU≡GPU, conservation, replay, scenarios (named), golden images;
   claim scenarios it satisfies.
10. Bench — scenario names, targets per hardware class, what counts as regression.
11. Migration — save version impact; what existing code is deleted (LOC estimate).
12. Risks and open questions — each with a mitigation or an experiment (spike) and its cost.
13. Sessions — ordered implementation sessions with exit criteria and estimated LOC per session.
14. Alternatives rejected — at least two, with the reason, as a mini-ADR.
```

Length: 2–6 pages. More is a sign of unresolved design; less is a sign of hand-waving.

## B.4 Review rubric (run by a separate critic agent that did not write the SDD; must pass before implementation)

Score 0–2 each; all must be 2:
1. Every struct has sizes and residency; the memory total per hardware class is computed.
2. Every algorithm has a cost model and a determinism note; no floats in state.
3. Every dependency is on the dependency graph and acyclic.
4. Every claim C-* assigned to this system has a named scenario.
5. The GL/no-compute path is specified or the system is proven GPU-optional.
6. Degradation by distance and sleep are specified, with the overlay metric named.
7. Content is RON with a scaffold and validator rules; no new `match` on ids.
8. Tests cover: correctness, conservation (if matter/energy), replay, hardware classes.
9. Each session has an exit criterion that `just verify` can check.
10. Risks have experiments with costs; anything > 1-week consequence is an ADR.
11. Deletion: the SDD names what existing code it replaces.
12. The critic can find no sentence that is a wish rather than a mechanism ("should feel alive" → fail; "scent plane emitted by X at rate Y, consumed by Z" → pass).

Failing SDDs go back with the critic's list; three rounds maximum, then escalate to the human with the disagreement stated in ≤ 10 lines.

## B.5 Spikes (where the design is unknown, measure before deciding)

Required spikes before the SDDs that depend on them, each ≤ 1 session, each producing numbers and an ADR:
- S1 Brick CA on iGPU: 8³ vs 16³ bricks, workgroup 64 vs 128, shared-memory halo vs global reads; H1 emulation. Decides kernel shape.
- S2 Liquid model: pressure-column vs. iterative relaxation vs. SPH-lite; measure equalization speed, cost, artifacts on `u_tube`/`dam_break`.
- S3 Rendering: binary greedy mesh + indirect vs. brick ray-march; H1/H3; decides renderer.
- S4 CPU reference throughput: SIMD CA cells/s per core on x86-64-v2; decides H0 viability and awake caps.
- S5 Nav summaries: recompute cost per brick change; decides nav budget and mob caps.
- S6 VM: bytecode interpreter throughput and sandbox cost; decides instruction budgets and whether glyph compilation is on-line or offline.
- S7 Resonance field: can "mana from standing waves" be a cheap pressure/vibration plane with a few operators, or is it a scripted abstraction? (Lore depends on it; decide honestly.)
- S8 Matter compiler information metric: compression-based bits-per-template on real builds; decides C-S2 balance.
- S9 Orbital layer: integer patched conics stability over long runs; decides C-S5 scope.
- S10 Civilizations generator: a prototype that outputs names, a temple mechanism, and a downfall event for 10 seeds; human review for coherence; decides C-Z1 approach.

## B.6 Session discipline

- One system or one spike per session. Start by loading `CLAUDE.md`, the SDD, and the last session's exit report only.
- Subagents: research fan-out, kernel pairs (GPU+CPU), scenario writing, critic review. The critic never shares context with the author.
- Every session ends with `docs/sessions/<date>-<system>.md`: what changed, LOC delta, bench table by class, claims status, next session's exit criterion.
- `/compact` with the exit report as the focus; never carry raw file dumps across sessions.
- Escalate to the human only on: a rubric deadlock, an ADR with product consequence (what the core is; permadeath rules; naming), a spike whose numbers invalidate a milestone.

## B.7 First five sessions (fixed)

1. Research fan-out: reference analyses (A.1) and mod families; `docs/research/*`; naming proposals.
2. `docs/MASTER_PLAN.md`: inventory, dependency graph, claim and feature assignment to milestones, spike list ordered by what blocks the most SDDs.
3. Spikes S1, S4 (kernel shape, CPU throughput) — both block core SDDs.
4. SDDs: det/coords, brick storage, CA framework, actions/replay — through the critic.
5. SDDs: powder/liquid/gas/thermal/reactions — through the critic; then implementation begins.

The human reviews MASTER_PLAN.md after session 2 and the first passed SDDs after session 4. Nothing else needs approval until an escalation.