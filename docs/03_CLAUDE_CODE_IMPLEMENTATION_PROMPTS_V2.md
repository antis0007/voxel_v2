# CLAUDE CODE — EXECUTABLE PROMPT PACK v2.0
**For:** Claude Code working directly in the existing Rust + raw-`wgpu` variable-voxel game repository.  
**Purpose:** High-quality, bounded, playable implementation with clear code ownership and adversarial verification.  
**Companion docs:** `00_MASTER_DESIGN_AND_INTEGRATION_V2.md`, `01_SCIENCE_INDUSTRY_AND_CHIP_DESIGN_V2.md`, `02_DUNGEONS_AND_IMMERSIVE_SIM_V2.md`.

## How to use this pack

Place the four `.md` files in the repository (e.g., `docs/design/`), or supply their full paths to Claude Code. Start with **Prompt 00: repository reconnaissance**, then select *one* implementation prompt in its own session. Re-run the targeted audit at each handoff. The prompts are written to result in **actual code and playable vertical slices**, not indefinite speculative planning.

**Track dependency:** `00` precedes all; `01` can lead into `02`/`03`/`04`; `05` (chip editor) can begin after the event/controller/energy contract is established, using prepared chips; `06` (fab) follows `05`; `08`/`09` (dungeon art/enemies) can proceed in parallel in isolated worktrees; `10` (dungeon mechanisms) integrates with verified existing power or `01`/`04`, not every advanced fab feature. `11–13` are integration and verification stages. Assign exclusive writer ownership over shared source files first.

### UNIVERSAL EXECUTION CONTRACT — paste at the top of each implementation session, or keep in `CLAUDE.md`

> You are the senior systems engineer and adversarial integrator for a real, existing Rust + raw-wgpu voxel game. Your job is to ship **one small, complete, measurable, maintainable playable improvement**, not to produce a speculative architecture document. Follow the v2 design docs where they apply, but **inspect reality** first: read `CLAUDE.md` / `AGENTS.md`, root Cargo workspace/Cargo.lock, current branch, `git status --short`, relevant source/tests, save schema and known concurrent owners. Do not trust proposed file paths or old test counts. Distinguish: *reported*, *found in code*, *built*, *tested headlessly*, *seen on screen*, *played*, *measured*. Do not claim any higher level without evidence.
>
> Do not introduce Bevy, another renderer/ECS, another giant simulation framework, or a universal scripting engine. Preserve real SI units where claimed, physics/energy/material identities, active machine/save/world contracts, and performance. Preserve staged progression: abstract nanosecond timing and realistic fab wait duration for gameplay but retain measurement, operation order and physical consequences. No fake machine effects, no invisible unlimited power, no purely decorative wires substituted for actual networks.
>
> **Worktree safety:** Do not overwrite/reset/clean/stage unrelated user or agent changes. Inspect ongoing shared edits. Propose/claim exclusive file/module ownership for this slice. If shared-file ownership conflicts, avoid those files, negotiate a small interface with integrator, or work in an isolated worktree; never edit the same hot shader/generator files concurrently. Don't automatically merge or push; make scoped commits only if allowed and after tests. Do not change unrelated test tolerances or game settings to mask regressions.
>
> **Execution:** (1) Audit and record baseline. (2) Define invariants and minimal integration interfaces. (3) Implement a thin but complete vertical slice, including UI/world interaction and persistence. (4) Add adversarial automated tests; run `cargo fmt --check`, relevant test targets, Clippy as repo supports, and comprehensive tests if practical. (5) Launch app/play test/screenshots where environment allows; otherwise clearly state visual/runtime proof unavailable. (6) Profile relevant hot paths and compare baseline. (7) Critically review your own changes for bugs, misleading abstraction, safety, stale caches, high allocations and duplication. (8) Provide exact changed files, evidence, measurements, residual risks, and next slice. Use concise, readable Rust and existing conventions.
>
> **Stop rule:** Do not widen the scope to optional systems or hundreds of content assets until the declared acceptance gate is genuinely met. If blocked by missing infrastructure, implement the smallest working enabler with tests and an explicit handoff; no invented screenshots, benchmarks or capabilities.

### Required response format for each agent

```
BASELINE — branch, dirty files observed, current behavior, commands run
SCOPE/OWNERSHIP — files touched, interfaces relied on, excluded hot paths
DECISIONS — model choice, costs, approximations, rejected alternatives
IMPLEMENTED — real world-visible behavior, exact paths and key functions
VALIDATION — unit/integration/visual/gameplay/save/load, with PASS/FAIL/NOT RUN
PERFORMANCE — hardware, scene, measured baseline/delta (or unmeasured)
RISKS — regressions, known limitations, migration, unsolved edge cases
NEXT — one smallest valuable verified successor task
```

---

# PROMPT 00 — ARCHITECTURE ARCHAEOLOGY, OWNERSHIP MAP AND BENCHMARKS

