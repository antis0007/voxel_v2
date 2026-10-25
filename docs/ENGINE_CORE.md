# Engine core: evolving design and implementation roadmap

Status: **long-range proposal; implemented increments are recorded separately.** This document and `docs/GAME_DESIGN.md` **supersede `docs/PLAN2.md`** as the roadmap (user decision 2026-10-23). PLAN2's decisions D1–D7 carry over; its open phases are mapped in §11. ADR 0014 reconciles the original spec. The 2026-10-24 refinement in `docs/adr/0017-roadmap-contracts-and-sparse-activity.md` corrects layout, capacity and timing ambiguities without changing the running game's authority or save format.

Current execution order and the painterly/architecture integration are in `docs/plans/2026-10-24-engine-and-world-refinement.md`. The request to continue planning and implementation authorizes bounded improvements to existing code; it does not make every proposed migration complete or approve retiring the legacy simulation.

Source: `docs/specs/ENGINE_CORE_PROMPT_v2.md`, sections III, VIII, X, XI and XII. Research notes: `docs/research/README.md`. Gameplay side: `docs/GAME_DESIGN.md`. The reconciled inventory and sequencing in `docs/MASTER_PLAN.md` also incorporate the game bible and mod addendum; notably, the shared VM and Primer begin earlier than this original schedule.

---

## 0. The code today (what the plan starts from)

| Area | Today | Where |
|---|---|---|
| Crates | 9: core, content, audio, voxel, worldgen, sim, sim_gpu, gpu, app | `crates/` |
| Live simulation | f32 per-cell water and sand, thermal (32k cells), rigid bodies (64 × 128 voxels), wall-clock budget 2.5 ms | `crates/app/src/simulation/` |
| New CA | Existing integer Margolus kernel now invoked by an opt-in, sealed headless CPU region. Ordinary window play remains legacy. Earlier CPU/GPU equivalence and desktop timing are historical evidence, not reverified for this integration | `crates/sim/src/`, `crates/app/src/simulation/cpu_region/` |
| CA authority | Small app-side `CaBackend`, exact snapshots and two-CA-tick mirror in the isolated CPU path; GPU and general gameplay cutover pending (ADR 0018) | `crates/app/src/simulation/cpu_region.rs` |
| Mutation path | `World::set` and siblings, about 70 call sites; no `Action` enum, no log, no replay | `crates/app/src/world.rs` |
| Tick agreement | Both drivers execute shared gameplay/finish functions per 120 Hz step; legacy wall-time scheduling still prevents an equivalence claim | `crates/app/src/simulation/fixed.rs`, `headless.rs`, `main.rs` |
| CPU CA activity | Dense awake-chunk index and set-bit enumeration; sleeping resident chunks are excluded from work discovery. Compile-checked, runtime equivalence and timing pending | `crates/sim/src/store/activity.rs` |
| Rendering | CPU greedy meshing, 52-byte vertices, streaming arena with multi-draw indirect, CPU frustum culling, 4+1 shadow cascades, heightfield horizon | `crates/app/src/mesh.rs`, `crates/app/src/render/` |
| Tests and gates | Headless scenarios with snapshots, content round trips, criterion benches with an 8% gate, `gpu_*` tests, layout tests | `crates/app/src/repo_layout.rs`, `scripts/bench.py` |

## 1. Standing decisions (D1–D7, carried over from PLAN2)

- **D1** 32³ chunks for storage and streaming; 8³ bricks on the GPU; a `u64` dirty mask per chunk, one bit per brick.
- **D2** `Scale` reserved in the brick header and brick coordinate type; unused until variable-scale LOD lands.
- **D3** All CA state is integer; every random choice is a positional hash; hash and Margolus phase order pinned in one `const` block with a golden test.
- **D4** `CaBackend` trait; gameplay reads state two ticks behind the authoritative backend (ADR 0009; implemented only in the isolated CPU development path, ADR 0018).
- **D5** No ECS (ADR 0010).
- **D6** `naga_oil` is tried first; otherwise the ≈80-line resolver.
- **D7** Save v29 with a v28 loader. Loaders older than v28 are removed when no save or scenario needs them.

