# THE LIVING ENGINE — MASTER GAME-SYSTEMS SPECIFICATION v2.0
**Project:** Existing Rust + raw-WGPU variable-resolution voxel game  
**Status:** Design and implementation contract, **not a claim of repository inspection or completed code**  
**Date:** 2026-10-22  
**Read with:** `01_SCIENCE_INDUSTRY_AND_CHIP_DESIGN_V2.md`, `02_DUNGEONS_AND_IMMERSIVE_SIM_V2.md`, `03_CLAUDE_CODE_IMPLEMENTATION_PROMPTS_V2.md`

## Executive decision
The goal is not “GregTech with more recipes” or “a more detailed dungeon generator.” It is a **single physical, systemic sci-magic world** in which the player discovers and reconstructs scientific capability, engineers original machinery, builds reliable electrical and material networks, designs useful computers, explores ruins of a lost technological civilization, and can solve problems by construction, destruction, automation, magic, stealth, diplomacy where implemented, and experiments.

**The distinctive gameplay loop** is:

> Encounter a constraint → inspect it → form a hypothesis → modify physical apparatus or control logic → measure the outcome → record/reproduce the successful design → exploit the new capability in the world.

The value of a machine or processor lies in what it **does**. The value of a dungeon lies in what the player can **change**. A technology is worth implementing when it creates new meaningful choices, not when it simply adds items to a recipe ladder.

### Hard constraints from the earlier specifications
- Existing Rust, raw `wgpu`, existing engine and site/expedition systems. **No Bevy**, second world engine, redundant renderer, bespoke scripting engine, or generalized full SPICE/CFD/atomistic simulator as a prerequisite.
- Time **abstracted for gameplay and education**: preserve real units and cause/effect; do not require waiting real wafer-fab times, timing nanosecond transitions, or manually driving process stages with arbitrary countdowns. Advanced CPU timing is a *compiled characterization*, not a requirement to emulate every transistor clock cycle in-world.
- All useful energy, matter, charge, inventory ownership, and world changes are authoritative, persistent, and conserved to the declared model tolerance. Presentation cannot decide reality.
- Preserve old saves and previous machine semantics through explicit migration; don't silently duplicate or erase energy, charge, items, claims, bosses, or structural edits.
- Reuse verified existing machines (reported batteries/induction matrix/biomass burner/assembler/cables/transformer), process screens, physics, creature rigs, Astral Sanctum/Undertow Knot systems, and terrain edits where possible. The reports are **investigation leads**, not evidence of current repository state.
- Avoid stepping on concurrent shader, horizon, water, UI and terrain work. **One owner per shared code path**, isolated worktrees for parallel agents, one designated integrator.

## The five pillars of the product

1. **Construction as engineering.** A player's arbitrary-sized multiblock becomes a compiled model whose material, geometry, capacities, ports and process internals determine function. “Larger” does not always mean “faster.”
2. **Factories as connected systems.** DC/AC power, shafts, heat, pressure, chemical processing, item belts, pipes, trains/storage and controls interact through consistent contracts, each solved with the appropriate inexpensive abstraction.
3. **Electronics as invention.** Players can design a microcontroller from functional blocks, route nets across layers, characterize a chip, manufacture wafer lots, test/bin packaged parts, save versions and deploy hardware that changes automation capabilities.
4. **Worlds with functioning ruins.** Dungeon machines, lighting, barriers and hazards have real state; players can remove power, feed alternate power, dig, reroute material, divert enemies, repurpose salvaged electronics, or apply magic subject to its own limits.
5. **Multiscale scalability.** Physical voxels author topology; compact graphs/segments/lot records carry simulation; event schedules sleep stable systems; renderer instantiates visible detail from authoritative snapshots. Unloaded areas retain coherent logical state.

## Critical design corrections (decisions, not suggestions)

