# 02 — AXIOM: RUST / WGPU ENGINE ARCHITECTURE
**Revision:** U1 — 2026-10-26 | **Scope:** executable design decisions with named alternatives/spikes | **Important:** sizes/budgets are starting hypotheses, not measured values. Existing repository conventions take precedence until reviewed.

## 1. Performance philosophy and ground rules

The engine must represent an enormous 3D destructible planet, elaborate local materials, a dense industrial base and a mechanically complex creature **without processing all stored matter every tick**. Computation should scale with visible surfaces, active material regions, active network boundaries and causal events. The canonical world remains exact about meaningful edits; derived geometry, collision, lighting and remote simulation may approximate detail under declared contracts.

**Five independent resolutions:** (1) persistence/storage, (2) rendered geometric LOD, (3) material numerical update, (4) collision/navigational precision, (5) nested machine/editor scale. Never treat them as one mutable chunk LOD. A 1 m construction block can have a true locally editable 1/8 m architectural eave while a semiconductor design uses a separate micrometer/nanometer-inspired *nested editing domain* with summarized machine behavior in the overworld.

**Hard invariants:** unique authoritative owner for every quantity; correct world-space volume under refinement; deterministic accepted edit ordering where required; no stale mesh/collider publication; no lost saves/edits; no persistent world gaps; bounded frame/job/VRAM cost; visual effects do not create authoritative material; physical simulation cannot silently invent resources during sleep/wake.

**Avoid:** Bevy rewrite, dense global voxels, universal SPICE/Navier–Stokes, mesh-per-cell or rigid-body-per-cell, unconditional GPU readback, shader-on-every-voxel, authoritative geometry derived from distant heightfields, a single giant octree for all systems, simulation of all machines at 60 Hz, and a data-driven system that hides unmeasured O(N²) traversal.

## 2. Architectural layers and ownership

```
Player / NPC / scripts → validated Commands / capability checks
                                  │
                       Authoritative tick + ordered transactions
                                  │
          ┌───────────────────────┴──────────────────────┐
          │                                              │
     World Kernel                                   Object / Entity authority
 (generator + sparse exact edits)             (dynamic voxel bodies, inventory,
          │                                      machines, crops, creatures)
          └───────────────────────┬──────────────────────┘
                                  │ world-change journal + typed events
        ┌─────────────────────────┼─────────────────────────────────┐
        │                         │                                 │
 Physics/material islands    Compiled port networks         Derived spatial caches
 granular/fluid/heat/...      power/belts/fluids/VM         mesh/collision/nav/light
        │                         │                                 │
        └─────────────────────────┼─────────────────────────────────┘
                                  │
                read-only / bounded-snapshot GPU staging
                      wgpu visibility/render/compute
                                  │
                      pixel-art presentation / UI
```

### Proposed Rust boundary modules (not necessarily separate crates)

- `world_coords`: absolute integer coordinates, scale hierarchy, valid transformations and relative camera coordinates.
- `world_kernel`: canonical occupancy/material ownership, queries, edit transactions, persistent overlays.
- `world_store`: segment/chunk file format, atomic checkpointing, version migrations, journals.
- `world_plan`: generator semantic passes, civilizations, POIs, secrets, place graphs; pure deterministic generation.
- `world_jobs`: budgeted scheduling, duplicate suppression, dependencies, revision checks, cancellation.
- `world_materials`: property tables, reaction rule indexing, cellular islands and aggregate numerical fields.
- `world_bodies`: rigid assemblies, connectivity, fracture, joints, collision proxy ownership.
- `world_networks`: typed ports, machine graphs, power, heat, fluids, belts, kinetic, item transactions.
- `world_vm`: common typed IR, runtime sandbox, authoring frontends for glyphs and machine programs.
- `world_ecology`: populations, creature biology, growth, dynamic nav, utility AI, crews.
- `world_render`: WGPU device/feature caps, geometry arenas, culling, LOD residency, lighting and VFX.
- `world_assets`: asset metadata, atlases, Blender conversion, schema compilation, validation.
- `world_diag`: capture receipts, input/event replay, CPU/GPU scopes, performance/failure artifacts.
- `app`: player, UI, input, camera, audio, save/load presentation, mode dispatch.

Consolidate trivial modules and preserve existing code where the actual repository proves they work. Cargo crates are not themselves a quality metric. **Dependency graph must be acyclic at the core library level; runtime feedback among interacting simulations is allowed through typed tick stages**, not circular Rust crate imports.

## 3. Spatial coordinates, voxel scales and planet topology

### Canonical address contract

Use signed 64-bit coordinates for base spatial indexing, with a documented base **quantum** and aligned extents. Proposed first test: 1 m primary building module, with dyadic refinements to 0.5, 0.25, 0.125 m (and optionally smaller only in special volumes). A physical region is `[min,max)` in absolute world units and owns a material occupancy distribution. `RegionKey` includes origin and scale level; mixed-resolution neighbors share exact integer-aligned faces.