```text
Apply the UNIVERSAL EXECUTION CONTRACT in 03_CLAUDE_CODE_IMPLEMENTATION_PROMPTS_V2.md. Read all three v2 design specifications. Act as a skeptical repository architect, not a feature implementer.

Goal: establish the actual codebase state so that subsequent coding agents can work independently without corrupting save data or fighting over central files. This is a bounded reconnaissance phase; do not rewrite production code or opportunistically refactor the engine.

Inspect Cargo workspace/crates, application/bootstrap loop, world voxel/chunk/material model, variable voxel resolution, terrain edits and collision invalidation, existing power networks/batteries/induction matrix/cable tiers/transformers, machines and assembler, fluid/pipes/inventory logistics, save/load/version migrations, expedition/site generation, Astral Sanctum/Undertow Knot, creature archetypes/animation/hit detection, HUD/UI and WGPU render passes. Include exact source paths and key type/function names supported by current code. Verify the earlier reported save format v21 rather than assuming it. Look for active concurrent edits and recent branches; never read those as authorization to change them.

Deliver docs/implementation_baseline_v2.md (or repository-conventional equivalent): (A) diagram of authoritative data flow and state ownership; (B) current vs missing features labeled VERIFIED / PARTIAL / REPORTED ONLY / NOT PRESENT; (C) stable IDs and known state ownership risks; (D) dependency map and one-file ownership proposal for parallel agents; (E) test and runtime launch commands; (F) existing save fixtures and migration policy; (G) profiling entry points and sample scenes; (H) specific first vertical slice recommendations for E1/H1/D1, ranked by actual dependency. Provide a machine-readable or Markdown event/interface contract sketch only where the existing code proves it can fit.

Run existing fast relevant tests without changing baselines; record failures with precise reproduction and ownership attribution. If a benchmark harness exists, run one small factory scene and one dungeon reference scene at documented settings. If not, give a minimal benchmark fixture implementation plan, not imaginary numbers. Obtain actual screenshots only when the game can launch; screenshots in uploaded prose descriptions are NOT mounted source images by default.

Acceptance: the next agent can locate its sources, run the correct tests, identify which objects own charge/cargo/claims, understand the renderer/simulation boundary and work without collision. Only change documentation/optional test harness when safe. Return evidence, not broad unsupported summaries.
```

# PROMPT 01 — ELECTRICAL FOUNDATION: REAL DC, ENERGY OWNERSHIP AND WORKSHOP

```text
Apply UNIVERSAL EXECUTION CONTRACT; read master + science v2. Own only the DC electrical/component and small directly related instrumentation/persistence modules agreed in the baseline; do not touch AC/dungeon/terrain shaders.

Implement one real playable DC workshop through EXISTING world placement and machine systems. Physical source and return paths must exist (ordinary paired-conductor block is allowed). Source has nominal voltage, output impedance/current/power limits; conductors have length/material/area resistance; switch has explicit open/closed state; resistive load/lamps convert delivered electrical energy into heat/brightness; meter shows voltage/current/watts/line loss and states WHY circuit failed (open return, short/current limit, undervoltage, disconnected port). Keep stable physical terminals and topology revisions; combine untapped cable runs where appropriate. On removal/mine/chunk crossing, no stale powered path or duplicated machine state.

Integrate with one preexisting machine (heater/assembler/burner or closest proven equivalent) by a small adapter: it consumes actual delivered Joules exactly once; progress never advances from requested-but-undelivered watts. Leave reported legacy cable tiers compatible by migration, rather than breaking old saves indiscriminately. Ground for the numeric solver must never create an invisible physical return. Bounded linear nodal/MNA solve is sufficient, with current-limited fault handling; no universe-wide SPICE framework. Record solver convergence/invalid topology diagnostics. Model machine heat only to the degree the selected demo needs.

Specific test fixture: ideal 48 V DC source, 4.8 ohm heater, 0.2 ohm round-trip conductor: current 9.6 A, load voltage 46.08 V, conductor loss 18.432 W. Test a genuinely open return and conductor removal while loaded; crossing cables that are not joined; a high-load brownout; shared battery/induction matrix limits and persistence if already present. Provide property tests for current/energy balance and save/mine/reload no charge duplication. Ensure thermal loss and solver numeric residual are not conflated.

Playable demo: place source, switch, wire, load and meter; see it light/heat; break/repair return; upgrade conductor; watch quantified loss and machine progress improve. Verify scene in runtime if available. First ensure no repeated engine-wide scans each tick; cache stable networks and schedule meaningful events. Measure time to edit a short/long run, solve an unchanged network, and update active vs idle. No high-voltage AC or chip editor content in this change.

Final: tests+runtime proof, migration behavior, exact code ownership and next smallest enabler.
```

# PROMPT 02 — INDUSTRIAL LOGISTICS WITHOUT PER-ITEM PHYSICS

