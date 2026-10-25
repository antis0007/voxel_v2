# DETERMINISTIC REFERENCE SCENARIOS AND ACCEPTANCE FIXTURES v2.0
These fixtures help Claude Code turn broad game requirements into repeatable proof. They are **design test cases**, not evidence that the repository currently contains corresponding machines or that the numbers below are current game balance values. Adopt precise values only when the necessary device models exist. Save scene seeds and model versions with the results.

## Fixture E-DC-01 — Closed-circuit, damaged wire and measurable repair
**Setup:** Ideal 48.0 V supply with realistic finite ratings chosen above demand, a 4.8 Ω resistive heater, and a supply/return conductor pair with total 0.2 Ω round-trip resistance. A player-accessible switch and three meter points (source, heater terminals, conductor) complete the scene.

**Analytical steady-state expectation:** `I = 48/(4.8+0.2)=9.6 A`; `V_heater=46.08 V`; `P_heater=442.368 W`; `P_wire=18.432 W`; `P_source=460.8 W`; residual (before numerical tolerance) exactly zero. At a chosen 0.1 s step, source energy 46.08 J, heater 44.2368 J and wire heating 1.8432 J. All energy is integrated against authoritative elapsed time.

**Actions:** Read the healthy meter; open the return conductor; confirm zero sustained load current; replace thin run with better effective resistance and confirm diminished `I²R` losses; short load deliberately with finite source current protection; reload while switch is open; mine charged storage device through defined transfer path. **Never** allow a numerical reference ground to reclose open return.

**Observability:** meter labels voltage/current/delivered power/loss and explicit fault reason. If the source model is nonideal/current-limited, update expected values consistently and declare parameters.

## Fixture E-AC-01 — Three-phase high-voltage power economy
**Setup:** Idealized balanced three-phase 100 kW load at PF 1.0, 400 V line-to-line distribution vs 10,000 V line-to-line transmission. Use per-phase conductor resistance 0.05 Ω over a chosen corridor, separate transformers with stated nonideal losses, and a controlled demonstration comparison. The simple loss computations below approximate current from nominal voltage and neglect feeder voltage-drop effects; validate with actual model in implementation.

**Illustrative currents:** at 400 V `I = 100000/(sqrt(3)*400) ≈ 144.34 A`; at 10 kV `I ≈ 5.77 A`. Three-phase conductor heat is `3 I²R` ≈ 3,125 W vs 5 W respectively, *excluding transformer losses and higher-order load-flow effects*.

**Actions:** wind/select different relative turns, start AC generator, energize line, watch reduced current and high-voltage insulation requirement, overload breaker, destroy/repair one pylon, save/load cross-chunk span. Attempt to power a passive transformer using steady DC; it must not behave like an efficient DC/DC converter. Attempt unsynchronized generator connection; fail safely if no sync logic.

**Visual:** wire catenary spans existing voxel pylons; inspector shows endpoints/line rating/sag/temperature and no magic conductor blocks along every meter.

## Fixture E-CAP-01 — Charge ownership, storage and burst request
**Setup:** Bare 2 F capacitor initially at 12 V, connected to rated finite-resistance discharge path. Usable energy down to 10 V is `0.5*2*(12²-10²) = 44 J`; total initial stored energy 144 J. A regulated capacitor bank may change output-voltage profile but not available Joules.

**Actions:** discharge 44 J equivalent work while voltage falls, stop at bank minimum, disconnect and pick up, place elsewhere; verify no duplicate charged world item and energy item. Test an RC branch analytic result for a separate supported simple circuit, with a labeled simulation-time trace. Change physical capacitance only via a safe modeled operation, not keeping voltage fixed to mint Joules.

## Fixture L-BELT-01 — Item logistics doesn't grow linearly with individual visual objects
**Setup:** 200–2,000 physical belt voxels in a straight and curved route, one inlet that produces 10k distinguishable ordered cargo items and one endpoint. Add branching splitter, inserter, full inventory, mid-path mine, and saving while cargo is in flight. Expand to 100k conveyor blocks and 1m represented items as **aspirational stress**, not baseline expectation.

**Oracle:** an intentionally slow tiny reference stepper validates final ordered deliveries and correct item count under controlled randomized edits. Production uses compiled transport runs/relative gaps/shared motion. Mass and unique-item count invariant before/after every operation. The rendered items can move individually via instances, but no per-item physics objects are required. Report CPU time, allocations, memory and GPU instance count for scaled cases and whether simulation work correlates with junction/handoff events.

## Fixture M-FURNACE-01 — Two different multiblocks, same physical process
**Setup:** Two player-built valid furnaces with equal refractory material grade but distinct internal volume, heat-transfer area, exhaust rating and burner capacity. Feed equal characterized material composition and energy sources; create a controlled variant by changing insulation only. Dimensions must be in physical meters even with different voxel resolution.

**Expectations:** larger furnace can hold more charge, can need more startup heating and may suffer a different loss/surface-to-volume ratio; throughput cannot exceed heating/feed/exhaust/output limits; changing insulation changes heat loss without arbitrarily changing alloy melting temperature or chemistry. Missing refractory panel is leak/degradation; destroyed essential chamber module disables safely with single owner of contents. UI explains which limit is active.

**Numerics:** choose actual authored process data and calculate a benchmark ledger for feed mass, byproduct mass, energy in/out/thermal change, output throughput. Do not claim industry-grade blast-furnace modeling from a simple lumped heat model.