**Important:** a 0.125 m cell occupies 1/512 the volume of a 1 m cell. One cubic kilometer at 0.125 m would be 512 billion cells. Do **not** allocate such space globally. 1 km³ at 0.25 m is 64 billion cells (256 GB at 4 B/cell before metadata).

Proposed type sketch:

```rust
struct WorldCellKey { origin_units: I64Vec3, level: u8 }
struct VolumeAabb { min_units: I64Vec3, max_units: I64Vec3 }
struct RegionRevision { region: RegionId, revision: u64 }
struct OwnedMaterialVolume { material: MaterialId, units3: u64 /* or wider */ }
```

Numeric units, fixed-point widths, overflow checks and huge-world extent must be chosen in an ADR. Use u128/wider intermediate multiplication where needed. Perceived camera movement uses chunk-local positions plus origin rebasing; GPU matrices operate on small relative f32 values. Avoid subtracting huge f32 world coordinates directly. Physics uses local islands, rebased anchors and robust contact tolerances; authoritative world positions remain stable.

### World topology research gate

The lore specifies a **single planet with a ring and deep core**, but that does not mandate globally curved voxels on milestone one. Choose by ADR after a feasibility spike:

- **A:** vast planar/local-coordinate voxel continents with planetary framing; easiest for Minecraft-scale build, huge procedural underground, distant horizon.
- **B:** cube-sphere/patch-based planetary voxel topology with local tangent frames and spherical gravity; more complex boundaries, physics and caves.
- **C:** hybrid geodetic world regions joined through explicit geography/orbital transfer; richest long-term route at manageable initial cost.

Do not promise physically seamless digging from surface to opposite hemisphere or literal one continuous mantle-to-orbit render without choosing/benchmarking planet topology. Ring/space are independent simulation domains with explicit transform, atmosphere transition and transfer semantics.

## 4. Storage: sparse exact world + compressed derived summaries

### Canonical layers

1. **Procedural base** `base(seed,generator_version,region)`: immutable/deterministic, generated on demand. Older generator versions remain addressable.
2. **Persistent edit overlay:** canonical placed/removed/sculpted material, fluid/source changes and construction at authored resolution. Sparse, journaled, exact. Edits never disappear merely because a visual LOD cannot represent them.
3. **Persistent dynamic objects:** machine inventories/heat/charge, voxel assemblies, organisms, ownership and dungeon mechanisms with stable IDs.
4. **Derived caches:** occupancy mip summaries, coarse geometry, meshlets/quads, collision, nav, light, volumes. Versioned and replaceable.
5. **Runtime islands:** hot dense bricks/field pages only for locally active numeric simulation; activation is a state conversion, not a second permanent copy of world authority.

**Brick candidates:** 8³ or 16³ dense hot bricks; 32³ or 64³ grouping for streaming/mesh jobs; homogenous regions stored as one material/phase; palette-bitpacking, RLE or dictionary as appropriate for cold data; compressed edit spans and bitmask occupancy. Do not assume a universal sparse octree wins hot kernels. Benchmark flat hash→brick indices, paged trees and hierarchical masks.

**Brick header (illustrative):** region+scale key; compressed-form enum; palette reference; dirty masks; wake flags; per-domain revision and dependency bounds; active surface/contact mask; packed physical property table references; optional field slab handle. Avoid per-cell heap allocation. Hot material state may use SoA arrays, bitplanes and compact material IDs. Separate persistent state, immutable material definitions and transient solver scratch.

### Transaction rules

A `WorldCommand` executes through validation, authorization/resource reservation, deterministic conflict ordering, apply-on-authoritative-state, stable object allocation and change record emission. Adjacent edits are coalesced for **derived rebuilds**, not silently combined when it would alter game-visible timing.

```
validate(Command)
  → reserve resources and ownership
  → apply to canonical world/objects (atomic gameplay step)
  → append journal + version changes
  → invalidate dependent spatial summaries
  → enqueue bounded derived rebuilds
  → publish only if source dependency revisions are current
```

Every artifact includes **neighbor/dependency revisions** as needed. A chunk mesh built from neighborhood voxels cannot be considered valid using only its own chunk's revision. A powered gate must stop when its line is cut even if its visual mesh job is pending. Failed transaction restores reservations. All transfers of fluid/item/energy have one source and one destination with atomic or explicitly delayed commit.

### Save format

Use versioned manifests plus append-only write-ahead journal/delta files and atomic snapshots (write temp + sync/rename at safe intervals). Stable identifiers separate world objects, voxel region, entities, machines, graph/network, design revision and GPU handle; GPU handles never persist. Store seed, generator version, content pack version/hash, mod registry mapping, RNG streams, relevant transactions, background simulation events and binary data integrity CRC/hash.