```text
Apply UNIVERSAL EXECUTION CONTRACT. Read master/science v2 and the verified repo baseline. Own inventory transfer, item/bulk lots, belt run and first needed hydraulic hooks; coordinate shared item/voxel IDs with the save owner.

Implement a playable mining-to-furnace-to-storage logistics route where conveyors are physical placed voxel belts but movement is compiled into transport runs. Each run stores an ordered lane with compact item lots/gaps and one shared movement offset between handoffs; precomputed path supports bends/inclines at valid voxel scale. Inserters/junctions/splitters are explicit boundary events. No transform/collider allocated or updated per ordinary item every tick. Rendering instances use path positions and interpolate independently of authority; preserve visible individual items at close distances, use appropriate LOD farther out. Bulk ores/fluids should use aggregate mass/composition/enthalpy when carried as bulk rather than millions of item instances.

Implement atomic/reservation-based inventory transfers with consistent source/destination capacity, stable lot identity and deterministic contention. A removed belt, pipe, full output, unloaded chunk or save/reload can't clone cargo. Reuse existing inventories/assembler and UI rather than building a second logistics game. If the repo already has pipes, integrate a minimal compatible aggregated segment with controlled tank/pump transfer; do not introduce complete Navier–Stokes. For a hydraulic pressure demonstration, use bounded rated pump/flow/pipe length/head and explicit quantity conservation, but only if required by the first actual machine; otherwise isolate the interface and defer the solver honestly.

Adversarial tests: packed-lane order and spacing; outputs full → backpressure; multiple inserters attempt same lot; loop/reroute blocked; belt split and reconnection with items in flight; world mining mid-transfer; partial/unloaded boundaries; roundtrip legacy save; high-volume lot movement; two identical production runs deterministic. Compare behavior with simple reference item-step implementation on tiny fixtures, NOT by running detailed physics in production.

Benchmark before/after as items and conveyor length grow; report simulation time per run/handoff, allocations, memory, render instances and save bytes. Expect scale roughly with actual handoffs and active junctions in stable paths; do not claim constant time in all editing/unloading cases. In runtime demonstrate one line delivering real inputs to a real machine and recovery from blockage.

Stop when this works. No giant trains/robot-network rewrite or a dozen machine recipes. Submit exact handoff APIs for the multiblock owner.
```

# PROMPT 03 — VARIABLE-SIZE MACHINE COMPILER AND ONE REAL PROCESS

```text
Apply UNIVERSAL EXECUTION CONTRACT; read master + science v2. Use current voxel/machine/process/save facilities. Own structure validation, compiled machine geometry and one furnace family; share ports/lot APIs with their designated owners without rewriting them.

Build a real player-sized furnace/oven multiblock. Use a controller as a recognition aid, but compile the physical chamber/enclosure, refractory materials, heating surfaces/burners, flue/exhaust path, available material ports and one existing electric/mechanical/thermal source. Machine dimensions must be measured in actual world units for variable voxel sizes. Two valid player designs with different internal volume, external area, burner area and exhaust must yield different heat-up, capacity, efficiency or steady production RATE for physically explained reasons—not an unconditional multiblock-size multiplier. Use a small coherent thermal model, sufficient material and useful-energy accounting, bounded process stages and visible diagnostics.

Compile only on relevant world edits; keep stable MachineId and material state when wall/port changes; nonessential damage may reduce insulation, essential chamber failure disables without cloning contents. Reject physically invalid assemblies with specific on-screen reasons. Preserve old machine and save compatibility, current production statistics and item routes. Do not create a broad universal chemistry engine or 100 machine classes before this works.

Demonstration: assemble a small and a large furnace, insert the same characterized material lot, measure heat input/loss and compare batch work; change wall insulation or add an exhaust and observe a predictable relevant difference. Make the in-world interface show the limiting bottleneck and required utilities. An early player can use manual feed; an automated player can connect existing belts/ports.

Tests: volume/area scaling under different voxel resolutions; enclosure validation; finite-energy work/thermal conservation; insufficient input and power; removal of wall vs essential part; concurrent world edits and stale async compiler result; save/unload/reload without losing/doubling material/heat; performance proportional to touched region/compiled modules, not furnace voxel count every tick.

Deliver the complete single machine, reusable validated interfaces and a rigorous statement of the approximations. Do not add a boiler, distillation tower or semiconductor furnace until the implementation is proven.
```

# PROMPT 04 — AC POWER GRID, TRANSFORMERS, OVERHEAD CABLE SPANS