## Fixture H-MCU-01 — Two working designs, different real use cases
**Technology/Die:** Start with an educational 12×10 effective placement grid. This is explicitly a *game placement abstraction* under a chosen mature process, not 120 transistors or 120 nm-squared cells.

**Suggested basic module shapes (illustrative balancing, revise after UI evaluation):** `Core 3×3`, `ALU 2×2`, `ROM 3×2`, `SRAM 3×2`, `Timer 2×1`, `GPIO 2×2`, `ADC 2×2`, `Watchdog 1×2`, with area outside filled shapes reserved for interconnect and power. Every module exposes typed required pins and a semantic behavior; no unlabeled adjacency buffs.

**Design A — Minimal controller:** CPU/core, ROM, timer, GPIO, basic power/clock. Implements hysteretic `tank low -> pump ON; tank high -> pump OFF`. It is low area and cheap but cannot handle a second analog pressure input without a valid ADC/path or extra processing capability.

**Design B — Precision controller:** Adds ADC and memory or faster routable critical path, with greater I/O/work capacity at area/energy cost. It monitors two independent inputs, may use closed-loop temperature compensation or detect pressure/temperature conflict and enter safe state. Compiled task profiles must account for sensor sample frequency, actual physical actuator rate, code work and memory traffic.

**Acceptance:** both compile and genuinely run. From the game, player can install A then B and observe B's additional valid behavior (or lower latency if the time resolution supports observable difference) plus higher energy/area/cost. A faster ALU without adequate memory/dispatch should give negligible benefit to the pump task. Invalid memory/clock/power net yields an explicit compile error. Moving blocks and manually routing nets can improve timing without granting universal arbitrary speed bonus.

**Variant C — Shorter layout, same architecture:** Move SRAM close to core, create a shorter legal critical net and compare a calculated timing improvement at same process and voltage; if physical proxy is too small to make a meaningful gameplay difference, use a more interconnect-sensitive test workload or report the lower bound honestly.

## Fixture H-FAB-01 — Reusable masks and correct process ordering
**Input:** One validated MCU blueprint revision, a verified planar macro PDK-like technology, wafer substrate lot and a manufacturing process plan. **No pixel-perfect mask doodles can become functioning logic without semantic/netlist validation.**

**Stage cards:** substrate preparation/measurement; oxide or selected thin-film formation; photoresist; alignment/exposure/development; patterned selective underlying-layer etch or mask transfer; strip/clean residues; masked intended doping/anneal when route requires; deposition/contact/via/interconnect and passivation; inspect/grade/package.

**Checks:** After develop, it is the **resist** pattern that changed; after etch, underlying selected film changes in opened regions; after dope, targeted semiconductor regions change electrical type/profile rather than contamination being “removed”; after contact/interconnect a testable net path exists. Chemical cleaning (including any piranha-type residue process) is high-level hazardous handling, NOT a universal oxide/metal etch or practical recipe. An invalid stage order is rejected with a useful explanation.

**Batch test:** Same immutable mask used for Lot A and Lot B; one lot under sufficient alignment/cleanliness process, another deliberately out of tolerance, both reproducible under identical saved seeds. Detect a defect and show effect on electrical test grade, rework or rejection. Save half-finished lot, edit design to revision 2, reload and finish lot under revision 1 with no mutation. Deploy a packaged working chip in same pump/MCU scene.

## Fixture D-SANCTUM-01 — Legible, systemic, recoverable ruin
**Site:** Fixed seed containing supported 3-space entrance/gallery/binding benchmark, plus distinct service route, phantom-floor recovery and return connector where supported. Uses existing names and IDs: Astral Sanctum / Undertow Knot / Warden / Aether Moth / crab-like archetype.

**Route A:** Gallery combat: observe Warden directional shield, bait committed bash, exploit flank/recovery or use cover; obtain rightful objective access and a functional reward with stable item identity.

**Route B:** Service engineering/terrain: inspect actual power or already-implemented mechanism state; isolate/restore a real line or alter terrain geometry; route to chamber using different risk/cost. The cut changes containment and other connected lamps/lift if they share supply. Do not award combat-gated rewards by inventing dead guardians. Make a recovery path when the lift fails.

**Mistake:** Fall through demonstrated phantom-floor state, reach lower works and eventually return without teleport/reset. A newly placed cover block must stop moth projectile in actual physics. Crab footing and chipping (if supported) honor the same voxel collision/material rules. Save/load after guardian death and claimed reward cannot respawn it.

**Visual capture conditions:** Screenshot normal eye-level without labels; upward view showing ceiling supports, gallery depth and sparse star-glass; Knot locked vs disrupted vs claimed; Warden attack windup/recovery; moth wing silhouette; crab mass/claws. Repeat at lower quality preset, with reduced bloom/shadows, verifying threat and objective readability. Include CPU/GPU counts for relevant active scene.

## Cross-fixture acceptance
- Tests explicitly identify **simulated game seconds** vs visual animation and do not require nanosecond timing input.
- All numerical comparisons state their source voltage, nominal or receiving terminal, phase conventions, source and component limitations.
- Any measurements not actually run against the engine remain labeled **PROPOSED/UNVERIFIED**.
- The official save owner validates migrations and identity transfer before combined integration.
- A scenario may use existing/temporary prepared feedstock, legacy machine APIs and test fixtures so the first complete gameplay loop is accessible before the entire technology tree exists.