Persistence test matrix: incomplete save/crash recovery; refines→unload→reload→coarsen; generator 53/54/55 regression where applicable; two agents edit adjacent brick seams; fast teleport while saving; mobile assemblies crossing region boundaries; old saves never overwritten without backup/migration prompt. Migrations preserve player ownership and resources and must be idempotent.

## 5. Hierarchical rendering and LOD correctness

### Baseline: GPU-driven raster, hybrid geometry only where valuable

- Cubic structural terrain: packed face/quad representations; binary greedy meshing where shape/material constraints allow; compact vertex pulling or equivalent; atlas/material id, normals and lighting data in small GPU-friendly records.
- Natural/organic smooth surfaces **only when art/geometry demands them**: Marching Cubes + Transvoxel for compatible 2:1 boundary transitions, or Dual Contouring with proven transition handling. Do not force SDF on cubic architecture.
- Distant proxies: same canonical generation/edit data, conservative mesh silhouette and occupancy summaries; no separate unrelated heightfield pretending to be identical to near cave/overhang/arch terrain.
- High-detail special objects: curated low-poly models, sparse voxel bricks or optional direct volume traversal by measured choice. Most 3D assets can be lightweight meshes with honest collision rather than hundreds of destructible fine voxels.

### LOD-residency contract (critical for reported horizon problems)

1. A visible spatial region must have a resident **valid** parent proxy or appropriate leaf mesh (except intentional visibility clipped by camera/frustum/occlusion).
2. Request new child LOD; build mesh from versioned world state with neighbors; upload; verify GPU residency; transfer visible coverage; retire parent only after replacement is usable.
3. If a chunk becomes unloaded, the parent/fallback must cover it before child retirement. Never present an unmodified base proxy on top of player edits that change the visible shape.
4. Use neighbor level restrictions and transition/stitch geometry. Morph geometry/normals or use controlled fades without z-fighting, cracks, light leaks and misleading collision silhouettes.
5. Persistent macro-structures (arches, caves, bridges, towers, trees) must remain recognizable as parent features. LOD does not erase significant topology because a heightmap lacks it.
6. Track temporal histories; LOD swap, large terrain edits, exposure changes and rebasing invalidate/reactivate relevant TAA/upscaler/Hi-Z history conservatively.

**Anti-pattern:** radial discard + sinking the horizon mesh through resident chunks. It hides cracks rather than agreeing on geometry. The known legacy tree LOD disconnected canopies must be diagnosed at generation, proxy/culling/parent or alpha stage, never masked with fog.

### Draw pipeline

```
CPU: canonical view → streaming candidates → job budget → shared GPU arenas
GPU: instance/region bounds → frustum → previous/conservative Hi-Z visibility
   → visibility compaction → indirect draw stream → depth/material/shadow
   → lighting/volumes/transparent effects → temporal resolve → UI
```

Use conservative bounds with camera acceleration, edits or stale occlusion history. Indirect multi-draw/count and GPU timestamp features are optional via `wgpu::Features`; feature probe and fallback. Mesh shading/ray queries in `wgpu` 30 are experimental and cannot be core requirements. The renderer may run on native D3D12/Vulkan; OpenGL/no-compute fallback need not reproduce high-tier effects if it provides safe reduced fidelity. No need to rebuild all geometry into GPU meshlets before first proven indirect-draw baseline.

### Memory/resource model

Persistent pooled GPU geometry and instance arenas, versioned allocation handles, deduplicated material/texture atlases, upload staging ring buffers, deferred frees after fences/completion, per-frame upload budget, high/low watermarks, compact residency telemetry. When requested LOD exceeds memory, preserve valid coarser coverage and prioritize view-critical surfaces over distant ornaments. No per-chunk buffer creation/destruction every edit.

**Asset material scheme:** intentionally authored pixel albedo; material masks (wetness/roughness/metal/heat/rune) and stable textures; atlas padding and mips; world-aligned texel grammar; no photorealistic noise. Near pixel materials and distant filtered mip levels balance pixel identity with anti-shimmer. Decals and variations cannot jitter in world coordinates.

### Lighting & visual style

Maintain a high-quality conventional raster foundation: stable directional/point shadows, selected contact AO, sky/atmosphere, physically coherent fog, screen-space or probe-based indirect light with bounded updates, stylized energy emission. Distinct cave light compositions; avoid uniformly yellow torch-washed underground stone. Emissive effects affect surrounding light as budgets allow, but gameplay material temperatures do not depend on shader color. Ray-based GI and mesh shaders remain optional research paths.

Material classes: stone/strata, wood, metal/corrosion, ceramic/glass, fluid/wet, crystalline/aether, fungus/biology, arcane/kinetic field, soil/organic, cloth, atmospheric/volumetric. State may alter appearance: temperature, moisture, corrosion, charge, failure, mana and field resonance. Support close/medium/far effects levels. The default block shader is inexpensive.