```text
Apply UNIVERSAL EXECUTION CONTRACT. Read master/science v2 and the verified E1 power interfaces. Own AC RMS solver/transformer/converter adapter, transmission-span state and a contained rendering adapter negotiated with render owner. Do not rewrite legacy DC transformer's behavior without migration.

Implement a working three-part power demo: small AC alternator → step-up transformer → non-voxel overhead transmission → step-down transformer/substation → real factory consumer. Preserve RMS voltage, 50/60 Hz nominal configuration where selected, source/load ratings, resistance/approximate line impedance, active power and reactive-power factor for compatible supported loads, conductor heat and transformer core/copper losses. Start with radial feeder graph; do NOT pretend to solve arbitrary unsynchronized meshed grids or electromagnetic transients. Clearly prevent a passive transformer operating on steady DC. Keep existing DC/DC converter separate and migrate old 'transformer' device with documented compatibility.

Transformer is a compiled player-built structure with magnetic core, winding count/relative turn ratio, conductor cross sections, insulation and cooling/thermal ratings. Include a default preconfigured working blueprint before allowing deep customization. Under overcurrent/thermal/voltage fault, breakers behave deterministically and machine meters explain the condition. Coupled power debit is atomic across converter ports—no free delivered output from a stale source balance.

Pylons/anchors are voxel structures with stable IDs. Players link terminals using a cable tool that creates persistent non-voxel span objects (not one cable voxel per meter). Precompute cable length/catenary or suitable sag approximation, check terrain clearance, use a small graph branch with electrical resistance and thermal/length rating. Draw it with WGPU using existing line/mesh conventions, with LOD, culling and sensible material/style. Collapsed pylon instantly severs the graph; spanning unloaded chunks still works logically. Cable editing/reconnection transfers no energy or material by itself.

Interactive acceptance: same real load at low voltage vs stepped-up high voltage over the same distance exhibits physically meaningful I²R change, while transformer losses and limits remain visible. User places anchors, wires the long-distance line, reads meters at station, cuts an anchor and observes consumer stop, then repairs/recloses a breaker. Test turns ratio, rated power, line length/resistance, phase configuration, DC input rejection, chunk/save integrity, short-trip/reset, no duplicated energy across rectifier/DC loads. Profile cable geometry rebuild vs normal network solve; verify screenshot at gameplay distance.

Do not introduce orbit-scale physics or HVDC/chiplet content into this slice. Hand off the same stable mechanism-power API to dungeon integrator.
```

# PROMPT 05 — THE FIRST REAL PLAYER-DESIGNED MICROCONTROLLER (H1)

```text
Apply UNIVERSAL EXECUTION CONTRACT. Read all master/science docs and inspect the current simulation, controller/board/saves/UI interfaces. You are building the highest-value silicon feature: a working, editable, physically constrained module floorplanner whose chosen architecture and routing **change a real machine's behavior**. This prompt is NOT a decorative tech tree or a generic quality-score minigame.

Deliver a complete functional 8-bit industrial-controller designer on a Minecraft-like effective die-area grid. Provide place/rotate/remove/move rectangular verified modules (tiny CPU control core, ALU/register bank, program ROM, SRAM, timer, GPIO, optionally ADC/watchdog), named typed connection ports, 1–3 conceptual routing layers, vias, clock/reset/power requirements and legal-grid occupancy. Initial viable design can be inserted via preset; player then changes it. Provide pan/zoom, keyboard placement, undo/redo, layer toggles, selected-net highlights, connected-port warnings and a readable compile report.

Use a declarative immutable module registry and typed functional graph. Implement bounded deterministic auto-route for most nets (grid A* or comparable algorithm with length/via/congestion costs). Manual rerouting overrides auto suggestions and must be persisted. Re-route only dirty nets/modules after edits; avoid a heavyweight external OpenROAD process on each drag. Make unsatisfiable designs say why, and suggest one concrete fix. DRC-lite: overlaps/keepouts/spacing, layer crossing/via legality, package pins and power-domain compatibility. LVS-lite: extracted net connectivity matches the intended signal graph. STA-lite: simplified critical-path analysis with sequential cutpoints and interconnect proxy; do not falsely claim full transistor signoff. Detect invalid combinational cycles rather than overflowing event queue.

A chip design must produce a functional controller: compiled digital logic/firmware tasks use registered GPIO/sensors/control outputs, memory and CPU capabilities. Default programming UI offers presets for 'maintain furnace temperature', 'pump until tank full', 'trigger after item counter', plus a small typed condition/event script if supported. Players are not required to hand-program an ISA to use their first MCU. Hardware variants are characterized by actual critical path, effective ops/s on specified tasks, memory capacity/bandwidth, GPIO/ADC count, response latency, idle watts, dynamic J/task, die area and estimated manufacturability. Extra ALU without dispatch/bandwidth MUST NOT confer free throughput. Better CPU cannot bypass heater/pump physics.

Persistence and copyability: `DesignProject` points to immutable `DesignRevision` with stable ID/content hash, nets/floorplan/route/libraries/model-version, derived characterization; editing creates a new revision and does not silently alter installed chips. Save/export/import or in-world design-library integration according to existing repo patterns. Manufactured prototype can initially use prepared chip blanks or salvaged IC packages with transparently simplified fabrication; full lithography is H2.

PLAYABLE ACCEPTANCE: Build the default controller, install it in a real existing machine, watch correct condition-driven control (low tank → pump on; high → off) with bounded response. Save as revision A; rearrange a critical SRAM/CPU route or add a timer+GPIO/redundancy as revision B; demonstrate an actual measured difference (event-response capacity, energy use, latency or enabled function) and a meaningful cost. Switch between installed chips and observe behavior. Show side-by-side performance diagnostics. Demo must not hinge on a cosmetic '+20% factory speed' multiplier.

TESTS: module/port validation; deterministic router; crossing nets don't short without via; disconnected power fails; too few address/data pins; no memory execution; clock/critical path orientation; max-workload capacity; unauthorized/recursive logic bounded; same revision same behavior; different revisions appropriately characterized; no state/energy duplication on chip removal/reinstall/reload; legacy saves without chip library load. Profile editor recompilation and runtime when 10k identical controller instances are mostly idle; cache immutable compiled function across chip instances.

Do not create nanometer voxel surfaces or simulate every transistor. Make the first controller feel polished, fast and understandable. Ship source+UI+tests+one real machine demo, runtime captured when possible. Leave detailed fab for H2.
```