| Temptation | Adopt instead | Why |
|---|---|---|
| One universal grid where pixels directly become arbitrary working transistors | Three-stage editor: *functional block architecture* → *verified physical macros and routing* → *optional expert mask/device editor* | Otherwise arbitrary layouts would be impossible to validate or become a shallow fake |
| Processor “quality = grid occupancy × node tier” | Workload-specific latency, throughput, memory, I/O, power, reliability and manufacturing cost derived from validated architecture and physical constraints | Supports genuinely different chips and avoids a single meta-layout |
| Process node literally doubles every property | Fixed die size yields more **digital placement capacity**, with nonuniform scaling of analog, I/O, voltage and SRAM | Scientifically defensible and preserves mature-node usefulness |
| Every metal layer is another layer of computation | Interconnect layers and vias connect logic; true active 3D die stacking is a separate late technology | Physical correctness |
| AC as DC with a sine-wave icon | Explicit DC, AC RMS and converters; transformer is AC device; DC/DC uses switching-converter averaged behavior | No impossible DC transformer; meaningful long-range voltage economics |
| Every placed block is an updateable simulator node | Compile voxel geometry into runs, edges, volumes, segments and machine components | Enables enormous factories without CPU proportional to raw block count |
| Arbitrary physics solves each tick | Bounded model families, derived analytic results, event wakeups, adaptive coarse integration | Determinism, speed and comprehensible diagnostics |
| A cut power cable is a scripted dungeon gate | Power network state changes actual lights, machines, doors and hazards, with legitimate alternatives | Systemic immersive sim rather than puzzle theater |
| All dungeons become invulnerable when a bypass is found | Ordinary editable voxel materials and explicit protection where justified by fiction, cost and clarity | Preserves freedom to improvise |
| “All complete” means unit tests pass | Actual build, headless integration, runtime walkthrough/screenshots, save/load, profiler traces and player-visible explanation | Avoids visually weak or nonfunctional implementations |

## Formal boundaries and data ownership

```
Voxels/World State (authoritative mutable occupancy, edits, persistence)
 ├─ Structure compiler → Stable multiblock machine identities and revisions
 ├─ Electrical compiler → Stable terminals, conductor runs, AC/DC islands
 ├─ Logistic compiler → Belt runs, hydraulic segments, storage/ports
 ├─ Spatial gameplay → Navigation/collision, sites, mechanisms, enemies
 └─ Render adapter → Read-only draw/instance/light state snapshots

Industrial authority (one owner of each quantity)
 ├─ Source/storage energy & thermal enthalpy
 ├─ Reaction/process batches, mass composition, work progress
 ├─ Transactional inventories and transfers
 ├─ Digital board/firmware state
 └─ Characterized chip design + immutable mask/revision library

World events → topology invalidation → bounded compile/solve
            → staged resource reservation → consistent commit
            → change notifications → rendering / instruments
```

**State ownership rule:** An inventory lot, battery charge, item drop, boss drop, claimed reward, chip die and machine-internal material has **exactly one authoritative owner** at any instant. Temporary compiled objects reference ownership; they are not themselves duplicating persistent energy or loot.

**Stable identity:** Distinguish world object ID, structure ID, network ID, chip-design revision, fabrication-lot ID, render instance ID, and cache generation ID. Never use a temporary graph index or chunk-local mesh handle as a persistent item/energy identity. A structural edit invalidates caches and functional validity; it does not automatically erase material inside surviving vessels.

**Physical scale:** All machine geometry derives from actual world-unit dimensions and actual volumes, not count of voxels, because the renderer/world supports variable voxel resolution.

**Explicit typed ports:** Electric (DC polarity or AC phase/conductor topology), fluid (substance domain, pressure), mechanical (shaft), heat, cargo, signal, and mana use typed port descriptors. Converters bridge domains with simultaneous debit/credit accounting. A cable, machine and anchor all expose stable terminal IDs.

**Single simulation time:** All live-world processes advance from the authoritative elapsed simulation time. UI animations never change Joule consumption, recipe throughput, capacitor discharge, enemy attack windows or objectives. Laboratory replay may stretch displayed time only in isolated previews.