**Shields:** use original localized Fresnel/event ripples and anchored material pattern, bounded hit records and per-emitter charge; kinetic facets, electromagnetic conductive branches, arcane glyph lattice, biological membrane, crystal fractures. Visible topology follows emitter anchors and damage, not an always-on neon hex sphere. No verified publicly available proprietary Mycopunk shield shader source should be claimed.

## 6. Active sparse material simulation and adaptive time

### Material registry

Material definitions describe density, mechanical strength, hardness/fracture, flammability/ignition, thermal capacity/conductivity/phase change, permeability/viscosity where relevant, electric conductivity, chemical family/reagent properties, optical/emissive and *explicit fictional* arcane couplings. Persistent cells refer to shared material IDs; dynamic fields store only active state (heat/temperature derived from enthalpy, damage, moisture, composition, charge, pressure when supported).

**Reaction matrix rule:** All material combinations have a defined resolver result (often inert/no effect). Reactivity rules are indexed by tags/properties/conditions and compiled into fast lookup dispatch. Do NOT enforce >=30% of all N² material pairs as nontrivial reactions; it would produce expensive, incoherent chemistry and block content growth. Instead test *curated* interaction breadth: each signature material must have at least several meaningful relations and no unexplained gaps in intended chains. Reaction fronts/contact surfaces are a better work metric than all interiors.

### Solver families (bounded and honest)

| Domain | Candidate solver / representation | Hot activity | Cold/sleep contract |
|---|---|---|---|
| Static solid | Sparse immutable occupancy/material | Event-driven edits | Zero routine per-cell updates |
| Sand, snow, rubble | 3D cellular local movement with density/friction/piles; ballistic ejection for fast sprays | Active surface/bricks | Stable pile sleeps |
| Water/lava locally | Conservative finite-volume/cellular pressure/flux or an experimentally chosen scheme | Active interfaces, moving front | Coarse basin/tank or mass/level state |
| Large reservoirs | Compressed connected cavity/volume, level, pressure head and links | Changing connectivity/valves | Aggregated conserved volume |
| Gas/smoke | Sparse transport and pressure zones; visual particles separated | Moving fronts and occupied regions | Coarse compartment state |
| Heat/fire | Enthalpy/material interfaces; front propagation and thermal events | Gradient/interface regions | Analytic cooling/coarse time step where valid |
| Chemistry | Contact-driven reaction registry with reactant/product debit and heat | Active contact sets | Dormant concentrations retained |
| Structural | Incremental connectivity + selective load/fracture checks | Local supports and detach | Stable loads/structure graph |
| Organic growth | Resource/temperature/light-limited growth events | Growth fronts and nearby organisms | Scheduled coarse biological change |
| Radiation/field | Bounded spatial source/attenuation, solver chosen per class | Field emitters, modified topology | Cached regional field/mode |
| Arcane resonance | Cheap source/mode graph, sampled intensity and field properties | Changed resonators, sustained spell fields | Cached modes and periodic refresh |

Avoid a global PDE solver for the entire world. In world interfaces where precise pressure/fluid momentum matters, activate a local domain with numerical tolerances and bounded complexity; elsewhere approximate in physically intelligible terms.

### Conservation at mixed resolution

A cell's volume is `(world_size(level))³`. Restrict fine-to-coarse by conserved extensive quantities (mass, enthalpy, momentum, species inventory) and recompute intensive quantities (temperature, pressure) from totals and volume/material parameters. Do **not** simply average temperature when different heat capacities/densities matter. Across coarse-fine face boundaries compute one authoritative integrated flux and debit/credit both sides exactly (integer/fixed or tolerance-accounted numerics).

**Prolongation:** refining a coarse mixed cell must reconstruct plausible state without inventing matter/energy. Preserve exact micro-edits in overlay and keep subcell occupancy detail where needed for topology. If hidden microscopic configuration is ambiguous, use deterministic reconstruction plus conserved reservoirs; only infer what model permits. A physics LOD transition cannot permanently erase a player-created 0.125 m tunnel.

### Time model

- Authoritative fixed simulation tick proposed 60 Hz for gameplay actions; test 30/60 alternatives on hardware. Rendering interpolates; some local collision/projectile/force updates substep where stability requires.
- Material islands and compiled networks use event-driven ticks, variable substeps or coarser batch advancement according to declared validity. Separate **simulation time** from wall-clock/UI time. Offscreen factories advance using preserved inputs, capacity and bounded scheduled events, not speculative free production.
- GPU compute suitable for large coherent active regions, not small random networks or gameplay logic merely because GPU exists. Keep compact event exchange; avoid full state readback every frame. Deterministic CPU reference for rules requiring replay/multiplayer; visually rich GPU particles need not be bit-identical across devices.
- Physics island activation priority depends on potential consequences, velocity, hazard, player proximity, visibility, topology changes and available work budget. No indefinite starvation of saves or gameplay-critical pending events.