New: ADR 0015 (hybrid cell), ADR 0016 (`lowend` = Iris Xe laptop).

## 2. Invariants (target)

The spec's invariants (III.1) are the target. Each one is added to `CLAUDE.md` in the session that makes it true, not before (ADR 0014).

| Invariant | Enforced by | Session (§14) |
|---|---|---|
| Integer-only CA state (no `f32`/`f64` in sim state and CA modules) | Grep test over `engine_sim` and the CA modules | M1 S2 |
| One tick-order `const` array; headless and window agree | Array test plus a same-hash test (window driver in headless mode) | M1 S2 |
| No `Instant` in simulation; no `rand` in game code | Grep test | M1 S2 |
| One mutation path: `Action` enum is the only writer | World write functions private to the action applier | M1 S3 (CA writes), M2 (all) |
| Budgets per system from a per-profile RON table | Budget struct, overlay report | M1 S7 |
| Sleep by default: a brick is awake iff `dirty != 0`; a static world costs zero dispatches | Bench assertion: awake count 0 in a static scenario | M1 S7 |
| Data over code: no material or item id matches outside the content crate | Grep test | M1 S6 |
| CPU reference ≡ GPU kernel, byte-exact | `just test-gpu` on 200 random bricks per kernel | each CA session |
| File ≤ 300 lines; function ≤ 60; nesting ≤ 3; public items ≤ 40 per crate | Lint, new code only (ADR 0014 row 8) | M1 S3 |
| No unwrap outside tests and startup | `clippy::unwrap_used` scoped to non-test code | M1 S3 |

## 3. Cell and brick layout (ADR 0015)

### 3.1 Base cell (built, unchanged)

```
Cell: material u8 | volume u8 (0..=64) | temperature u16 (0.1 K) | pressure i16 | charge i16   (8 bytes)
Brick: 8³ = 512 cells = 4 KiB; 64 bricks per 32³ chunk
```

### 3.2 Optional planes (new, per brick, on demand)

```
motion: vel_x, vel_y, vel_z: i8 (1/16 cell per tick) | aux: u8      4 bytes per cell = 2 KiB per brick
u8 fields: oxygen, mana, aspect, moisture, scent, light_sky, light_block   512 B each
u16 item plane (items on belts and in cells)                              1 KiB
```

A plane is allocated when a material that needs it enters the brick (powder needs `motion`; fire needs `oxygen`) and freed when the brick sleeps with the plane at its default value. An absent plane reads as its default. The spec's packed two-plane layout is not adopted. Material widens to u16 only past 255 materials, by its own ADR and a save bump.

### 3.3 Brick header (target, 32 bytes, GPU array)

```
word 0..1: dirty_lo: u32 | dirty_hi: u32 (one bit per 2³ Margolus block)
word 2: flags: u32 (planes and derived-work flags)
word 3: support_min:8 | awake_ticks:16 | lod:8 (packed explicitly into u32)
word 4..7: mesh_slot: u32 | plane_base: u32 | light_base: u32 | reserved: u32
```

This is a proposed 32-byte wire layout, not today's CPU `ChunkEntry` or GPU layout. Current dirty masks have one bit per brick in a chunk; the proposed sub-brick mask is a separate future layer. Non-empty uniform bricks are not compressed today.

At 20k bricks, base cells cost 78.125 MiB, motion adds 39.063 MiB, seven u8 fields add 68.359 MiB, and the item plane adds 19.531 MiB: **205.078 MiB before mirrors, headers, staging, meshes or allocator overhead**. The earlier 120 MB estimate covered roughly base plus motion only. Admission must use actual bytes per plane, not just brick count (ADR 0017).

### 3.4 Pools and lists