# PROMPT 06 — PHOTOLITHOGRAPHY, WAFER LOTS, MASKS AND MASS PRODUCTION (H2)

```text
Apply UNIVERSAL EXECUTION CONTRACT. Read science v2 and working H1 chip-artifact schema. You are adding truthfully layered manufacturing that **mass-produces the functioning designs** already made by the player. Do not replace the valid chip compiler with arbitrary mask-pixel alchemy.

Implement the smallest coherent planar-IC production route with existing process machinery and prevalidated device macros. Data artifacts must remain distinct: immutable DesignRevision + ProcessTechnology/PdkLite (supported devices/materials, minimum features, overlay tolerance and layer rules) + generated immutable MaskSet + ordered ProcessRoute + mutable WaferLot + deterministic process/inspection records + DieGrade + Package/firmware. Use sparse/vector 2.5D masks, vertical layer stack, selected dopant profiles and aggregate wafer-lot state. No billion tiny voxels. Existing chip operation remains functional even if detailed fab models are temporarily simplified.

Build a usable multilayer viewer/editor: select layer, pan/zoom, toggle patterned resist/mask/film/dopant display with false-color legend, magnified cross-section, show intended vs manufactured structure, process stage and why a rule fails. Show at least one transparent example: substrate prep, oxidation or film deposition, coat photoresist, exposure+development, selectively etch compatible underlying film or transfer mask, strip/clean, introduce intended dopant with appropriate mask, anneal where required, create interconnect/contact, inspect electrical function, cut/package/test. These operations are not interchangeable and ordering matters. Some stages can be abstracted/preset for approachable game pacing; label fictional work/energy presets rather than implying exact factory minutes.

For piranha-type cleaning, implement ONLY high-level compatible organic-residue cleaning in sealed processing equipment, with waste/containment rating and material compatibility. This is not a universal oxide/silicon etch and no real mixture ratios, stepwise mixing instructions or practical handling parameters belong in player tutorials. Support a safer alternative cleaning route where gameplay allows.

Player may manufacture a valid process in a small accessible lab using prepared crystal/wafer feedstock before full mining/refining dependency exists. Supply upstream material capability hooks for later crystal growth, precision optics, vacuum/industrial gases, polishing and chemistry. Two independent lots from same MaskSet should reproduce comparable predictable output, with deterministic lot seeds/process settings and inspectable yield/defects. One lot produced at a less capable stage may require derating/rejection while a better process lot yields more usable chips. No random invisible quality roulette or reload reroll.

Prove mass/energy accounting (inputs, reagents, waste, heat, packaging and working dies); lot immutable design revision; overlapping design edits can't mutate active lot; masks reusable; manufacturing physical limits not bypassed by layout editor. Add DRC-lite violations (resolution/overlay/width/via) separate from electrical functional violation and from process contamination/grade. Testing can distinguish speed bins with an explicit model and traced causes, not arbitrary RNG.

PLAYABLE END-TO-END: player saves MCU revision, produces/uses mask set, runs two wafer lots through lab, inspects each processing change in cross-section, bins chips, installs them in controller and demonstrates same actual digital behavior with different ratings/yields. Save/reload at half-fabricated stage and finish same outcome. Capture screens from actual executable if available and measure mask caching/per-lot simulation cost. Stop without inventing an entire real 2 nm fab.
```

# PROMPT 07 — SCIENCE CAPABILITY GRAPH, INSTRUMENTS AND TECHNOLOGY CONTENT

```text
Apply UNIVERSAL EXECUTION CONTRACT. Read master/science v2 and inspect actual items, research, recipes, data format and localization/UI conventions. Add the smallest technology progression expansion that connects THREE already-playable systems, rather than constructing a massive prerequisite ladder.

Introduce a data-driven capability graph: material/equipment/instrument/process knowledge unlocks useful *operations and measurement precision*, with alternate routes for salvage, primitive mechanical control and magitech where already modeled. Each node must have at least two meaningful uses or one distinctive research experiment. Add a small historical vertical slice: telegraph electromagnet/relay → basic analog component or detector → transistor switching → simple controller/IC, with optionally a vacuum-tube demo only if glass/vacuum capability already exists. Use existing machines to produce components with specific electrical ratings; a 'circuit board' is not just a magic ingredient.

Add a compact codex/instrument overlay that distinguishes scientific laws from balancing approximations: meter explains resistance/voltage drop; capacitor scope can replay isolated RC behavior; process panel shows actual mass/energy and batch bottleneck; wafer viewer links a layer effect to its process stage. Make process order and voltage/supply conditions understandable; no precise manual timers or huge paragraphs on every interaction.

Use authored process definitions with schema validation; no arbitrary chemistry/ODE scripting. Preserve useful older tech; don't force all players to reenact every historical step. Build one player experiment with measurable different outcomes, manufacturing once then automated repeat, salvage alternate source and a persistent blueprint/process preset. Provide source provenance for codex claims.

Acceptance: player can obtain (not just inspect) a useful new component, apply it in a factory or field tool, compare alternatives, and use an instrument to explain the result. Test capability validation, recipe cycle/dependency issues, meaningful mass/energy bounds and save/load. Do not add 300 unused materials.
```