### Parallel correctness

Noita's 2D four-phase checkerboard update is a useful reference but not a universal 3D solution. Define conflict-free writes with region ownership, bounded neighborhood/halo, multi-phase spatial coloring or buffered cross-region transfers and barrier synchronization. For 3D CA, update phase count follows write radius and dimensions—prove it, don't assume four. Maintain seed-independent/discrete ordering where required. Use property tests, ThreadSanitizer where practical and randomized boundary stresses.

### CPU/GPU numerical authority

Use exact integer identities and ownership, integer material counts/volumes, quantized energy and charge where gameplay budgets demand conservation. Not all physics state must ban floats: local pressure and rigid-body simulation can use f32/f64 with bounded accuracy, deterministic step ordering where feasible, and quantized authoritative commit. Cross-platform bit-exact GPU floating-point is not a realistic universal promise. Explicitly describe numerical error and conservation policies in each solver's SDD.

## 7. Structural physics, moving voxel assemblies and collisions

**Static world** retains simple collision proxies derived from authoritative occupancy; Rapier/Parry's voxel collider can be evaluated as a starting point, including local `set_voxel` updates and internal-face awareness. Version-check collision region changes, and do not block the render frame rebuilding million-triangle colliders. Shape choice is heterogeneous: voxel collision for cubic static surfaces, triangle/compound simplified shapes for irregular/moving geometry and exact local volume queries for editing.

**Structural stages:** local edit → anchored connectivity/support dirty region → constrained support/update → detach identified component (if relevant) → create moving object-local voxel volume with independent transform → broadphase simple body → narrowphase relevant contact region → optional secondary fracture into limited children → settle/fuse/recover/reclaim static ownership. Prevent material double-ownership when turning terrain into a moving structure.

**Incremental topology:** union-find is useful for newly added structural connections but does not alone support deletions. Use region-local flood/split checks, cached component boundary connections, and hierarchical deferred global reconnection where necessary. Limit huge collapse fanout via cohesive chunks/aggregated debris; independent rigid bodies only for components with visible/gameplay consequence. Conservation and energy dissipations are modeled within chosen physics resolution.

**Movable craft:** ships, trains, rovers, airships and golems can contain editable object-local material, explicit utility networks and compartment summaries. Damage localized to physical segments affects networks, sensors, motion or hull pressure. Crew orders act on actual reachable modules; pathfinding uses object-local nav and connectors to world, rather than voxel-by-voxel AI searches across every ship compartment each frame.

**Force/collision consistency:** physical dimensions and mass depend on real volume, density and additional components, not voxel count. Change of grid scale doesn't modify object momentum or resistance. Broadphase overlaps with island bounds; narrowphase only visible/active surfaces; reliable raycast from camera and hands. Physics authority can be separately ticked from visual smoothing; avoid dynamic polygon rebuild at every minor texture variation.

## 8. Machine networks: compiled sparse processes instead of per-voxel updates

**Rule:** world voxels author machine *geometry and connections*. Actual machine functions compile into compact ports, graphs, segments, tanks, thermal zones and process state that do not require updating every shell block.

| Network | Core abstraction | Behavior under edit |
|---|---|---|
| Item belts | Packed transport-line runs with ordered gaps/items and shared progression (Factorio inspiration) | Split/merge affected run; preserve items and order |
| Inventories | Unique inventory/lot owners, capacity, transactional transfers | Broken container drops or retains exact items according to explicit policy |
| Fluid pipes | Control volume nodes, hydraulic resistance and pumps; typed species/pressure/temperature | Cut port disconnects; inventory fluid remains in real surviving segments |
| Mechanical | Shafts, drive/bearing edges, torque/speed and rated losses | Shaft failure severs branch and changes machines/load |
| DC electrical | Typed positive/return paths, resistance and source/current limits, small bounded nodal/MNA solve | Wire removal invalidates branch immediately |
| AC electrical | Averaged RMS/frequency, phase/impedance, rated transformers and conversion losses | Recompute affected island, not world-wide waves |
| Heat | Energy/enthalpy nodes with thermal links and emitters | Wall area, contact or insulation change alters correct conductance |
| Mana/aether | Field sources, capacitors, channels, resonator graphs, explicitly costed machine ports | Damaged focus reroutes/stops local magical flow |
| Logic/signals | Evented typed IO + VM/controller program | Program runs bounded instructions, cannot directly mint energy |
| Chemical batches | Ledger of mass/species/temperature/catalysts/process stage | Real changes to heat/input affect yield and throughput |
| Biology | Growth compartments, nutrient/oxygen demand and scheduled traits | Conditions affect growth and harvestable product |