- **Brick pool:** one GPU buffer, free list, background defragmentation at most 1 ms per tick. A plane pool per plane type.
- **Awake list:** GPU compaction each tick (prefix sum) feeds indirect dispatch. No CPU readback.
- **Chunk grid:** hash map from chunk coordinate to 64 brick ids, resident ring of radius `view_distance` with hysteresis. Paging to disk in LZ4 is part of the save v30 work.
- **Entity tables (CPU):** generational columns for bodies, mobs, machines, projectiles, items, players, vehicles, NPCs. Counting sort by brick id per tick for locality, within budget. Chunk-local spatial hash for broadphase.

## 4. Margolus CA kernel

- **Block:** 2×2×2 Margolus block. One 64-thread workgroup per awake brick per phase. Each thread owns one block in registers; a 10³ shared tile supplies the halo.
- **Phases:** today 2 offsets per tick, pinned. The spec asks for 8 offsets permuted per tick by a positional hash, which removes directional artefacts. M1 S4 measures both on the avalanche scenario (artefact metric plus cost) and records the choice in an ADR; the pinned golden changes once.
- **Rule order per block (const, tested):** `reactions → phase_change → transport_cells → powder → liquid → gas → velocity_integrate → growth → sleep_check`.
- **Dirty propagation:** a changed cell sets dirty bits on touched neighbour blocks, including across brick faces.
- **Sub-step:** fast powder cells (|velocity| above one cell per tick) get extra sub-step dispatches from a compacted list, within budget.

```
for block in brick.blocks_in_phase(phase_order(tick)[phase]):           # 64 threads, one block each
    cells = load_2x2x2(block, halo_tile)                                 # registers, base cell + planes
    for rule in RULE_ORDER:                                              # fixed const order
        cells = rule.apply(cells, hash(seed, tick, block_origin, rule))  # positional hash only
    store_2x2x2(block, cells)
    mark_dirty_neighbours_if_changed(block)
brick.sleep_check()                                                      # clears dirty if no change
```

## 5. Physics rules (M1)

Each rule is a data-driven row where possible. The numbers below are targets set in RON, not constants in code.

- **Powder:** per-cell velocity in `motion`, gravity from material, sub-cell accumulator in `aux`. Blocked cells spread two-thirds of the velocity laterally, biased by existing motion. Angle of repose per material. Shatter above `shatter_speed` in the reaction table.
- **Liquids:** volume-based (0..=64). Hydrostatic pressure per column computed in phase 0 into the cell's `pressure`. Face flow by pressure gradient, capped by `flow_rate`. Lateral spread at most 3 cells per tick. Interior cells sleep. Compression above full under ceilings, for U-tubes. Viscosity, surface tension threshold, evaporation, freezing, boiling from temperature.
- **Gases:** density 1–15, buoyancy from density and temperature, diffusion, dissipation threshold. Slower tick rate away from the player (`gas_divisor`). Oxygen refill from sky. Methane and hydrogen explode through pressure pulses.
- **Thermal:** explicit diffusion with per-material conductivity (`SimTraits` in the content crate). Phase changes with latent heat as an energy delta, conserved by the ledger (ADR 0007 rule 6).
- **Reactions:** table compiled to a GPU hash table keyed by `(a, b)` and `(a, self)`. Row fields: condition bitmask (oxygen, water, light, mana, pressure), temperature min and max, products, energy delta, probability as `u16`, event. Today: 5 rows in `assets/reactions.ron`, evaluated by `crates/sim/src/react.rs`.
- **Fire:** fuel in `aux`, ignition by temperature, consumes oxygen, emits heat and smoke, leaves ash or char. Extinguished by water, sand or low oxygen.
- **Support and collapse:** support relaxes from bedrock (255). Per-material `max_span`. Below `collapse_threshold`, small clusters become powder; larger clusters become rigid bodies (M2).
- **Growth:** plants need light, moisture and a temperature band. Stages are materials. Tree growth uses incremental templates.
- **Light:** sky and block light by BFS on `light_dirty` bricks, budgeted.
- **Weather:** per-chunk exposure. Rain adds water to exposed surfaces; snow accumulates powder; lightning is a heat and charge impulse.