# PROMPT 08 — ASTRAL SANCTUM ARCHITECTURE, CEILINGS, LIGHT AND KNOT (D1)

```text
Apply UNIVERSAL EXECUTION CONTRACT; read master + dungeon v2 and inspect actual site generator/render/materials. Own dungeon kit/generation art composition and Knot mesh/state visualization; negotiate light/shader changes with designated render owner. The earlier screenshot critique is a design starting point, not a claim you've seen the current app.

Create one deterministic, walkable three-space Astral Sanctum benchmark: compressed entrance/service, gallery/workshop with useful overlook, tall binding chamber. Establish player-readable doorway sightline, hierarchy and exits; physically supported piers, beams and vaults through a compact construction grammar: pier, supported vault ribs, coffer/beam, gallery/balcony rail, service trench, framed oculus, and damaged transition. Specify world-unit clearances before quantizing to actual voxel resolution. Preserve actual phantom-floor mechanics where the site includes them, not a fake replacement.

Replace repeated full-face cyan/red crosses and evenly scattered purple overhead clusters with calm stone/structural metal/selected ivory, sparse functional gold/silver detailing and concentrated astral star-glass. Materials read at a distance with consistent scale and appropriate roughness/normal; readable silhouettes must survive reduced bloom. Add warm useful lighting at route and equipment points, and limited special emission. Diagnose actual material-light pipeline before changing it; don't brighten entire underground world or create dozens of shadow casters.

Redesign the Undertow Knot as an intentional installed apparatus: credible plinth, containment frame, visible cable/field attachment, separate locked/powered/disrupted/claimed silhouettes/indications. State visuals consume actual objective/mechanism data; no screen text substituting for missing geometry. Keep existing ID, legacy guardian claims and unrelated site content intact. If a new physical mechanism API is not ready, use verified current state behind a small adapter, not fraudulent network simulation.

Acceptance: an ordinary walking recording shows spaces from eye level, upward ceiling forms, route split, readable exits/landing/ceiling and the Knot without text labels. Measure in-game light counts, static geometry, draw calls, shadow caster/perf footprint, and memory at a representative low quality preset. Document before/after matched camera captures only if actually available. Geometry/collider clearance tests and procedural seed fixture must pass. Do not add more room types before quality is established.
```

# PROMPT 09 — THREE COMPLETE ENEMIES, HONEST ATTACK PHYSICS (D2)

```text
Apply UNIVERSAL EXECUTION CONTRACT; read master + dungeon v2. Inspect and identify actual existing Sanctum Warden, Aether Moth and crab-like archetype IDs, meshes, rig, AI, hit system and drops. Do not invent a fourth enemy from a screenshot. Own creature models/animation/behavior/pose collisions only; don't concurrently edit generator/horizon shaders.

Build three distinct, readable archetypes: Warden is a heavy, directional shield-bearing lane defender with a braced committed bash, flank weakness and honest recovery; Aether Moth has paired wings, compact core and a hover/visible aim/dart/reposition loop with bounded logical light/stimulus query; crab has arched articulated shell, working claws, low flank/lunge/recovery, and optional bounded chipping ONLY if existing terrain damage can already support it. Reuse existing rigid part transforms/material assets where possible, not per-frame mesh creation or a fresh skinning/behavior-tree engine.

Simulation owns attack transition states, authoritative transforms, shield facing/hit regions, collision and damage. Rendered pose interpolates from it; do not read a pose from render output for combat. Use swept collision for quick hit/projectiles and world-edit-aware wall collision/cover; update terrain occupancy/nav before affected movement. Movement respects stairs, ledges, enemy dimensions and natural retreat when blocked. No omnidirectional shield protection, through-wall moth darts, instant-turn lock-on during committed attacks or repeated attack loops without recovery.

Complete one functional drop/use loop per enemy only where feasible: Warden shield → actual oriented placed cover; Moth lens → legitimate light/lure emitter; crab claw → existing bounded chipping work if implemented. If the third underlying capability isn't present, defer that reward and use a supported nonfake substitute. Stable loot ownership must survive death, drop, pickup, mining, place, unload and reload exactly once.

Test 1v1 at normal encounter distance, then all relevant pairings. Capture silhouettes/poses in idle, wind-up and recovery and short real motion demonstrating at least one missed attack, valid flank, blocked dart behind newly placed wall, ledge/footing response, unique recovered ability. Unit tests for pose-hit agreement, shield direction, swept collision, no stale terrain/nav, stimulus query bounds, guardian stable identity and save/load. Performance: per-enemy update, active navigation/collision queries, GPU instances and stress vs ordinary scene; no invented benchmark numbers.

Stop at complete gameplay loops and measurable clarity before adding variants or bosses.
```

# PROMPT 10 — PROCEDURAL IMMERSIVE-SIM SITE AND ALTERNATE SOLUTIONS (D3)