**Port schema** (RON or equivalent data-driven format) must have stable `(ObjectId,PortId)` identity, domain, direction, capacity/rating, connect rules, optional channel/phase and pressure reference. Explicit converters couple domains using debit→credit transactions. Not every energy pair needs a converter. A closed loop of *passive/ordinary* converters must have accounted inefficiency and cannot generate unbounded energy; fictional magical source behavior is separately defined, metered and constrained.

**Sample electrical physics:** 48 V source, load 4.8 Ω, round-trip conductor 0.2 Ω → 9.6 A, 46.08 V across load, 18.432 W conductor loss. Show meter diagnostics for disconnected supply, open return, overload, brownout, thermal fault. Resistive conductor should heat with `I²R` within declared model. Ordinary convenience paired-conductor blocks reduce placement tedium without inventing an invisible circuit return. Passive transformer cannot act on DC; use rated DC-DC converter separately.

**Conveyor complexity:** join many ordinary belts into long transport lines so stable packed runs update primarily at their boundaries; transfer at inserters/splitters/edits. Rendering close-by items may be instanced while authority remains compressed. Splitting/removing a belt midway retains exact cargo ownership and reserves placements. Never allocate one rigid body for each throughput item.

**Multiblock example:** a variable-size furnace is a validated connected chamber with refractory lining, burner/heating surface, volume, geometry-dependent loss, actual inputs/outputs, exhaust and a small thermal state model. Small and large designs should differ for physically comprehensible reasons. Cache structure validation until touched, mark it invalid safely during reconstruction, and preserve input batches/charge when compatible portions survive.

**Digital design:** functional module editor → validated typed nets/macros → characterized performance/power/heat/failure → saved mask/version → fabricable wafer lot. Use nested 2.5D fabrication grids. The actual processor's computational capacity can determine bounded VM throughput, memory or I/O, but not depend on emulating every transistor toggle in ordinary play. Timing during user interaction follows gameplay-compressed but real causal process stages.

**Matter compiler:** feedstock reservoir, real mass/species, authoring blueprint, bounded assembler rate, power costs and device quality; anti-cheat and mod consistency. Complex items may consume more specialized processing/correction operations. **Do not use raw compression entropy as a mandatory 'bits-per-block cost'**: random noise/uncompressible images would become arbitrary economic exploits and penalize decorative builds.

## 9. Shared VM, typed world operations, magic and player tool code

### IR / sandbox

Two UI notations—engineer ladder/text/blocks and Chorister glyph/resonant geometry—compile to a shared typed small-step intermediate representation where semantically valid. Founders' artifacts use extensions gated by content capabilities. The bytecode runtime does arithmetic, fixed vectors, branching, time/event wait, sensor reads and typed `request_action` calls. It does not directly reach OS APIs, render buffers, raw voxel pointers or inventory memory.

**VM authority contract:** a program runs with object identity, permissions, precharged resource budgets, instruction fuel, memory bound, per-tick event cap, allowed port references and deterministic event ordering; scheduling respects global server/per-world budget. Opcodes may request `HeatVolume`, `MoveMaterial`, `LaunchProjectile`, `SetPort`, `EmitField`, `ReportSensor` etc. The domain solver validates the operation and debits energy/material as appropriate. An aether program cannot create a physical projectile without authorizing the requisite mass/energy/fictional source.

**Noita-style wand engine:** fixed wand capacities/statistics; ordered/shuffle deck rules; triggers/timers; calls and wrapping; multi-action frames; intrinsic effects; material modifiers; explicit execution stack/state. Test nested triggers and chain reactions. Power is limited by fuel, payload cost and event budgets, not arbitrarily denying ingenious effects. Make spell graphs and tooltips inspectable: cast order, expected costs, cooldown, trigger interactions, physical world target.

**Programmable robot/crew scripts:** same IR where feasible but different permission set and domain APIs. Avoid building a second unrelated interpreted language for every NPC and machine. Utility AI selecting goals remains a separate optimized runtime.

**Mod interfaces:** RON/data schemas for material/machine/creature/spell defs, codegen/scaffolds, validators for IDs, ports, physical dimensions, rendering palettes and compatibility. Hard-coded efficient kernels can be shared by data-driven definitions; do not prohibit small `match` statements on closed hot-loop enums merely in pursuit of 'zero matches'. Sandboxed WASM optional for complex events, with capability and fuel restrictions. No WASM call per material cell. Mods must be versioned, migrated and attributed in performance diagnostics.

## 10. Dynamic world generation and physical secret placement

**Pure generation stages:** geological tectonic/climate model → hydrology/aquifers/fault/ore/arcane field → ecology and regions → historical civilization footprint and infrastructure graph → named landmarks and site connectivity → architecture `PlacePlan` → material/voxel stamps → deterministic physical validation → secret/hint distribution.