## Multiscale detail-of-simulation policy

| Scale | Simulation | Rendering | Runtime contract |
|---|---|---|---|
| Placed conductor voxels | Per-run resistance, thermal capacity, faults; explicit supply/return | Mesh instances, overlays | Recompile touched runs only |
| Non-voxel transmission spans | Few conductor branches + mechanical support limits | Catenary meshes by LOD | Anchor destruction disconnects instantly |
| Factory machine shell | Derived vessel/chamber geometry and a few thermal/flow zones | Detailed multiblock voxel mesh | No per-shell-voxel thermal solver |
| Belt items | Packed ordered lanes, gaps, aggregate movement | GPU instanced visible items | Stable run advances with O(1)-like shared offset between handoffs |
| Fluids in pipes | Control volumes and hydraulic edges, limited axial segments as needed | Flow indicators/contained fluids | No per-pipe-voxel CFD |
| Wafer and IC | Immutable masks/layout + lot statistics/process state | 2.5D viewer/cross-section | Do not voxelize nm geometry in 3D world |
| Embedded CPU | Functional firmware/macros, event-driven jobs, characterized capacity | Module UI, package object | No millions of cycles/tick |
| Active dungeon enemy | Deterministic intent/action states, nearby physics/collision | Animated rigid-part pose | Attacks obey world edits/collision and attack timing |
| Distant unloaded factory | Persisted/logical state, scheduled thresholds, bounded coarse integration | None | Cannot output resources with frozen input/energy |

## Shared numerical and transactional rules

- Joules and watts are separate; energy is the time integral of delivered power. A converter produces <= efficiency × input, with the difference accounted as heat or other declared losses.
- Thermal state is stored energy/enthalpy, not arbitrary temperature that can be reset without consequences. Reactive chemistry records balanced mass/species and reaction heat within a calibrated simplified process.
- Electricity reference ground is a numerical choice, **not a magical return path**. Physical conductors, ports, switch state and topology determine whether sustained current flows.
- DC has source impedance and current limits, nonlinear loads bounded near low voltage, optional capacitance state, and documented solver convergence policy. AC has RMS voltage, frequency, active/reactive demand, line impedance, phase configuration and rated transformers. DC/DC and AC/DC bridges are coupled accounting boundaries.
- Transfers are reserved then committed across source/destination under one deterministic ordering. Refund/release reservations on failed operation. Never count the same stored Joule or cargo item twice.
- Cache and published output carry world/network revision. Stale worker results are discarded. World-edit effects (terrain collision, electrical severance, objective state) become authoritative before the next affected gameplay step.
- All approximate models declare their validity range. If an unsupported configuration is built, expose **unsupported/out-of-range**, not invented physics.
- Nonconvergent electrical/hydraulic calculation must not commit invented outputs. Preserve a last valid state if safe, or fail closed and notify the player.

## Player experience: short loops and deep optimization

**First hour:** Simple circuits, a meter, safe paired-conductor cables, crude mechanical work, a useful electric heater/motor, a foundry or refining experiment; explanations describe what changed and why. No need to hand-build a separate return cable for every ordinary machine.

**Early electronics:** Telegraph, relay logic, resistors, RC experiment, crude detectors/diodes and transistors, vacuum-tube demonstrations as optional history. A compact working board automatically controls a furnace or sorter.

**Grid age:** AC alternator, credible transformer with windings/core/cooling, overhead lines and substations, DC/DC converter and rectifier. Players measure line loss and justify stepping voltage up for long distance.

**IC age:** Simple validated modules on a floorplan; auto-route initial design; measure timing, memory bandwidth, I/O, thermal ceiling and power. Save version 1, improve version 2, fabricate repeatable wafer lots with patterned photoresist/etch/deposition/doping, test/bin, deploy.

**Late industry:** Better process capabilities, crystal growth, vacuum, controlled atmosphere, polishing, optics, metrology, cleanroom, automated logistics, robot systems, custom accelerators, chiplets, radio/satellite and precision scientific instruments.