```text
Apply UNIVERSAL EXECUTION CONTRACT; read master + dungeon v2 and integrated D1/D2 state. Own site graph validation, objective policy and authored mechanism glue. Coordinate any shared voxel generator/state files with an integrator. Extend existing expedition/site systems, not a parallel generator.

Build an authored reference site followed by controlled variants. First compile spatial plan: entrance overlook; upper gallery/cover route; lower service works; binding objective; fall/recovery loop; return shortcut. Geometry-first validate actual player and Warden/crab clearance, sightlines, feasible steps/jumps, accessible controls, safe entry and exit, and no unreachable objectives. Then materialize real voxel geometry and lighting. Preserve original phantom-floor mechanics, Astral Sanctum name/IDs and legacy claim logic. Generation variation must alter approach/elevation/line of sight/mechanism state/route resource cost, not only mirror walls or randomly scatter decor. Use deterministic seeds and bounded retries.

Implement at least two legitimate approaches for ONE same seed, using existing real world operations: Warden/gallery combat, and lower service/terrain-building/engineering alternative. A maintenance mechanism should have real connected power or an honest existing mechanism interface, not a fake cable texture controlling an unrelated Boolean. If full electrical power network isn't ready, use the verified existing device state and declare future adapter link; never claim 'physical power simulation' without a functioning circuit. Removing a cable/support or mining wall changes actual collision, navigation, consumer state and route. The objective evaluator should accept allowed outcomes, NOT require one scripted switch press, but legacy guardian-gated claims remain appropriately gated.

Introduce one recoverable setback: phantom-floor drop to service route, disabled lift after power cut, or alarm/changed enemy positions. Explain how the new state changes choices. Provide a real way out without hidden teleport/reset; make invalid softlock states rejected in generation or handled by a clear recovery mechanism.

Play and document route A and route B on same seed; one terrain breach/bridge; one controlled fall/fault with subsequent recovery; claim/use reward and return; save/reload after edits, guardian deaths, activated connector and claimed object; test no duplicated rewards or fabricated deaths. Inspect fixed seed batch for distinct spatial decision topology and clearance, with invalid seeds rejected and exact reasons. Profile generation, local nav rebuilds, mesh upload spikes and objective checks.

Do not expand all dungeon biomes or add arbitrary antimagic shells. Finish one honest, memorable, deeply interactable site.
```

# PROMPT 11 — CROSS-SYSTEM INTEGRATION: SCIENCE MEETS DUNGEON (I1)

```text
Apply UNIVERSAL EXECUTION CONTRACT; read all v2 specs. Act as the only designated integrator for the shared power/world/item/save/objective interfaces. Do not take over individual owners' art, shaders or industrial algorithms. Review verified branch outputs, identify actual conflicts, and integrate one thin end-to-end scene.

Goal: make an abandoned scientific/magitech apparatus obey the same operational rules as the player's factory without needing the whole historical tech tree. Choose one powered device with persistent identity (service lift, containment coil, industrial lighting, field analyzer or locked mechanism). Register its source/consumer/connector with the actual supported network; inspect it with existing meter; allow disconnect, reroute or energize using player equipment/salvage (and optionally an authored mana→electrical converter with explicit costs if already available). All dependent devices respond: removing supply can disable containment but also removes lights/lift; reconnecting supply restores compatible behavior subject to safety. No scripted fake Boolean if a real network exists.

Second path through the scene must remain viable without a late semiconductor chip: a manual/mechanical alternate control, dig/build/service passage, or existing creature/tool interaction. For a smart controller, an early salvaged board can add convenience/automation and be reverse-engineered later; it must not be the only path to progress, and a processor faster than physical machinery cannot invalidate device limits. If manual wiring is too difficult, use paired conductor connectors and good in-world meter explanations.

Version state explicitly: source/battery energy, converter transaction, mechanism position, powered consumer state, terrain edits, guardians, claim eligibility, unique controller/salvage ownership and released design revision. Save/reload at mid-transition and replay one sequence; there must not be a free energy/cargo loop or duplicated reward. Test AC vs DC port mismatch and controller undervoltage where supported. Keep emergency visibility/escape route so a power cut isn't an accidental softlock.

Deliver a playable encounter demonstrating both the engineer route and ordinary exploration route, short true in-game capture where available, benchmark normal vs activated mechanism, and an integration handoff contract for future scientific/astral dungeons. Keep the system generic enough to use in ordinary factory cells, but do not implement multiple new biomes.
```

# PROMPT 12 — PERFORMANCE, CONCURRENCY AND SAVE/LOAD RELIABILITY PASS (I2)