## 6. Rigid bodies (M2)

Today: CPU bodies, at most 64 bodies of 128 voxels each, floats, voxelised back each substep (ADR 0011). M2 keeps the CPU path and moves it onto bricks and the action path.

Target (III.4): integer fixed-point bodies with a brick-aligned grid of at most 64³, GPU flood fill for creation, GPU surface-cell contacts read back at most 4 KB per tick, sequential impulses, fracture on the weakest axis-aligned plane, sleep and re-fuse snapping to the 24 cube rotations within 10°. Joints (hinge, slider, fixed) and rope chains. The integer and GPU parts are decided by ADR at M2 exit (ADR 0014 row 3).

## 7. Mobs (M2)

`MobTable` with position, velocity, capsule, kind, health, needs, goal, path handle, brick id, LOD tier, inventory handle, faction, status flags. Today's enemies and five bosses (`crates/app/src/world/adventure/`) migrate onto it with their behaviour intact.

- **Movement:** capsule sweep against the same tick-stamped two-CA-tick gameplay mirror near and far (D4). Prediction may improve local responsiveness without committing independent authoritative writes. Modes: walk, climb, swim, fly, burrow.
- **Navigation:** per-brick `NavSummary` (walkable bitset plus six portal bytes), coarse graph, HPA* with cached paths and jittered replans.
- **Senses:** scent (gas plane), hearing (event ring), sight (at most four rays per mob per tick), vibration (support and body events).
- **Behaviour:** utility AI defined in RON. Actions emit `Action`s, never direct writes.
- **LOD:** tiers 0, 1 and 2 by distance.

## 8. Machines, networks and power (M4)

Today: 27 machine kinds in `assets/machines.ron`, integer power at 250 J units per 0.25 s tick, packet item transit, multiblocks.

- **Transport as cells:** belts, splitters, mergers, inserters as one-cell entities, pipes as confined fluid, pneumatic tubes as pressurised gas carrying item cells.
- **Machines:** `MachineTable`, eight-slot inventories, energy, heat and mana buffers, recipe, progress, divisor buckets. Multiblocks from RON shape matrices.
- **Networks:** electric (supply and demand solve, voltage tiers), kinetic (speed and stress), heat and mana (fields), signal (fixed evaluation order by cell id).
- **Digital storage:** item index and crafting planner in CPU tables, budgeted.

Each network type is solved once per tick in deterministic order.

## 9. Rendering and audio

- GPU binary greedy meshing per brick into pooled 8-byte quads. Indirect draws per material class. GPU frustum and Hi-Z culling. Cascaded shadows with per-cascade culling.
- Volumes (liquid, gas, fire, mana) by quarter-resolution raymarch over a 3D texture built from planes.
- Dirty to mesh: at most one frame locally, two frames far away.
- Audio: event-driven from the `audio_events` stage; library choice (`kira` or `rodio`) needs an ADR.

Visual changes need before and after captures (`docs/design/VISUAL_DIRECTION_EXECUTION.md`). Meshing and LOD changes need a fixed-seed camera route across chunk and LOD boundaries.

## 10. Save format and migration

- **Current source, checked 2026-10-24:** `VXWSAV32`, `SAVE_VERSION = 32`, generator 46. The loader accepts version headers 1 through 32 with version-specific decoding. This is a source observation, not a newly verified migration result. D7's v29 and the spec's v30 are historical targets; preserve the existing loaders.
- **Future migration (version unassigned):** paged chunks with LZ4, entity tables, plane pools, action-log tail, incremental autosave. Assign a version from the live source when implementation starts; versions 30–32 are already used. Requires its own ADR and old-fixture compatibility evidence (`just test migrate`, `just test save`, deferred under the current no-tests instruction).
- Generator version gates are never edited; a behaviour change adds a new gate.

## 11. Milestones (supersede PLAN2's phases)