**World narrative:** Ruined electrical grids, abandoned laboratories, star-glass archives and orbital relics hold valuable salvage. Player can restore, reroute or hijack real infrastructure. Nonindustrial magic is another authored resource with measurable effects and bounded costs, not a substitute for electrical accounting.

## Tech progression is a capability graph

Maintain **capabilities with dependencies and alternatives**, not only recipe gates. Historical exhibits should work, but need not be compulsory. Proposed interconnected branches:

- Earth/stone/wood/mining → metalworking → casting, bearings and tools → furnaces, shafts, pumps, precision machining.
- Chemical separation/purification + mechanics/heat → glass, ceramics, electrolytes, reagent handling, chemical processing and waste recovery.
- Induction/electromagnets → motors, alternators, transformer cores, grid protection, distribution and electric instruments.
- High-purity materials + crystal growth + optics + controlled surfaces/vacuum + metrology → semiconductor processes.
- Analog amplification/relay control → programmable electronics → automated instruments → lithography control and robotics → better chip quality.
- Mechanical structure + propulsion + communications + advanced control → aerospace, satellite, tether/skyhook and orbital infrastructure.
- Magical ecology → specialized field sensing/actuation and hybrid materials, interfacing to the ordinary utility/logistics systems through explicit converters.

Found or salvaged equipment may leapfrog a dependency **for use** without magically granting the ability to mass-produce it. Reverse engineering, repair and substitution enable creative alternate routes.

## Priority ordering — no Big Bang rewrite

**Foundation F0 — Repo reconnaissance and baseline.** Verify architecture, save format, machine code, active changes and existing tests; produce evidence-linked map and a reproducible benchmark. No feature claims based on memory.

**Industrial E1 — Closed energy loop.** Readable DC source/return, wires, loss and instrument; one existing machine consumes *delivered* energy once. Build/split test, storage ownership, save/load.

**Industrial E2 — Logistics.** Compile belts into transport runs, aggregate fluid pipes, implement transactional lot movement on one automated line. Bench active and idle factory before content expansion.

**Industrial E3 — Meaningful multiblock.** One variable-sized furnace/reactor with derived geometry, useful bottleneck, heating, feed/exhaust and diagnostics; no bespoke giant machine catalog.

**Industrial E4 — AC and overhead distribution.** RMS grid/realistic transformer, converter boundaries, line supports and non-voxel spans, outages and repair in a test scene. Do not overspecify detailed AC dynamics up front.

**Electronics H1 — Grid-based *working controller*.** Functional module layout, typed nets, routing, registers/memory/timer/GPIO, characterized workload, saved revisions. One machine behaves differently depending on chosen valid chip.

**Electronics H2 — Manufacturing real layout designs.** Verified cell templates, layout layers, wafer process plan, lot states, inspection, repeatable masks and binning. First working IC end-to-end; optional transistor/pixel editor later.

**World D1 — Three-space astral benchmark.** Architecture, restrained material hierarchy, working local lighting, Undertow Knot states and route readability. Match gameplay camera and collider.

**World D2 — Three creature roles.** Warden, moth, crab with honest hit geometry, distinguishable silhouettes/motion, positional counters and useful rewards.

**World D3 — Systemic first site.** Two routes, one recoverable mistake, one terrain edit, objective state persistence, and optional actual power isolation only when shared network interface exists.

**Integration I1 — Horizontal playtest.** A salvaged control module or power line can change one legitimate dungeon mechanism; preserve manual/terrain alternative. Test with physics, power, terrain collision, optics/lighting, save/load and low graphics preset.

**Optimization I2 — Representative factory and dungeon stress suite.** Profile before optimizing. Continue targeted fixes; do not claim a machine capacity or fps objective not measured.

## Integration rule between feature tracks

The dungeon must **not** wait for the entire AC grid, semiconductor fab or industrial rewrite. A small `MechanismPowerState` adapter can initially connect to existing authoritative machine supply. When the full electrical network lands, replace the adapter behind its stable port contract without rewriting dungeons.

