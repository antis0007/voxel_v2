# SCIENCE, INDUSTRIAL ENGINEERING & PLAYER-DESIGNED COMPUTING — SPEC v2.0
**Scope:** Replaces/extends the supplied electronics implementation brief. Read master document for nonnegotiable shared contracts.  
**Engine:** Rust + raw `wgpu`; authoritative CPU simulation; variable voxel resolution.  
**Status:** Proposed specification; repository interfaces and save format must be discovered, not invented.

## 1. Product requirement in one sentence
Let the player reconstruct the history of industry from raw materials; design arbitrary *valid* machine dimensions and useful electrical circuits; invent, physically lay out and manufacture original microcontrollers with reproducible functional differences; and scale factories to enormous size using compiled simulation objects instead of per-voxel/per-item ticks.

### Four player stories that every design choice must support
1. **A clever early engineer** makes a dynamo, builds a current-limited DC circuit with return wiring, measures a voltage drop, fixes it with a better conductor, and can explain the reduced loss.
2. **A production designer** builds two differently sized furnaces with different insulation and burner/exhaust capacity. The larger one does not always win. Both accept conveyors, pipes, thermal sources and automatic control.
3. **A hardware inventor** designs two chips using the same process node: a tiny inexpensive pressure-controller chip and a larger faster sensor-processing chip. The first has better cost/energy efficiency for a pump; the second improves a data-heavy surveying machine. The player can save both and reproduce or revise either.
4. **A scavenger/explorer** recovers a unique industrial controller from a damaged astral machine, measures and uses it immediately, then develops the materials and manufacturing capability to reproduce a compatible version later. They may power the original machine from an improvised grid or magical transducer with accounted limits.

## 2. Guiding rules: what to simulate and what to abstract

**Preserve:** physical connection topology, mass/energy conservation, voltage/current/real losses, meaningful current/voltage ratings, shaft work, pressure/flow/head, material compatibility, stoichiometric process classes, deposition/patterning/etching sequence, manufacturing capability windows, electrical netlist functionality, CPU architecture/data paths, timing *limits*, device power/thermal/reliability, state/revision provenance.

**Abstract:** nanosecond switching and transistor-by-transistor execution; individual chemical molecules; atomistic dopant diffusion; real fab waiting times; full Navier–Stokes through each pipe voxel; complete atmospheric chemistry; kilometric cable rope physics; GUI animation time as a physical quantity; nominal multi-GHz execution loops. Every approximation must be labeled in the codex or diagnostics.

**Research discipline:** Real processes are process-specific. Neither larger nor newer is always better. Real semiconductor patterning involves repeated film addition, lithography, development, etch/selective treatments, strip/clean, doping and metallization. Piranha-type processes belong to compatible residue-removal/cleaning; **do not** model them as a universal silicon/oxide etchant, and do not provide real-world mixing instructions. For gaming, hazardous chemistries are closed, abstracted machines with compatibility, waste and containment, not interactive actionable lab recipes.

## 3. Architecture: source of truth and derived data

```
World edits / assembly transactions
    ├─ StructureCompiler: voxel geometry → Stable MachineId, immutable geometry snapshot
    ├─ ElectricCompiler: conductor paths/ports → revisioned AC/DC graphs
    ├─ HydraulicCompiler: connected vessels/pipes → control-volume hydraulic graph
    └─ TransportCompiler: belt paths → ordered lane runs and junctions
             ↓
WorldSimulationScheduler (authoritative simulation time)
    ├─ ElectricalSolver and converter coupler
    ├─ Thermal/Mechanical/Fluid process evaluators
    ├─ Transactional cargo + bulk inventory movement
    ├─ Process lot/work advance + deterministic quality
    └─ Digital/analog controller tasks and network signals
             ↓
Commit world consequences + publish revisioned snapshots
             ↓
wgpu draw instances, circuit overlay, process inspection, wafer editor
```

**Ownership table** (write an actual code-linked version in repo prior to implementation):

| State | Owner | Cache/view consumers | Never |
|---|---|---|---|
| Item stack/bulk lot | Inventory authority (world, player or machine) | Belt rendering, process display | Clone lot in two containers |
| Electrical storage charge | One battery/capacitor identity | Solver, UI, save | Store authoritative charge in solver-node index |
| Material thermal enthalpy | Lot/control-volume or body | Heat solver, render | Reset with mesh rebuild |
| Multiblock geometry | World voxel occupancy | Compiled geometry | Make visualization authoritative |
| Physical circuit links | World ports, wires, anchors | Cached island graph | Retain old powered link after edit |
| Process result | Transactionally committed batch/lot | Statistics, FX | Produce if inputs/power aren't debited |
| Chip design | Immutable revision/version store | Editor, compiler, fab masks | Mutate a released revision silently |
| Fabrication yield | Lot's saved seeded state + tests | Diagnostics and binning | Reroll on reload |

### Core interface sketches (adapt to existing engine)