| Milestone | Content (spec XI) | Absorbs from PLAN2 | Sessions (§14) |
|---|---|---|---|
| M1 Foundation | CaBackend wiring, actions and replay, planes, powder, liquid, gas, thermal, reactions, fire, support, light, GPU awake lists, brick meshing, volumes | A6 (wiring), A7 (ledger, boundary pass), A8 (ballistic particles), B (render) | S1–S8 |
| M2 Bodies and mobs | Rigid bodies on bricks, joints, fracture, mobs, nav, utility AI, animals, villagers | (new) | S9–S14 |
| M3 Survival core | Needs, durability, crafting, smelting, farming, maps, beds, trading, advancements, weather and seasons, text layer v1, profiles | E (text layer, profiles) | S15–S20 |
| M4 Engineer path | Kinetic, steam, electric, belts, pipes, inserters, multiblocks, networks, logic, storage, trains, drones | C (factory scale) | S21–S28 |
| M5 Mage path | Mana, wands on primitives, runes, rituals, alchemy, enchanting, constructs, aspects, codex | D (unified rules) | S29–S36 |
| M6 World and depth | Depth tiers, dimensions, structures, bosses, dungeons, worldgen | (worldgen crate work) | S37–S42 |
| M7 Hybrid, education, polish | Techno-magic, lessons, audio, accessibility, modding, performance, Steam Deck | F (polish, roadmap) | S43–S50 |

Milestones are cut lines, not promises. Each ends with a playable build, a captured demo, a bench table, and LOC and test counts.

## 12. Benchmarks and acceptance

| Target | Value | Source | Status |
|---|---|---|---|
| CA correctness exit | 250k active cells at 60 tps on the CPU reference; GPU byte-identical | PLAN2 §1a | Pending (M1 S6) |
| CA performance exit | 500k awake cells at 60 tps on `lowend`, GPU backend | Spec III.3, ADR 0016 | Pending (M1 S7) |
| Frame rate | 60 fps p95 on `lowend` at 12 chunks; 144 fps desktop at 24 | Spec III.8 | Pending (M1 S8) |
| Static world cost | Zero CA dispatches, zero remesh, zero body updates | Spec III.1.5 | Pending (M1 S7) |
| Perf gate | Median of 3 runs, fail above 8% | ADR 0006 | In place |
| M1 scenarios | `avalanche_100k`, `dam_break_200k`, `u_tube`, `fountain`, `gas_room_seal`, `methane_explosion`, `lava_water_obsidian`, `oil_fire_forest`, `ceiling_collapse`, `rain_flood_valley`, `crop_growth_season`, `rust_in_rain` | Spec III.3 | Pending; today `assets/scenarios/` has `sand_falls`, `smoke`, `water_falls` |
| M2 scenarios | `boulder_rain_200`, `collapsing_tower`, `log_raft`, `glass_shatter`, `mob_swarm_300`, `flood_evacuation`, `predator_prey_100` | Spec III.4–III.5 | Pending |
| Conservation | Mass per material changes only by reaction deltas; thermal energy by the ledger | Spec X, ADR 0007 rule 6 | Pending (M1 S3) |
| Replay | Every scenario replays without desync | Spec X | Pending (M1 S3) |

## 13. Risks

| Risk | Mitigation |
|---|---|
| iGPU throughput: 500k awake cells may not fit an Iris Xe at 60 tps | Measure on the target device before acceptance; desktop timings and scaled estimates remain separate. Bound authoritative work deterministically |
| GPU memory on iGPUs | Plane pools sized per profile; eviction by distance; residency in the overlay |
| Readback latency | Tick-tagged result ring with exactly two CA ticks of gameplay lag (D4); deadline misses stall authority, never publish an older snapshot as current |
| Margolus artefacts | Phase permutation measured in M1 S4, pinned by test |
| Cross-vendor determinism | Integer-only state, hash golden test, CPU≡GPU on 200 random bricks per kernel |
| Save migrations | Every bump gets an old-fixture test (saves rule) |
| Wiring the new CA breaks gameplay or water presentation | Stage command ingress, CPU adapter, mirror, GPU adapter, then cutover; keep legacy rollback until equivalence, saves, visual and performance gates pass |
| Content explosion | Validators for references, reachability and energy cycles; the feature matrix cost column |
| Feature creep and scope | Pillar test; milestones are cut lines |
| Multiplayer | Lockstep-ready design (actions, replay); transport deferred by ADR |
| Education accuracy | Constants cited per lesson; lesson review before release |
| Magic balance | Mana budgets, backfire and corruption mechanics |