**Generator versioning:** keep generators 53 and 54 behavior for old saved worlds; add generator 55 only when canonical place generation changes. Generated output must be independent of chunk visit order and rendering detail requested. Use named deterministic RNG streams keyed by seed/feature coordinates; never use global RNG advancement dependent on thread scheduling.

**Functional POIs:** dams hold and channel water, mines have supports/rails, towns connect to resources, Chorister spaces contain acoustic/rune structure, Founder archives expose actual decipherable records. Distinct visual civilizations share world-system physics. Inter-place quests/secrets arise from explicit graph/hint placement validation, not probabilistic object scatter.

**PlacePlan:** site; landscape approach; main/secondary masses; room/circulation graph; protected 3D sightline/traversal volumes; support geometry; functional machine/puzzle/resource anchors; light and climate plan; dressing sockets; secret clues; LOD-critical silhouette records. Stamp from macro to micro and validate occupancy collision and all required routes with player/creature movement constraints.

**Secret rule generation:** each world-scoped mystery has seeded predicates, several hints and evidence locations, optional experiments, physical outputs, and a proven reachable route. Not all secret mechanisms are per-seed random chemistry; broad material laws remain understandable. Where seed varies transmutation recipes, build a solver test to establish that clues specify a bounded hypothesis space. Non-scripted emergent discoveries do not need pre-authored hint sets.

**Nested worlds:** pocket spaces/domains and electronic virtual worlds may share region and save interfaces, but different boundary physics, resource transfer, time scale and avatar presence demand explicit rules. A 'virtual space in an in-world computer' can use the same content/editor technology without magically granting arbitrary physical items to the parent world.

## 11. Dynamic AI, navigation, ecology and procedural raids

AI navigation uses hierarchical cell/region portals + local traversability; different nav modes for ground, flying, swimming, burrowing, wall climbing and vehicle interiors. Preserve portals through generated caves/arches and player excavations. Spatial edits invalidate only relevant summaries; no global navmesh rebuild per mined block. Distant populations use scheduled ecological aggregate state, not thousands of individual walking NPCs.

Senses derive from real sightlines, audible events/local acoustic portal summaries, heat/smell where justified, vibration/arcane sensors and faction knowledge. Utility AI agents have tasks and needs; use cheap evaluation scheduling based on actual decision points. Crews can diagnose subsystem damage and execute repair/evacuate orders if physical path and materials exist. Combat behaviors telegraph physics and functional weak points.

Enemy spawning must have visible/ecologically relevant origin (nest, migration route, portal, habitat), with localized disturbance budgets. Player can intercept, farm, reduce, cultivate or reroute threats. Persistent secure bases should not have teleporting random enemies. Difficulty comes from exciting regions and operations, not an ever-ticking global spawn meter.

## 12. CPU/GPU budget scheduler, backpressure and precision

**Work category** | **Priority and constraint**
:---|:---
Critical | input, authoritative local edits, player contact, attack outcome, critical nearby hazards and core network severance—must finish coherently or enter a safe constrained state
Interactive | visible nearby meshing, fluid/reactive fronts, nearby NPC decisions, high-value lights and audio—budgeted and responsive
Background | remote ecology/factory integration, generation, save snapshots, distant nav, streaming compression—service without starvation
Opportunistic | far aesthetic refinement, extra indirect-light bounce, visual particle density, advanced cache compaction—drop/reduce freely

A job has stable identity, input dependency revisions, cost estimate, deadline/importance class, memory and upload budget, cancelation state and permitted result destination. Coalesce obsolete geometry jobs. Use an owner-execution model for conflicting simulation writes, work-stealing for independent tasks, explicit staging buffers and bounded queues. A slowdown reduces cosmetic fidelity and far LOD quality before gameplay truth; conservative parent representation preserves terrain coverage.

**Dynamic adjustment:** a governor monitors CPU+GPU frame time, 95th/99th percentile, upload delay, VRAM occupancy, simulation backlog and persistence lag. It adjusts separate budgets subject to hard minimum safety requirements, hysteresis and no quality oscillation every frame. An overheating machine does not silently stop being authoritative under heavy GPU load.

**Precision and parallelism:** no cross-branch use of unstable floats as save identity; fixed-point/integer extensive quantities where appropriate; gravity/contact local physical units; log numerical residuals on iterative solvers. Carry interface ownership and material conservation through sleep/coarse/fine/wake. Separate render motion vectors from true world transforms for temporal stability.

## 13. Performance target matrix and benchmarking (proposed, unverified)

**Three target classes (definitions require actual hardware discovery):**

- **H0:** low-spec integrated GPU/CPU configuration; reduced draw distance/volumetrics and limited active material bricks; target playable 30 FPS at 720p–1080p under defined reduced settings.
- **H1:** mainstream current desktop discrete GPU; target 60 FPS at 1080p in typical scenes and stable factory demos with defined active regions.
- **H3:** development-class high-end RTX 4090 (24 GB) or equivalent; **stretch** 120 FPS 1440p ordinary exploration, and **stretch** 60 FPS in a *defined* heavy physical scene. These are aspiration targets, not measurements.

