# 06 — REFERENCE DATA CONTRACTS, ALGORITHMS AND ACCEPTANCE SCENARIOS
**Status:** illustrative design-level code/schema examples, **NOT** current repository types or executable fixtures. Intended to guide system-level SDDs and integration tests after repo reconnaissance. All dimensions, bounds and formats require ABI/serialization review.

## 1. Stable IDs, coordinate transforms and lifetime ownership

```rust
// Sketch only: real names, integer quantum, traits and serialization are repo-defined.
struct AbsolutePos { base_units: [i64; 3] }
struct CellExtent { origin: AbsolutePos, log2_size_units: u8 }
struct WorldRegionId(u64); // identity independent of streaming array index
struct ObjectId(u128);      // persistent identity, generated deterministically or saved
struct MaterialId(u32);    // registry-versioned, migrated by stable string ID
struct WorldRevision(u64);
struct LocalPortId(u16);
struct Tick(u64);

struct WorldChange {
    revision: WorldRevision,
    bounds: (AbsolutePos, AbsolutePos),
    kind_bits: u64,
    touched_regions: Vec<WorldRegionId>, // compacted journal; not hot path heap / voxel
}

struct ArtifactRevision {
    own: (WorldRegionId, WorldRevision),
    adjacent: Vec<(WorldRegionId, WorldRevision)>,
    registry_generation: u64,
}
```

**Design rules:** 64-bit or 128-bit IDs are stable across reload, versioned registries remap material IDs, derived IDs are never player save IDs. A world edit modifies canonical data *once*. GPU allocations and materials are generated from stable source IDs; they do not own matter. Ghost/resident/LOD representations have explicit owner to prevent duplicate visible surfaces and double CPU simulation.

**Voxel-size invariants:** Each cell occupancy is measured in exact world units; face contact uses rational dyadic alignment. Restriction sums extensive quantities, while intensive parameters are computed using material composition. Mixed-resolution collision uses physical world coordinates with no hidden rounding that changes passage clearance. Erosion removes matter by actual volume and may generate appropriately quantified item or particulate output.

### Mesh publication pseudocode

```text
request_mesh(region, resolution):
  deps = kernel.snapshot_source_revisions(region, neighbors, palette)
  geometry = mesh_from_snapshot(deps.snapshot)
  if !deps.still_current(): discard(geometry); optionally re-request
  else upload_behind_existing_parent_proxy(geometry)
  if upload_finished && deps.still_current():
      publish_new_geometry_coverage(region, resolution)
      retire_old_geometry_after_gpu_completion()
  else retain_valid_old_parent()
```

**Edge cases:** parent may not represent microscopic hole visually, but parent is not permitted to *physically cover* a near active traversable edit. Locally refine the needed region or attach an exact render/collision edit overlay. Cross-chunk quad decisions require dependency on neighbor revisions. After ref change, invalidate temporal history and occlusion conservative coverage; no stall can turn into an empty screen region.

## 2. Proposed material definition and runtime state

```ron
// EXAMPLE conceptual schema (not a claim of current parsable RON):
(
  id: "underkingdom:copper",
  family: Metal,
  reference_density_kg_m3: 8960.0,
  heat_capacity_j_kg_k: 385.0,
  thermal_conductivity_w_mk: 401.0,
  electric_resistivity_ohm_m: 0.0000000168,
  softening_k: 900.0,
  melting_k: 1357.77,
  mechanical_class: DuctileMetal,
  tags: [Conductive, Smeltable, Oxidizes],
  base_texture: "materials/copper_clustered",
  palette: "metals/warm_copper_v1",
  response_class: Metal,
  dynamic_overlays: [Wet, Hot, Corroded, Cracked],
  reactions: ["underkingdom:copper_oxidation", "underkingdom:acid_copper"],
)
```

**Note on accuracy:** Values are illustrative physical baselines, not a promise that a game should simulate exact chemistry or all temperature ranges. Definition must explicitly state which are absolute-world quantities and which are stylized coefficients. The runtime material cell references `MaterialId` and stores only relevant local state; high-precision values belong in material tables, not every voxel.

**Reaction rule requirements:** stable id, reactant selectors by tag/material, priority/conflict ordering, temperature/pressure/contact prerequisites, bounded rate expression, stoichiometry/material ledger, energy ledger, products, persistent byproducts, human-readable explanatory diagnosis, VFX class and tests. The fallback `NoReaction` is a valid fully specified resolution. Source claims of '30% every pair reacts' are not a shipping requirement.

### One conservative reaction step

```text
for each relevant active contact interface (source_region, target_region):
  choose allowed reactions by material tags & conditions
  compute candidate flux from contact area, dt, available reactants, catalyst
  clamp to source and destination material/energy capacities
  reserve reactants and energy on both sides
  commit stoichiometric removal/product addition together
  add explicit heat/pressure/event changes
  schedule affected regions, no full-world scan
```

**Tests:** symmetric contacts don't double-react; two reactions competing for the same reagent don't consume it twice; coarse↔fine face computes same integrated mass flux and respects edit overlays; reloading with a half-processed batch cannot produce it twice.