```text
Apply UNIVERSAL EXECUTION CONTRACT; read v2 specs and existing profiler documentation. Act as a skeptical performance/security/reliability reviewer. Your job is to measure real cost and fix the TOP measured bottlenecks without sacrificing functionality. Don't launch speculative rewrites.

Build/extend reproducible fixtures in headless/in-game form: idle and active electrical islands; changing long cable and overhead spans; 100k belt blocks with an increasing number of represented items; split/merge and blocked transport; a large number of identical idle compiled controllers; wafer lot batch repetition; factory with pipes/reactors; one Astral Sanctum route with 3 active archetypes, projectiles, power change and terrain damage; multiple fixed-generation seeds. Start at sizes current machine handles and scale carefully (aspirations in master spec are NOT guaranteed budgets).

Measure hardware/settings and CPU/GPU timings (average, p50, p95, p99, maxima where useful), allocations, memory footprint, event queue depth, topology compile, numerical solve, belt run handoffs, instance update, shader/lighting/shadow and chunk uploads. Separate one-time rebuild costs from steady simulation. Check unload/reload and topology revisions. If necessary add tracing behind development settings, avoiding per-entity logging overhead in production.

Focus top 1–3 bottlenecks: cache stable independent work, compress runs, reserve render instances, batch GPU work, bound spatial queries, remove repeated scans, reduce hot allocations/duplicate data. For each optimization run an authoritative equivalence test on key conservation/AI rules and before/after benchmark on same fixture. Do not 'optimize' by ignoring offscreen simulations with real effects or by freezing disconnected machine inputs.

Failure hunting: concurrent physical edits while worker compiles; stale path cache; double-delivery during transaction cancellation; charge and thermal state during mining; recursive digital events; half-unloaded energy networks; save roundtrip with released design revisions and existing guardian claims; render interpolation inconsistent with combat pose; old world migration. Randomized deterministic sequences and replay hash useful if feasible.

Output signed-off table of VERIFIED PASS / FAIL / UNVERIFIED for functional, visual, runtime, memory and performance. Exact regressions and unmeasured areas must be stated. Keep unrelated test tolerances intact.
```

# PROMPT 13 — ADVERSARIAL CODE REVIEW, MAINTAINABILITY AND FINAL INTEGRATION

```text
Apply UNIVERSAL EXECUTION CONTRACT; examine the actual changes and test evidence from selected implemented slice(s). You are NOT to automatically compliment them. Act as a principal engineer auditing a risky production game engine update. Review changed code line-by-line and run relevant test and runtime probes yourself where possible.

Rank findings by player-visible correctness, data-loss/duplication, physics validity, concurrency, persistence, performance, UI usability and maintainability. Look for: phantom generation/loss of Joules/cargo; stale network/terrain caches; accidental chip revision mutation; WGPU state becoming authoritative; inaccurate process semantics (photoresist vs etch vs strip; piranha cleaning role; passive transformer with DC); hidden scripted dungeon mechanics; fake omnidirectional shield; unrealistic universal 'CPU increases furnace speed'; unbounded recursive digital/graph operations; extreme allocation per voxel/lot; unnecessary crates and giant traits; collision between agents' dirty paths; broken legacy migration; conditions that cause unavoidable softlocks. Verify claims against repository and runtime, not narrative status notes.

For each issue: give file:line evidence, severity, exact reproduction, minimal fix and regression test. Then implement and verify as many highest-severity concrete fixes as safely possible within owned files. Never mass-refactor all modules merely for style. Consider whether a proposed abstraction is truly reused; if not, delete unnecessary indirection before adding generality. Protect public interfaces with contract tests; update docs to match actual behavior.

Run the principal user journey: build circuit → power process → place belt → manufacture usable component → program/control a machine; or dungeon journey: enter → recognize room → interpret enemy → choose route → recover mistake → interact physically → claim persistent reward. Mark any absent step explicitly; do not imply completion across disconnected prototypes.

Final deliverable: crisp architecture diff, verified findings/fixes, tests, actual gameplay/visual/performance evidence and one next action. Do not push, merge, or invent successful runs.
```

---

## Integrated acceptance matrix for the project owner

Use this table after each Claude Code session to prevent 'the suite passes' from becoming a synonym for 'the game works':

| Claim | Required proof |
|---|---|
| Real DC/AC power | Known circuit analytical fixtures + in-world meter behavior + wiring edit + save/port transfer invariants |
| Efficient logistics | Stable belt run tests + large-scene CPU/memory comparisons + visual moving cargo + no dupes |
| Variable multiblocks | Two different valid furnace geometries with separate measured thermal/capacity outcomes |
| Functional custom CPU | Architecture compiles + two designs execute workload + measurable different result/cost |
| Layer-driven manufacturing | Distinct masks/resist/film/etch/dopant; lot inspection; two batches; reusable unchanged design revision |
| Playable dungeon quality | Actual captures and route footage; 3 silhouettes; visible ceiling; alternate route + recovery + real loot |
| Systemic integration | Removing/rerouting real circuit affects appropriate consumers and never fabricated boss/claim result |
| Safe parallel agents | No overlap on hot shared files; clean scoped patches; explicit integrator approval; migrated old worlds |
| Performance | Actual hardware/scene/preset, before/after numbers, unchanged behavior tests |

## Prompt-authoring notes

- These prompts intentionally preserve previously established **abstract time for gameplay** while allowing physical and computational capacity modeling. This is not a request to create GHz single-step simulation.
- The original electronics brief does not itself implement AC, overhead transmission, full chip design or this expanded logistics; those are *new v2 specifications*.
- The original dungeon brief was already detailed about art and system honesty. The update mainly adds concrete world-engine integration, QA gates, stable-identity and multi-agent handoffs.
- Use the real repo as authority, not this pack's conceptual directories or imagined machine names.