No performance number is valid without hardware, resolution, graphics settings, camera path, scene seed, world state, active voxel count, material interaction set, machine counts and duration. Separate initial load, streaming travel, stationary factory, cave combat, structural collapse, flood and soak. Add warmup, repeated runs and cold-cache variants.

**Instrumentation:** CPU tracing (`tracing`/Tracy etc.), WGPU timestamp queries/profiler where supported, allocator metrics, GPU buffer arena/fragmentation, quads/triangles/instances, vertex pulling occupancy, world-gen/mesh build time, uploads/readback, active brick counts, island solver cost, save bytes and queue wait. Store capture receipts with executable hash + shader/asset fingerprint. A 'faster' frame that renders a hole or drops fluid is a FAILED run.

**Acceptance workloads:**

1. Horizon fly: 24 km-style vista if actual representation supports it; fast flight over mountains/arches/edited buildings; no near/far cracks, proxy holes or stale edited terrain.
2. Mixed LOD edits: dig a 0.125 m passage at a 1 m/0.5 m seam, move away, unload/reload and return; correct rendered/collision geometry and conserved removed volume.
3. Flood/chemical: dam breach through caves into a machine room; correct mass ledger, bounded simulation, no missing chunks or explosion of rigid bodies.
4. Destruction: damage tower supports, detach coherent body, fall into liquid/electrical site, save/reload, no duplicated mass and reasonable p99.
5. Factory: large packed belt runs, real loads, multi-source power, fluid tanks, repeated split/merge; transfers conserve items and charged energy.
6. Ancient dungeon: mine beams, pit shaft, library nave, hidden doors and multiple routes remain traversable with actual player capsule and lighting.
7. Overnight soak: large edit/save/explore/return history, VRAM pool stable, no unbounded queue growth, no dirty-save loss.
8. Mods: add a new data-only material and machine/spell recipe, validate coverage and measured behavior without unrelated source changes.

## 14. Failure taxonomy and remediation requirements

| Failure | Root-cause diagnosis | Prohibited workaround |
|---|---|---|
| Horizon gaps | Residency ownership, proxy topology, neighbor transitions, revision errors | Depth-sink, radial discard, thicker fog |
| Disconnected tree crowns | Parent LOD generation or mesh/culling | Add random leaf particles instead of fix |
| Coarse roofs at 1 m | Missing true fine geometry/physical proxy | Photogenic mesh with incompatible collision |
| Yellow dungeon lighting | Light color/intensity, albedo, exposure, tone map, indirect | Tint stone blue solely to cancel shader bug |
| CA race / duplication | Cross-chunk ownership, scheduler, boundary transfers | Silently delete overflow material |
| Factory dupes | Inventory/network authority conflict, transaction commit | Reconcile inventories from display state |
| GPU stalls | Too many buffers, uploads, readback, allocation churn | Drop near chunks or stall gameplay inputs |
| Wands look generic | Silhouette grammar, tiny-size material/animation, art production | More bolts, glow, bevels or heavy head |
| Boring generated dungeon | Protected voids, role grammar, functional anchors, light, traversal | More random containers/enemies |

## 15. Research-to-implementation decisions to measure

**Before changing major engine architecture, run isolated spikes:** dense 8³ vs 16³ active bricks CPU/GPU; volume/surface reaction kernels; greedy quad vs direct volume rendering; CPU SIMD active-CA throughput; conservative mixed-grid transport; structure connectivity under removals; real collision API vs custom static proxies; one plausible resonance graph field; content-driven procedural secret solver; novel artifact rendering 2D/3D hybrid; long belt compression; job scheduler hysteresis. Each spike has fixed scenes, results, a keep/reject decision, source ownership and an ADR.

**References (verified conceptual anchors as of 2026-10-26):**
- Noita GDC physics+design: https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design/
- Transvoxel 2:1 transitions: https://transvoxel.org/
- wgpu 30 docs and experimental capability notes: https://docs.rs/wgpu/30.0.1/wgpu/
- WGPU GPU timestamps example: https://wgpu.rs/doc/wgpu_examples/timestamp_queries/index.html
- Rapier voxel collider: https://docs.rs/rapier3d/latest/rapier3d/geometry/struct.Voxels.html
- Factorio transport-line optimization: https://factorio.com/blog/post/fff-176
- Official Aseprite asset automation: https://aseprite.com/docs/cli/

**Research freshness warning:** Some search engines surface publications with future dates (e.g. a December 2026 dynamic NanoVDB paper). Since this spec is dated **October 26, 2026**, such documents are *not evidence of currently available technology* and must not be represented as implemented/published as of this date. Sparse active-grid processing is still a worthwhile experimental design idea; evaluate independently.