## 3. Machine ports, graph compilation and transaction ledger

```rust
#[derive(Clone, Copy)]
enum PortDomain { Cargo, Fluid, Gas, DcElectric, AcElectric, Shaft, Heat, Signal, Aether, Bio }

struct PortKey { owner: ObjectId, local: LocalPortId }
struct TypedPort {
    key: PortKey,
    domain: PortDomain,
    frame_local_m: [f32; 3], // visual / connector transform, not save authority
    facing: Direction,
    capacity: CapacityInDomainUnits,
    mode: PortMode,
}

struct ConversionRecipe {
    input_ports: Vec<PortKey>,
    output_ports: Vec<PortKey>,
    transfer_rule: StableRecipeId,
    efficiency_or_source_model: EnergyModelId,
}
```

A machine may advertise DC energy and heat input, fluid intake, exhaust, cargo in/out, logic channel and mana port independently. The compiler finds stable connected typed ports from voxels/anchors, collapses long wires, belts and tubes into segments, then caches graphs until valid connections change. Graph indices are scratch; stable port keys persist.

**Ledger rule:** `source.available >= requested` and destination capacity sufficient, otherwise reserve partial work or reject. Consumption, production, heat and byproduct all commit in one stage. If a brownout occurs, machine workload advances only by actual received Joules, not requested watts. Network cache cannot temporarily deliver more than source stored energy during rebuild.

### Example multiblock machine contract

```ron
(
  id: "underkingdom:basic_foundry",
  appearance_family: "engineer:clay_forge",
  world_units: Meter,
  validation: (
    min_chamber_volume_m3: 1.0,
    needs_continuous_refractory_shell: true,
    needs_heat_input: true,
    needs_exhaust_path: true,
  ),
  ports: [
    (name: "ore_in", domain: Cargo, side: North),
    (name: "fuel_in", domain: Cargo, side: West),
    (name: "exhaust", domain: Gas, side: Up),
    (name: "heat", domain: Heat, side: South),
    (name: "ingot_out", domain: Cargo, side: East),
  ],
  simulation: "underkingdom:foundry_thermal_v1",
  visual_states: [Idle, Heating, Processing, Starved, Damaged],
  blueprint_source: "industrial/early_foundry_1",
)
```

Save design authority, material inventory and internal thermal energy, not simply recipe progress based on instantaneous UI. A removable shell block invalidates chamber geometry, not persistent input items. Machine repair should be user-visible and explain what changed.

## 4. Shared Work virtual machine / glyph and controller equivalence

### Typed IR examples

```text
LOAD_SENSOR   port=temp_sensor       -> r0 : Temperature
CONST         700K                   -> r1 : Temperature
COMPARE_GE    r0, r1                  -> flag
JUMP_IF_FALSE flag, label:safe
REQUEST_PORT  output=coolant_valve, value=Open, auth=owner
WAIT_TICKS    6
RETURN
```

Engineer ladder/text and Chorister rune diagrams both compile into the same typed IR. A compiler is allowed to reject unsafe/meaningless operations rather than silently reinterpret them. A spell requires the correct energy/material authorization, target queries and effect constraints. A controller's program cannot mutate arbitrary voxels by pointer.

**Sandbox fuel:** bounded instructions per active machine/trigger; bounded persistent registers, event lists, output effect bytes/entities and recursion. A pathological spell chain must fail in a coherent, visible way and cannot corrupt the world/consume infinite CPU; sometimes sim aggregation preserves the physical outcome of a huge number of identical events.

**Cross-dialect acceptance scenario:** a rune-powered controller and an electrical logic controller both read a shared sensor, actuate a physical cooling valve through an authorized port and produce **equivalent** resulting valve/machine state under matching inputs. The devices may have different energy costs and casting timings; semantics equal where the typed operator matches. Verify save/replay determinism for the authoritative action trace.

## 5. Declarative procedural `PlacePlan` and world secrets

```ron
(
  site_id: "engineer:abandoned_waterworks",
  generator_version: 55,
  biome_filter: [MountainValley, Karst],
  architectural_tradition: Engineer,
  site_mass_plan: "waterworks_gatehouse_and_tank",
  protected_spaces: ["main_approach", "primary_pump_hall", "spiral_stair"],
  route_nodes: ["surface_gate", "sluice", "service_tunnel", "main_chamber"],
  physical_anchors: ["reservoir", "pump_mount", "generator_mount", "return_pipe"],
  light_plan: "cool_stone_with_local_warm_utility",
  functional_mechanisms: ["gate_valve", "pump_control", "backup_generator"],
  secret_hooks: ["dam_water_antechamber_hint", "archive_cipher_fragment"],
  min_distinct_routes: 2,
  artifact_palette: "engineers_weathered_steel_limestone",
)
```