```rust
// Representative boundary sketches; actual Rust types/serialization adapt to repo.
struct MachinePort {
    id: StablePortId,
    machine: MachineId,
    local_transform: PortTransform,
    domain: PortDomain,  // ElectricalDc, ElectricalAc, Fluid, Shaft, Cargo, Signal, Arcane
    capabilities: PortLimits,
}
struct MachineDefinition {
    family: MachineFamilyId,
    required_modules: Vec<ModuleRequirement>,
    geometry_rules: Vec<GeometryRule>,
    supported_operations: Vec<ProcessCapabilityId>,
}
struct CompiledMachine {
    identity: MachineId,
    structure_revision: u64,
    geometry: GeometricCapacities,
    validated_modules: Vec<ModuleId>,
    ports: Vec<StablePortId>,
    current_process: Option<BatchId>,
    quality_and_condition: EquipmentCondition,
}
struct ProcessPlan {
    steps: Vec<OperationStep>,
    alternate_routes: Vec<AlternativeRouteId>,
    required_capabilities: Vec<ProcessCapabilityId>,
}
struct MaterialLot {
    identity: LotId,
    material_class: MaterialClassId,
    mass_kg: f64,
    composition: SparseCompositionId,
    thermal_enthalpy_j: f64,
    selected_quality: QualityProperties,
}
```

Use SI internally, dimensional wrappers where helpful (`Joule`, `Watt`, `Volt`, `Ampere`, `Ohm`, `Kelvin`, `Kilogram`, `Pascal`, `Meter`). `f64` for circuit network math, reproducible deterministic processing for critical inventory/counters and carefully designed numerical tolerance for float residues. Rendering converts to appropriate `f32` snapshots. Never mix Joules and kWh silently: 1 kWh = 3,600,000 J.

## 4. Multiblock machinery: compiler, not recipe-box detection

### Assembly semantics
A recognizable controller/anchor is a construction aid, not the actual machine. The structure is compiled from **working chamber**, **containment/structure**, **energy coupling**, **process internals**, **material ports**, **waste/exhaust**, **measurement/control**, and optional **maintenance/access** modules. Some components are integral and some degrade gracefully.

A structure is legal only if it satisfies explicit geometry and mechanical constraints: proper chamber enclosure, reachable ports, valid heat source, rated materials, supported internal layout, minimum clearance for moving parts, no overlapping active ownership with another machine, and bounded size. Use **physical meters** from variable-resolution voxel geometry.

On world edit, invalidate affected modules (not every unrelated structure). Recompile under a world-revision snapshot; reject stale worker results. Preserve machine ID and state if structurally compatible. If split or destroyed, perform a single defined material/energy disposition: remain in surviving subassemblies, drain to specified salvage, spill locally, or safely shut down. Never duplicate a tank because a wall was split.

### Nonlinear design models
- **Furnace/oven:** Working volume increases charge; exposed area drives heat loss; heating surface and burner rating constrain heating power; exhaust constrains throughput. Simple thermal body: `C_th dT/dt = P_heat - P_losses - P_process` with proper units.
- **Boiler:** Heating area, working pressure and feedwater matter; approximate saturation/steam capacity via validated tables or authored curves. Dangerous pressure failure is rare, fair, communicated and survivable when possible.
- **Chemical reactor:** Tank volume, mixing power, reaction kinetics proxy, cooling area and residence time affect yield/throughput; runaway treated as a declared safety consequence, not random punishment.
- **Distillation column:** Height/packing/trays affect number of effective stages; diameter and internals influence permissible flow; reboiler/condenser/reflux affect separation and energy use.
- **Electrolysis/plating:** Voltage/current/electrode surface/chemical inventory and bath condition affect production via simplified Faraday-style relationships plus loss/efficiency. No arbitrary free matter.
- **Machine tool:** Travel dimensions, power, rigidity, spindle, tooling and material decide achievable envelope/quality, with cost/heat from actual work.
- **Lithography system:** Optics/resolution/overlay, supported wafer size, stage precision, illumination and process conditions determine what features can be printed; bigger chamber is not smaller linewidth.

A common rate evaluator may take the most restrictive *compatible units* among heat, feed, reaction, exhaust, cooling, work/pressure and output limits. Do not conflate different physical units in a `min()` expression.

### Scope cap
Start with **one variable-size furnace**, **one useful hydraulic reactor/tank**, and **one equipment-derived electronics tool**. Only generalize where two distinct existing machines demonstrate shared behavior; don't build a universal machine grammar before a working example. Complete usable UI (cutaway, port states, limiting constraint, rebuild warning) before adding machine count.

## 5. Materials, process plans, lots and industrial science