## 14. Session schedule

Sessions below are work packages, not estimates or a requirement to combine several migrations into one session. The ordered, smaller increments in `docs/plans/2026-10-24-engine-and-world-refinement.md` refine S2/S3. Record compile, scenario, performance and visual evidence separately. The current user instruction prohibits adding/running tests or hosted CI; listed test gates remain pending, rather than being run implicitly through `just verify`.

**M1 Foundation**
1. Original planning deliverable: research notes, this document, `docs/GAME_DESIGN.md`, ADRs 0014–0016. Produced 2026-10-23; subsequent refinement is tracked in the dated execution plan.
2. Stage the game-to-CA integration (old PLAN2 A6): establish clock/command semantics before `CaBackend` (Cpu, Gpu), then the 2-tick mirror and shared driver. Remove residency wall-time decisions from authoritative logic. Cut over only after the separate correctness, persistence, performance and visual gates; there is no one-session legacy-retirement deadline.
3. `Action` enum v0 (CA writes), per-tick action log, replay and desync check for all scenarios; conservation ledger and boundary pass (old A7); file and function size lints and the unwrap lint for new code.
4. `motion` plane; powder momentum, angle of repose, shatter; ballistic particles (old A8); Margolus phase-permutation measurement and its ADR. Scenarios `avalanche_100k`, `fountain`.
5. Liquids with pressure and compression; gases with buoyancy; `oxygen` plane. Scenarios `u_tube`, `dam_break_200k`, `gas_room_seal`, `methane_explosion`.
6. Thermal phase changes with latent heat; reaction table growth (fire, rust, obsidian, oil); fire on oxygen; content-id lint. Scenarios `lava_water_obsidian`, `oil_fire_forest`, `rust_in_rain`. CA correctness exit (250k).
7. Support and collapse to powder; light planes; rain exposure; GPU awake-list compaction and indirect dispatch; per-profile budgets table. Scenarios `ceiling_collapse`, `rain_flood_valley`, `crop_growth_season`. CA performance exit (500k on `lowend`).
8. GPU binary meshing per brick, indirect draws per material class, liquid and gas volume raymarch; write the PERF and WORLDGEN docs. M1 demo: avalanche, dam break, forest fire.

**M2 Bodies and mobs**
9. Collapse clusters become CPU rigid bodies over bricks; fracture and re-fuse; ADR on integer bodies and GPU contacts.
10. Joints and rope chains; carts, boats and rafts as bodies. Scenarios `log_raft`, `windmill_hinge`, `cart_on_rail`.
11. `MobTable`; existing enemies and bosses migrate onto it; movement modes; senses from planes.
12. `NavSummary` and HPA*; replanning on edits. Scenario `flood_evacuation`.
13. Utility AI in RON; animals with herds and breeding. Scenarios `mob_swarm_300`, `predator_prey_100`.
14. Villager basics; a boss on the mob table. M2 demo: a collapsing cave with fleeing creatures.

M3–M7 sessions are scheduled at the end of M2 against measured velocity.

## 15. Open decisions for approval

1. Accept ADR 0014 (with the user decisions in rows 6, 11 and 14), ADR 0015 and ADR 0016.
2. Confirm milestone scope after the first integrated vertical slice; use the dated execution plan for the next bounded increment.
3. Legacy retirement remains contingent on the cutover gates, rather than on a calendar/session number.
4. Confirm the 250k correctness exit and the 500k `lowend` performance exit.