`PlacePlan` stages produce real usable rooms and structure. Verify route graph traversability using actual player collision capsule and clearance, not only a 2D open/closed map. Reserve major voids before furnishings/foliage. Attach lore according to economy/use/downfall: why waterworks exists, which civilization maintained it, why it failed. Generate multiple approaches and one salvageable subsystem the player can reuse.

### Secret-rule example with fairness guarantees

```ron
(
  id: "chorister:gate_of_sustained_chord",
  rule_family: ResonantGate,
  generation: (requires: [ChoristerRuin, ResonanceSource, SideAccess]),
  conditions: [
    (kind: SignalPattern, source: "local_resonators", expected: "chord_template_seeded"),
    (kind: DurationTicks, min: 180),
  ],
  clues: [
    (place_in: "ritual_archive", clue_type: Inscription),
    (place_in: "adjacent_instrument", clue_type: MeasurableResonance),
  ],
  physical_effect: "retract_ancient_gate",
  alternatives: ["power_repair_and_bypass", "hard_rock_excavation_with_cost"],
  permanence: PersistentIfMechanismRemainsActive,
)
```

**Fairness test:** seed/site validator ensures at least two different clue types exist in accessible places; required instrument/item is recoverable; if the main gate is permanently disabled, construction/knowledge alternatives remain physically possible. Human blind testers can infer the mechanic from evidence. Do not invent world state by freeform generated lore text.

## 6. Scenario harness and nested test coverage

For each feature, define expected **code-level, world-level and player-level** outcomes:

| Fixture | Action | Numerical/invariant oracle | Player-visible proof |
|---|---|---|---|
| `coarse_fine_acid_wall` | Pour acid across 1 m/0.125 m seam | Mass/enthalpy within declared bounds, no duplicated interface flux | Seam erodes consistently; nearby cave still collidable |
| `water_and_wire` | Flood a low-voltage circuit cabinet | Typed conductor/water interaction; breaker policy and charge unchanged except real debit | Fault, sound, meter diagnosis, responsive machine stop |
| `torch_library_nave` | Enter after sunset | Same physically tagged materials and source lights; no unjustified color-space gamma double conversion | Cool stone readable with warm light islands |
| `wand_trigger_multicast` | Shoot trigger with stored chained effect | Deck order/cost/event provenance and bounded fuel | Visually distinct second cast and real impact |
| `cast_vs_machine_vm` | Equivalent magic and electrical control programs | Same typed state transition, energy-accounted | Both devices operate real gate when supplied |
| `ruin_three_routes` | Pump/freeze/cut and traverse | Actual reachable paths and correctly consumed resources | All 3 approaches playable, not just quest completions |
| `rover_hull_pressure` | Breach moving vehicle under water | Reservoir/air coupling and collision stability | Flood, crew task, repair and altered mobility |
| `tree_lod_during_flight` | Rapid fly across seam near old generator site | No missing parent coverage, stable dependency revisions | No disconnected canopy or silhouette popping |
| `save_on_belt_split` | Remove busy belt and save mid-transfer | Exactly-once lot ownership before/after reload | Goods remain on correct pieces/in inventories |
| `foundation_collapse` | Remove support then drop attached volume | Detached object mass and momentum based on world scale | Coherent falling structure, not 100k independent bodies |

**Failure reports** should include `seed, generator_version, world_edits, tick, relevant revisions, asset_sha, app_exe_sha, gpu_backend, hardware profile, camera, debug-overlay settings, numeric residuals, exact commands`.

### Bench thresholds: set from observed baselines, not invented figures

- Compare H0/H1/H3 with a fixed full scene and fixed camera route. Store p50, p95, p99 frame, CPU/GPU timings, active islands/voxels, pending jobs, resident GPU arenas and texture atlas bytes.
- Profile cold and warm caches; capture first frames after teleportation, high-velocity flight, large collapse, sudden high-heat field, night/day transition and prolonged factory activity.
- **Visual truth is required:** missing world or unused spell effect does not count as a successful performance optimization.
- Keep user-facing title screen, hotbar, inventory and held item responsive under heavy world jobs.

## 7. SDD-to-code handoff acceptance checklist

- [ ] No authored code claims a non-existent repo file/API.
- [ ] One named gameplay goal and one reproducible canonical world scenario.
- [ ] Data ownership and persistent ID lifecycle defined.
- [ ] Numeric unit/rounding/model accuracy and error policy defined.
- [ ] Cross-grid and cross-chunk effects/tests covered where relevant.
- [ ] One minimal real player affordance, including accessibility where applicable.
- [ ] Actual Blender/source art to game importer path proven if visual asset.
- [ ] Old save/generator and concurrent worktree safety addressed.
- [ ] Native game screenshots separate from shader/build/tests.
- [ ] Perf delta measured if runtime available; otherwise NOT MEASURED.
- [ ] Scope excludes unrelated work and no giant speculative refactor.

**Implementation note:** The RON samples are illustrative user-facing authoring data. Production SDDs must define final schema types and validators, including safe untrusted-mod resource caps, stable naming, defaults and explicit migrations. H0 OpenGL/no-compute path needs actual capability checks rather than untested code guesses.