### Data vs process
- Materials store physical identity and selectively relevant properties (density, heat capacity by range, resistivity, melting point, chemistry and compatibility where authored).
- Processes store **typed physical transformations**: HeatTreat, Melt, Cast, Distill/Separate, GrowCrystal, Grind/Polish, Deposit, Oxidize, CoatResist, Expose, Develop, EtchSelective, StripResidue, Dope, Anneal, Metrology, Package, Test, Recycle.
- A recipe is a plan of operations/capabilities, not an arbitrary countdown. Duration is a gameplay work requirement coupled to delivered power/capacity. Animated durations are cosmetic.
- Chemical operations use bounded authored reaction records with molar coefficients, selected phase/enthalpy and declared waste. Validate element/species balances for supported reactions. Do not promise unrestricted reaction inference from arbitrary chemicals.
- Material quality is sparse and process-relevant: e.g. chemical purity, structural grain/defect grade, silicon dopant type/profile category, wafer particles/overlay, metal conductivity and mechanical tolerance. Don't propagate 50 unnecessary per-item floats that defeat stacking.

### Example materials progression (branching, not every gate compulsory)
Ore characterization → crushing/separation → coke/reductant + blast-furnace routes → cast iron/steel/copper → precision tools/pumps/pipe → clean glass/ceramics/insulation → chemical purification/industrial gases → purified silicon → single crystal → prepared wafer → discrete semiconductor → IC → custom microcontroller → controls that improve the prior processing chain.

Alternative paths: used components/salvage, magical purification with explicit costs, mechanical/relay automation before transistors, electrochemical energy, simple stand-alone instruments before advanced power systems. Each domain should enable at least two useful player activities; avoid dead-end subtrees.

### Additional critical technologies (must appear in the long-term capability graph)
Electrical protection and metering, AC alternators/transformers, power electronics, electrochemistry/batteries, dielectric materials, metrology/calibration, machine tools, vacuum pumps/seals, optics and lithography illumination, controlled atmosphere, cooling/refrigeration/cryogenics, materials testing, surface preparation, PCB/packaging, signal transmission, radio/radar, computer memory, software/firmware, digital/analog control, RF/antenna devices, industrial gas supply, recycling/environmental control, aerospace guidance and robotics. Later specialty branches: SiC/GaN power devices, optical sensors, MEMS, advanced integration/chiplets, photovoltaics, high-voltage insulation, superconductors, space systems, magical transducers.

## 6. Item/conveyor simulation: many objects with few updates

**Three representations:** discrete stack (`ItemKind`,`variant`,`count`), unique item (`ObjectId` + individual state), bulk lot (`mass`,`composition`,`enthalpy`); transfer between representations at explicit pack/unpack/mill/industrial operations. Do not allocate one entity or collider per rock on a belt.

### Belt topology compiler
Each placed belt block provides direction, lane occupancy, speed/carrying rating and optional interface. Compile maximal runs through straight and curved sections, including ramps/vertical conveyors where path rules permit. Split runs at junctions, insertion/extraction access, specialized speed transitions, reversible actuators or conditional devices. Precompute 3D arc-length path; don't move each cargo transform each tick.

A run has ordered item references and compressed gaps/positions, shared motion offset, endpoint, speed and blockage state. Advance common offset between interactions; only perform work for next handoff, insertion, removal, splitter routing or topology changes. Use stable cargo identity/transaction IDs for source-to-destination transfer. Preserve lane item ordering. A backed-up line must not overlap physical item occupancy or create one fake stack at a splitter.

Motorized belt energy: aggregate shaft/motor demand with approximate rolling/friction load, incline `P >= mdot*g*Delta_h/efficiency`, optional startup current. Mechanical motors do not obtain free infinite torque, but avoid a separate constraint per belt tile.

Render: construct visible item transforms on worker/GPU from compressed line positions and path; instance meshes; group by materials; far LOD simplifies/suppresses item-detail draw while simulation remains. Debug overlay shows throughput, blockage, power draw and queue occupancy.

**Factorio evidence:** Factorio FFF-176 documents consecutive transport-line compression and gap-based item movement. It is a design inspiration, not a guarantee the same data layout best suits the 3D/variable-voxel engine.

### Inventory transactions
Reserve source quantity and receiver capacity under deterministic ordering, commit both or neither. Routing uses indexed available providers scoped to connected logistic networks; no world inventory scan on each request. When a conveyor is mined mid-transfer, resolve cargo identity into a single remaining owner/physical pickup or fallback storage. Save/load and packet/event replays must not duplicate cargo.

## 7. Pipe and hydraulic networks

Compile uninterrupted pipe voxel runs into directed/undirected edges between junctions, pumps, valves, vessels and machine ports. Track length, effective bore, roughness, elevation, max pressure, contained volume, insulation/temperature effects. Treat a well-mixed short section as one control volume. Long pipes may carry an explicit propagation delay or a few axial volumes if that's relevant.

A hydraulic solver uses a bounded steady-state pressure/flow approximation and nodal mass continuity. For incompressible pipes start with Darcy–Weisbach friction plus fittings/elevation; pumps have characteristic pressure-vs-flow and efficiency. Thermodynamic/phase-change and compressible gas fidelity should expand only for particular processes with needed gameplay. Impossible flow or invalid phase needs a diagnostic, not negative mass.