Likewise, the industrial track must not replace terrain/generator/horizon shaders or the dungeon site generator to add a cable. Add hooks to existing world-edit and network-registration pathways. Rendering API changes are proposed to one designated owner.

**Example systemic shared puzzle:** A derelict archive's generator powers confinement, lights and a service lift. Sever the line and confinement drops, but lighting and lift also fail. Rewire an alternate supply, explore a maintenance branch, use ordinary building/digging, lure an enemy, or fight. Ensure claims depend on actual objective policy, not fabricated guardian deaths. The same generator and switch mechanics must work in normal factories.

## Reproducibility and visual verification

For every playable feature, deliver:
1. What existed before (grounded in inspected code), what changed, and exact ownership paths.
2. Minimal interactive steps a player performs, plus before/after screenshots or video where possible.
3. Headless unit/property/integration tests, especially conservation, revision invalidation and migrations.
4. Measured frame/update/compile/solve/memory costs with machine hardware, scene and settings stated. Report baseline and delta, not extrapolated promises.
5. Visual proof separately from build/test success. If runtime cannot be run, mark it unverified and give reproduction steps.
6. Honest residual risks, unsupported configurations and next smallest useful slice.

Aspirational synthetic stress profiles (not performance claims): 100k conveyor blocks, 1m represented cargo items, 10k machines, 100k pipe blocks, many electrical islands, repeated localized edits, and a visually complex room with active enemies/projectiles and terrain changes. Begin smaller and scale measurement gradually. No performance target can justify deleting gameplay logic or faking charge/resources.

## Guardrails for Claude Code and multiple agents

Before touching code: read repo instructions (`CLAUDE.md`, `AGENTS.md`, cargo workspace and other relevant guidance), `git status`, current branch, `Cargo.lock`, active processes and tests. Inspect specific modules rather than assuming names from this document. Assign ownership of files for each agent. Run overlapping code changes only in isolated worktrees/branches with an integrator and explicit shared API. Do not reset or clean unrelated edits, alter unknown user changes, weaken tests, make broad shader rewrites, or silently replace existing saves. Make commits only for the agent's verified work and only when repository policy/user authorization permits; never auto-push/merge unknown branches.

Every agent should follow **RECON → DESIGN → IMPLEMENT → TEST → DEMONSTRATE → MEASURE → REPORT**. Do not stop at writing a plan if a scoped playable implementation is feasible. Do not claim a renderer was tested unless it was launched and captured.

## Source and status notes

**Source briefs supplied by user:** `voxel_science_electronics_implementation_brief(1).txt` and `voxel_dungeon_quality_overhaul(3).txt`. This v2 respects the former's deliberately abstracted time, ownership and energy invariants and the latter's exact Astral Sanctum, Undertow Knot, Warden, Aether Moth, crab, phantom-floor and legacy claim continuity. This v2 **extends** them with grid-based chip design, AC/non-voxel transmission, logistics/multiblock compilers, interoperable dungeon infrastructure and finer implementation contracts.

**Real-world reference starting points** (use for inspiration, not a claim that the game implements these): Factorio FFF 176 belts https://www.factorio.com/blog/post/fff-176 ; FFF 416/430 fluids https://www.factorio.com/blog/post/fff-416 and https://www.factorio.com/blog/post/fff-430 ; OpenROAD https://github.com/The-OpenROAD-Project/OpenROAD ; SKY130 process rules https://skywater-pdk.readthedocs.io/en/main/rules.html ; ASML lithography primer https://www.asml.com/en/technology/all-about-microchips/how-microchips-are-made ; Computer History Museum Silicon Engine https://www.computerhistory.org/siliconengine/timeline/ . Verify licenses/dependencies before incorporating any third-party code or data.

**Evidence disclaimer:** These documents have **not** inspected your current game repository, loaded a save or run the game. All numerical workload sizes are test-scene designs, not achieved performance. Existing code facts mentioned from earlier conversations are marked for verification.