Rupture transfers *accounted* mass/enthalpy into local world fluid/thermal systems, spawning bounded visible effects rather than per-liter rigid bodies. Splitting or unloading a pipe cannot delete or clone its contents. A sealed remote pipeline doesn't mix instantaneously across continents unless a declared simplified model explicitly says so.

**Factorio tradeoff:** Factorio 2.0 fluid segments deliberately sacrifice realistic hydraulic pressure; a production-scale game can keep similarly compact segments but expose an optional physically meaningful network hydraulic solve for important circuits. Teach players with a gauge why a pump or wider pipe helps.

## 8. Electrical networks and energy ownership

### Physical connections
Power flows only through a valid circuit with source and return. A numerical 0 V reference is **not** a physical ground wire. Conventional convenience cables may bundle paired DC conductors or three AC phases inside one placed voxel. Benches expose individual conductors. Crossing wires do not connect unless the connection is intentional.

Graph nodes are junctions/terminals; finite-resistance wires are edges; long series runs are compressed; stable component IDs survive recompile. Merge components incrementally, use BFS/split checking on deletions (union-find by itself can't process edge deletion). Cross-chunk/unloaded regions maintain persisted connectivity. World edits mark affected ports invalid before solver results can be consumed.

### DC source, load and storage models
`P=VI`, `E=integral(P dt)`, `Delta V=IR`, `P_copper_loss=I²R`, `R=rho(T)L/A`.
Current-limited sources have droop/output impedance; faults never draw infinity. Heater `P ≈ V²/R` for fixed R. Regulated loads may draw `I≈P/V` only in rated window and otherwise limit/shut down with hysteresis. Motors use a bounded load/torque/stall model only when useful. Battery energy, open-circuit voltage curve and shared max charge/discharge are authoritative once per device, not per port.

Capacitor: signed `Q=CV`, stored `E=0.5 CV²`; preserve charge/energy across circuit rebuild, mining, pickup, reconfiguration and save/load. Ordinary lines' capacitance may be summed by branch if relevant. Bare capacitors do not act as fixed-voltage batteries. A regulated bank includes a converter and uses `E_use=0.5 C(Vmax²-Vmin²)` before losses. Simple RC experiments can use the exact exponential; general networks use bounded numerical integration. Avoid changing C at fixed V to produce free charge/energy.

Numerics: linear resistive networks via nodal analysis/MNA; ideal sources handled correctly; capacitance via implicit model where needed. Do not assume an MNA matrix is SPD. Device nonlinear solves are bounded, with convergence diagnostics; no invented output upon failure. Cache ordering/patterns and reuse factorization only when coefficients unchanged. Electrical accounting produces heat from actual modeled losses, separate from numerical residual.

### AC and transformers (important new feature)
AC grid uses RMS phasor/complex-power model at player-relevant timescales, with nominal frequency, phase configuration (single/three-phase), voltage/impedance, active power `P`, reactive power `Q`, `|S| = sqrt(P²+Q²)`, current and losses; synchronous generator compatibility and breakering. This is **not** a 60 Hz waveform sampled at the world tick, and it is **not** just sum-demand ÷ voltage for arbitrary meshed networks.

A transformer is a magnetic, passive **AC** two- or three-phase assembly with real winding ratio `Vs/Vp≈Ns/Np`, conductor losses, magnetization/core losses, insulation, thermal rating and applicable frequency/flux bounds. Coils/core/insulators/cooling are a recognized multiblock; dimensions affect realistic effective rating. A DC/DC converter is a separate averaged switching-converter device. Rectifiers/inverters bridge AC/DC with coupled input/output consumption and efficiency; no duplicate watts during a step.

Transmission examples must use actual rated voltage rather than invented arbitrary tiers. For simple three-phase balanced steady state `P = sqrt(3) V_LL I PF` (ignoring losses for the formula). Explain high-voltage benefit `I²R`, but also insulation/protection and transformer losses. Complex AC load flow can be deferred until actually building meshed multi-generator systems; initially support bounded radial feeders and state model limits clearly.

### Non-voxel power lines and substations
Pylons/towers, anchors and transformer-yard equipment are physically placed voxel structures. Overhead conductors span between stable anchors as non-voxel persistent entities. Compile each conductor bundle into one or a few electrical branches containing length, resistance, approximate reactance/capacitance and thermal limits. Render low-cost catenary/sag curves from height, span and tension; recompute when supports, temperature or relevant environment change. Bounds/clearance to terrain and structures must be checked; contact/damage is authoritative. Tower collapse disconnects before the next solver transaction. No kilometers of cable voxels or rigid-body chain constraints.

Substation features: transformer bank, switchgear, isolators, circuit breakers, fault trip/latch/reset, metering and bus sections. Electrical heat, fault and overload rules are declared with instrument explanations. HVDC becomes later technology requiring converter stations, not a cheap shortcut around all losses. Telecom cable/radio/fiber later shares world attachment/routing infrastructure but **not** the electricity solver.

## 9. The chip editor: player-authored silicon with a constrained truthful compiler

### Fundamental data separation
There are **four distinct things** players should see, even if one UI can switch among them:

1. **Architecture / logic:** What the chip does: modules, registers, ALUs, memory, buses, peripherals, state machines, timing domains, interrupts, firmware/logic semantics.
2. **Physical macro floorplan:** Where verified functional modules are placed on effective die area; named ports; signal, clock and power routing; wire lengths and congestion.
3. **Masks / fabrication layout:** Reproducible **2.5D** patterns and material stack from the selected process PDK-like templates; optional expert layout editor with guarded supported primitives. Grid squares are *placement sites*, not literal transistors.
4. **Manufactured/package item:** A wafer lot, yield/test results, selected speed/power bin, die/package/pins and programmed firmware. The package is what is installed in machines.

Never convert arbitrary player-drawn colored pixels into a “valid CPU” with invented behavior. The beginner route uses verified macro cells tied to a semantic functional model. Expert edits affect functionality only when extracted netlists can be validated using supported primitives.

### Progressive playable editors
- **Bench/board:** place discrete resistors, capacitors, transistors, diodes, relays, sensor/comparator, digital gates. The analog subset uses supported piecewise/MNA models; digital subset event-driven logic. Output an actual functional board in a machine slot.
- **Microcontroller grid:** place validated rectangular modules on a grid (for example beginner 8×8 or 12×10 effective sites; values are **game balancing**, not real node density). Modules rotate if allowed, expose typed pins, electrical power, geometry and timing constraints; player connects them with routing layer(s). Default auto-router can produce a useful design.
- **Physical IC:** once patterned manufacturing is available, choose process and die dimensions. A legal library maps modules to internal hard/soft macros, routes nets, checks DRC/LVS/STA-like constraints. Save mask layers. Players see true layers/vias/interconnect and a magnified cross-section, but do not manually place every gate.
- **Expert research:** optional individual standard cells/transistor devices, analog blocks, specialty power and magical interfaces, advanced die/package/chiplet design; only after MVP proves real usability.

### Module registry: no universal additive buffs
Each `ModuleSpec` declares:
- Functional ABI/protocol/semantics; typed input/output port bus width and direction.
- Bounding footprint for a given process technology, legal rotations and keepout region.
- Resource demand: gates/logic, SRAM/ROM capacity, clock/reset, power/thermal, pad/pin needs.
- Timing characteristics: critical input-to-output paths, sequential boundaries, setup/hold where supported, net parasitic proxies.
- Power characteristics: idle/leak, switching energy/activity proxy, dynamic voltage range, thermal limits.
- Manufacturing compatibility and specialized equipment/quality requirements.
- Reliability/diagnostic features (watchdog, ECC, redundancy) with real costs.

Example modules: 8-bit datapath, ALU, controller sequencer, register bank, ROM, SRAM, timer, GPIO, serial UART/I²C/SPI-like port, ADC, DAC, watchdog, DMA, bus/crossbar, small DSP, encoder/compressor, sensor-fusion accelerator, hardwired logic, analog comparator. A module is not functional unless connected and accessible to the selected execution model.

### How placement is validated and costed
- Block/footprint overlap, orientation, keepout and die-outline legality.
- Port compatibility (signal width, driver/fanout, clock/reset polarity/domain, analog/digital voltage, power, external package pins).
- Functional reachability from CPU/control graph to memory and required devices. Two ALUs without operand dispatch do not double performance.
- Ordered layer occupancy, crossings/shorts, vias, congestion, maximum route capacity and finite routing search effort.
- Clock/power tree: supply rails and return, clock source and bounded fanout; advanced tools optimize buffers/trees. Ordinary players use auto-repair.
- Workload characterization from architecture plus feasible operating frequency, not number of squares filled.

Routing algorithm for MVP: grid occupancy bitmap per routing layer; a bounded A*/Dijkstra route for critical typed nets, cost for length, turns/vias, congestion and high-fanout; deterministic connection order; incremental dirty-net reroute after a moved module. Larger designs can later use a global routing pass plus detailed legalization; never call a heavyweight external EDA stack at every mouse move.

### Process-node and effective-grid progression
Keep physical die area separate from effective placement-site count. Shrinking process pitch enlarges the beginner's **effective architectural grid** at the same die size, but device density gains are not fixed arbitrary ×4 and do not apply equally to digital logic, pads, analog, memory, power devices or routing. Modern node labels don't literally state measured gate length.

Increasing density creates constraints: power density and heat, clock distribution, line RC delay, leakage, defect sensitivity, mask/alignment demand, lithography complexity, wafer yield and cost. Older nodes remain useful for rugged controls, high-voltage circuits and cheap analog. Newer processes do not magically solve package/pin/bandwidth bottlenecks. True active-die 3D stacking is a later *packaging integration* branch, distinct from ordinary metal layers.

### Compiler-derived performance
Compute separate normalized measured capabilities, not a single `performance_multiplier`. Suggested categories:
- `max_reliable_clock_hz`: critical-path bound + margin, voltage/temperature/process grade and package constraints. Game need not step cycles.
- `instructions_or_ops_per_s` for a *specific workload* (effective IPC, functional-unit throughput and dependence).
- `memory_bytes`, `memory_Bps`, latency/cache hit proxies, bus bandwidth/fanout/route delay.
- `I/O channel count`, analog resolution/range, interrupt/event response bound, firmware storage.
- `idle_W`, `J_per_operation`, usable voltage and thermal envelope, cooling requirement, failure/reliability characteristics.
- Manufacturing: die area, estimated wafers/die, predicted defect yield, process steps and per-part resources.

First-order models: `f_max ≲ 1/t_critical`, `P_switch≈alpha*C*V²*f + P_leak`, throughput bounded by compute vs bandwidth × operational intensity. Evaluate an **application work profile**, not just peak ALU throughput. Preserve capacity ceilings and bottlenecks: software cannot magically exceed sensor sampling or furnace heat-transfer limits.

Runtime behavior: event-driven tasks/firmware with bounded operations, deterministic signal changes and persistent memory; simple board-level logic executes via signal graph, no GHz ticks. Microcontroller *compiled tasks* debit appropriate energy, respect local delays and bandwidth/I/O. If actual instruction emulation is used, bound instruction gas/time and sandbox it; no arbitrary filesystem/network access. Unsupported recursive combinational loop yields explicit diagnostic; stateful feedback is allowed.

### Player-visible hardware archetypes (all emergent, no forced prefabs)
1. **Cheap 8-bit safety controller:** small ROM, timer, watchdog, a few GPIO/ADC; good for stable industrial control and low watts, poor at streaming analysis.
2. **Low-latency machine controller:** prioritized interrupts, ADC acquisition and deterministic I/O; better regulation, higher silicon/pin cost.
3. **Data processor:** ALUs/DSP, SRAM and DMA; good for radar/terrain analysis but may bottleneck on memory.
4. **High-reliability orbital controller:** ECC/redundancy/wider temperature range; larger/expensive/slower but fault tolerant.
5. **Hybrid runic sensor controller:** reads supernatural field via bounded physical/magical interface; cannot turn magic into infinite watts.

**Non-negotiable:** Same furnace with same heat/feed cannot magically process more raw metal merely by installing a fast processor. The processor can reduce oscillation, improve yield through regulation, coordinate inputs and reduce idle time until a real physical bottleneck is reached.

## 10. Physical wafer design, fabrication and lifetime

### Immutable artifacts and versioning
```
ChipProject
  ├─ DesignRevision (architecture, nets, floorplan, optionally schematic)
  │   └─ CharacterizationReport (model version, timing/power/area/workload evidence)
  ├─ ProcessTechnology (legal patterns, supported devices, material/process limits)
  ├─ MaskSet (content-addressed immutable masks tied to revision + process)
  ├─ ProcessRoute (ordered operations, capabilities, compatible equipment)
  ├─ WaferLot (substrate, batch material, completed stages, seeded defects, metrology)
  ├─ DieTestResult / Binning (passed/derated/rejected grade)
  └─ PackagedChip (package pins/thermal, revision, fabrication provenance, firmware)
```
A blueprint edit creates a new revision; an in-progress wafer lot never silently switches to the newest design. Immutable mask sets can be reused across batches. Wafers are batch records, not millions of microscopic live items.

### Stage-by-stage layer process (game-visible actions with truthful semantics)
1. Refine silicon-grade material or acquire prepared substrate; characterize purity/defects.
2. Grow crystal (Czochralski or alternate float-zone route where later supported); slice, lap/polish and clean wafer.
3. Form a selected film (`Oxidize`, `Deposit`) with specified material, uniformity and defects.
4. Apply photoresist to an appropriate surface; select positive/negative behavior in advanced routes; verify compatibility.
5. Align mask, expose pattern and develop resist (lithography patterns resist, not directly silicon).
6. Use patterned resist **or pattern transfer into another hard mask** for the following selective process.
7. Etch exposed compatible material **or** perform defined selective doping/other treatment using appropriate masks; method determines anisotropy/selectivity/reach.
8. Strip/clean residues when appropriate, taking account of incompatible materials. A piranha-type cleaning process addresses residual organics, **not universal silicon oxide removal**; keep real-world operations abstract and nonactionable.
9. Implant/diffuse dopants (intended dopants are not contaminants), anneal after operations that require activation or damage repair. Avoid exposing photoresist to incompatible high-temperature operations.
10. Repeated deposition/patterning/contact and metal interconnection, vias, planarization, passivation. Each new layer has meaningful electrical or protective function.
11. Inspect overlay, thickness, contamination, leakage and electrical continuity; test die; grade and package using real pin/heat limits.

The exact sequence is process-technology specific: no one universal full-recipe progression. A beginner uses verified planar transistor/logic templates with 2–4 meaningful operations repeated in an understandable overview, while an advanced process reveals more granular choices. Teach by annotated magnified false-color cross-sections and layer-specific changes.

### Fabrication quality models
Deterministic selected defect mechanisms rather than opaque dice: contamination → leakage/yield, poor alignment → missing connection/short, nonuniform film → tolerance/variable threshold, optical limit → unresolved shapes, contamination/wet chemical incompatibility → damage, inadequate packaging → electrical/thermal risk. Each has a diagnostic measurement, possible prevention, and cost. Lot identity + process settings + stable seed determine randomized defect outcomes; reload cannot reroll.

Yield may use `Y_random≈exp(-D*A)` as a simplified independent random-defect teaching model, with systematic process violations handled separately. Must state area and defect-density units correctly. Larger die reduces number of dies per wafer and can reduce yield; replacing a mature controller with a larger new design may raise product cost despite better peak speed.

A packaged chip has distinct **architecture**, **fab-process revision**, **tested grade**, **package**, **firmware**. Many devices share immutable blueprint data and only store mutable runtime state.

## 11. Controls, instrumentation and teachability

Power UI: physically connected net/return, voltage/current/power at selected ports, cable loss/hotspot, open circuit, current limit, overload, brownout, charge/discharge state, transformer ratio, frequency, substation trip, line clearance. All data available to in-world instruments and expansion panels; do not add silent hidden variables.

Machine UI: section view, chamber dimensions/working capacity, valid/invalid modules, limiting bottleneck, heat and utility budgets, material/yield/waste state, batch plan, possible upgrades with causal explanation. An automatic preset works without the player micromanaging timings.

Chip UI: functional module grid + mini-map; net highlight and power/clock map; routing layers, legal/illegal cell overlays, concise DRC/LVS/STA-inspired reports, critical-path highlight, predicted per-workload capabilities, fab-compatible process list, cost/yield estimate, saves/branches/compare, one-click **good default** auto-route and explanatory suggestions. Users can compare two revisions on the same workload. Avoid enormous tables on the initial editor screen.

Wafer UI: wafer-lot flow card, current layer cross-section, mask overlay/false-color doping, metrology results, grade distribution, waste/rework decision and saved process preset. Time labels distinguish player gameplay time from approximate real-world processes.

Research/ruin UI: readable terminals showing voltage/power/condition; architectural support and alternative access; inspection tells players which subsystem can be rerouted or repaired. Preserve exploratory discovery rather than an intrusive wall of tooltips.

Accessibility: no information conveyed by color alone, text alternatives for visual states, adjustable UI scale, predictable keyboard/controller controls, undo/redo in editor, valid save on partially finished design, progressive tooltips rather than forced tutorials.

## 12. Scheduling, determinism, chunk bounds and cache budgets

**Event hierarchy:** committed voxel edits; topology change; device parameter change; power/hydraulic solve; reservation; machine work/thermal advance; digital control events; gameplay consequences; UI/render snapshots.

Sleep unchanged stable islands with analytic/threshold scheduling. Advance cached power energy over real authoritative game elapsed time, not visual clocks. Wake before battery empty/fuel empty/batch completion/thermal limit. Subtick transient accuracy is handled by analytic or bounded numerical models only when a relevant outcome depends on it.

Workers may compile graphs/paths/wafer layouts using immutable snapshots but publish only if input revisions still match. No reader of cached topology may assume it is current after a world edit. Loaded and unloaded chunks share authoritative logical state; don't freeze half an energy network while producing elsewhere.

**Perf observations to instrument:** time/allocations by world edit and graph compile; DC solve and AC solve p50/p95/p99; hydraulics; lot transfer; belt run update vs rendered instances; machine process scheduling; wafer compiler route/DRC; digital event queue max and latency; per-scene CPU/GPU frame time p50/p95/p99; chunk streaming/mesh upload spikes and active memory.

Set budgets from measured baseline and representative hardware **after** profiling. Don't declare a made-up 0.1 ms cost achieved or weaken tests to pass.

## 13. Headless test matrix (required)

| Domain | Required test |
|---|---|
| DC | 48 V source, 4.8 Ω heater and 0.2 Ω round-trip cable yields 9.6 A, 46.08 V at load, 18.432 W cable loss (ideal source assumptions) |
| Topology | Open return has no sustained load current; switch split/merge only affects touched islands; crossed wires don't short by proximity |
| Storage | Capacitor energy `0.5*C*V²`, finite inrush, no duplication on mining/rebuild/reload; battery shared limits enforced across ports |
| Converter | Passive transformer not working on steady DC; DC/DC/rectifier output debits corresponding input and losses |
| AC | Known ideal turns ratio, rated load, balanced three-phase current; incompatible unsynchronized generators not silently paralleled |
| Transmission | Break anchor, instantly remove branch from solve; recompute clearance/sag, persist endpoints across chunks |
| Multiblock | Nonessential wall damages heat loss; break essential internal module safely stops machine; fluid/material ownership survives split |
| Process | Mass/species, delivered Joules and thermal balance; absent input cannot produce output; repeat batch under equal conditions |
| Belts | Equivalent throughput and order under straight/curved lines vs reference item-level model; blocked/split/mine/reload no duplication |
| Fluids | Parallel network continuity, pressure head and pump limit; ruptured pipe transfers same mass to local world |
| Editor | No component overlap, port type/width mismatches rejected, invalid nets explained; routing deterministic under stable seed |
| Performance | Adding idle item-laden transport length scales substantially better than per-item/per-voxel update; record CPU+memory |
| Hardware | Two legal layouts with measurable different critical path/power; second ALU not useful when dispatch/memory bottlenecked |
| Workloads | Controller speed improves event response but doesn't violate heater/pressure throughput ceiling |
| Masks | Resist pattern, material etch, strip, doping and metal contact change correct distinct layer/state; invalid order diagnosed |
| Lots | Design revision immutable; same lot seed/reload same defects and bins; reject invalid feature sizes before fabrication |
| Save/load | Legacy devices/cable rules load under documented migration, unchanged unrelated world/chunks survive |
| Idle/unloaded | Cached power integrates and stops at empty battery; no offline mass/energy generation |

Also test adversarial cases: degenerate zero-ohm loops, conflicting ideal sources, huge graph, race between routing and excavation, deliberate structure changes during async compile, dependency cycles in digital combinational logic, paused-world trace exploit, power-off while batch completes, multiple simultaneous inserter reservations, broken game update on older world versions.

## 14. Recommended phases and definition of done

| Phase | Playable vertical slice | Hard gate before expanding |
|---|---|---|
| E0 | Inspect/migrate existing power and save subsystem | Baseline + ownership map + test fixture |
| E1 | DC workshop with actual return, meter and one machine | Physics tests, runtime circuit build, save/load |
| E2 | Aggregate belt/inventory transfers and modest pipe network | One automated line, no dupe on edits, benchmark |
| E3 | Scalable furnace with real geometry and thermal/feed constraint | Two legal differently sized designs with distinct measured behavior |
| E4 | Small AC transformer and span/substation test across terrain | Voltage reduction of loss, protection and anchor-destruction proof |
| H1 | Chip editor: valid 8-bit controller, two nontrivial layouts | Both execute same function; performance differences verified; usable factory controller |
| H2 | Process technologies, masks/wafer lot and packaged chips | Manufacture same revision in two lots; compare yield/grade; save/load |
| H3 | More historical/specialized chips, optics, fab automation | Each adds distinct use and no dead-end ingredient ladder |
| I1 | Integrate a salvaged controller in functioning dungeon/world mechanism | One real physical alternative, legacy claims preserved |

**First end-to-end objective to prioritize:** Player designs an 8-bit controller, watches auto-route produce legal nets, improves it manually (for latency or efficiency), saves revision 1.1, manufactures two batches through a simple valid planar process, bins chips, installs them in a real pump/furnace controller, and measures the outcome. Provide a prepared-wafer/salvage path to reach this before the entire upstream industry is complete. Then expand the upstream supply chain backward as worthwhile gameplay.

## 15. Findings that should not be “fixed” by adding content

- More recipe steps do not equal deeper science. Eliminate processes with no distinct causal meaning.
- More production hours do not equal investment. Reward early manual discovery with dependable presets/automation.
- Faster nominal CPUs do not create new machine throughput unless an actual control/computation bottleneck exists.
- Bigger chips have larger die and thermal/yield tradeoffs; old process nodes remain competitive.
- Decorative overhead power cables are not power networks. A break must affect real energy and equipment.
- Better fab process doesn't compensate for illegal logic or disconnected ports; separate DRC, LVS, timing, functionality and manufacturing quality failures.
- A process simulation without useful UI is not educational. Inspectors and explanatory diagnostics are acceptance requirements.

## 16. Primary external technical references

- Factorio belts, *FFF-176*: https://www.factorio.com/blog/post/fff-176
- Factorio fluids, *FFF-416 / FFF-430*: https://www.factorio.com/blog/post/fff-416 and https://www.factorio.com/blog/post/fff-430
- OpenROAD stage separation (floorplan/placement/power distribution/clock/routing): https://github.com/The-OpenROAD-Project/OpenROAD
- OpenSTA timing and integration/reference (note license if borrowing code): https://github.com/The-OpenROAD-Project/OpenSTA
- SkyWater SKY130 PDK design rules: https://skywater-pdk.readthedocs.io/en/main/rules.html
- ASML, conceptual lithography flow: https://www.asml.com/en/technology/all-about-microchips/how-microchips-are-made
- Computer History Museum electronics history: https://www.computerhistory.org/siliconengine/timeline/
- Princeton EHS, piranha solutions' specialized residue-cleaning purpose and hazards: https://ehs.princeton.edu/laboratory-research/chemical-safety/chemical-specific-protocols/piranha-solutions
- NIST chemistry reference values: use validated primary property tables per material, citing in the game's codex where authored. None of the above implies real semiconductor-grade accuracy is achieved by this game model.
