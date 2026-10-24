# Engine core: design proposal (session 1 deliverable)

Status: **proposal, not approved.** Nothing here is implemented. This document and `docs/GAME_DESIGN.md` **supersede `docs/PLAN2.md`** as the roadmap (user decision 2026-10-23). PLAN2's decisions D1–D7 carry over unchanged; its open phases are mapped in §11. Conflicts between the spec, PLAN2 and the code are resolved in `docs/adr/0014-spec-v2-reconciliation.md`, and that ADR governs. Where this document and the code differ, the code is described as "today" and the spec as "target".

Source: `ENGINE_CORE_PROMPT_v2.md`, sections III, VIII, X, XI and XII. Research notes: `docs/research/README.md`. Gameplay side: `docs/GAME_DESIGN.md`.

---

## 0. The code today (what the plan starts from)

| Area | Today | Where |
|---|---|---|
| Crates | 9: core, content, audio, voxel, worldgen, sim, sim_gpu, gpu, app | `crates/` |
| Live simulation | f32 per-cell water and sand, thermal (32k cells), rigid bodies (64 × 128 voxels), wall-clock budget 2.5 ms | `crates/app/src/simulation/` |
| New CA | Integer Margolus kernel on 8³ bricks, reactions from RON, CPU and GPU byte-identical (0.98 ms per 1M cells on an RTX 4090). **Not wired into the game.** | `crates/sim/src/`, `crates/sim_gpu/src/` |
| CA authority | No `CaBackend` trait and no 2-tick mirror (ADR 0009 corrected) | `crates/sim/src/kernel.rs` |
| Mutation path | `World::set` and siblings, about 70 call sites; no `Action` enum, no log, no replay | `crates/app/src/world.rs` |
| Tick agreement | Headless steps per tick; the window runs each system once per frame with `steps × STEP` as dt | `crates/app/src/headless.rs`, `crates/app/src/main.rs` |
| Rendering | CPU greedy meshing, 52-byte vertices, streaming arena with multi-draw indirect, CPU frustum culling, 4+1 shadow cascades, heightfield horizon | `crates/app/src/mesh.rs`, `crates/app/src/render/` |
| Tests and gates | Headless scenarios with snapshots, content round trips, criterion benches with an 8% gate, `gpu_*` tests, layout tests | `crates/app/src/repo_layout.rs`, `scripts/bench.py` |

## 1. Standing decisions (D1–D7, carried over from PLAN2)

- **D1** 32³ chunks for storage and streaming; 8³ bricks on the GPU; a `u64` dirty mask per chunk, one bit per brick.
- **D2** `Scale` reserved in the brick header and brick coordinate type; unused until variable-scale LOD lands.
- **D3** All CA state is integer; every random choice is a positional hash; hash and Margolus phase order pinned in one `const` block with a golden test.
- **D4** `CaBackend` trait; gameplay reads state two ticks behind the authoritative backend (ADR 0009, not yet built).
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
dirty: u64 (one bit per 2³ Margolus block)
flags: u32 (plane bitset:10, uniform, has_bodies, nav_dirty, mesh_dirty, light_dirty, network_dirty, weather_exposed)
support_min: u8 | awake_ticks: u16 | lod: u8 | mesh_slot: u32 | plane_base: u32 | light_base: u32
```

A uniform brick (one material, no planes) is a constant in the chunk grid, allocated on first edit. Expected residency on `lowend`: at most 20k allocated bricks, about 120 MB with planes (ADR 0016).

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

- **Movement:** capsule sweep against the CPU mirror near the player; one-tick-stale snapshot far away. Modes: walk, climb, swim, fly, burrow.
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

- **Current:** `VXWSAV29`, `SAVE_VERSION = 29`, loader for v28 (D7). The generator version is in `CLAUDE.md`.
- **Next (save v30):** paged chunks with LZ4, entity tables, plane pools, action-log tail, incremental autosave. Requires its own ADR and an old-fixture round-trip test (`just test migrate`, `just test save`).
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
| iGPU throughput: 500k awake cells may not fit an Iris Xe at 60 tps | Calibrate early (M1 S2 records the 4090-to-lowend ratio); gas divisor and distance degradation; awake caps per profile |
| GPU memory on iGPUs | Plane pools sized per profile; eviction by distance; residency in the overlay |
| Readback latency | Double-buffered small results with one tick of lag (D4) |
| Margolus artefacts | Phase permutation measured in M1 S4, pinned by test |
| Cross-vendor determinism | Integer-only state, hash golden test, CPU≡GPU on 200 random bricks per kernel |
| Save migrations | Every bump gets an old-fixture test (saves rule) |
| Wiring the new CA breaks the look of water and sand | Same-pose captures before and after M1 S2; legacy path kept behind a flag for one session |
| Content explosion | Validators for references, reachability and energy cycles; the feature matrix cost column |
| Feature creep and scope | Pillar test; milestones are cut lines |
| Multiplayer | Lockstep-ready design (actions, replay); transport deferred by ADR |
| Education accuracy | Constants cited per lesson; lesson review before release |
| Magic balance | Mana budgets, backfire and corruption mechanics |

## 14. Session schedule

Sessions are a planning unit, not a commitment. Each ends with `just verify` green, a scenario, a bench number on `lowend`, a capture when the picture changes, and the LOC delta.

**M1 Foundation**
1. This session: research notes, this document, `docs/GAME_DESIGN.md`, ADRs 0014–0016. Stop for approval.
2. Wire the game to `engine_sim` (old PLAN2 A6): `CaBackend` (Cpu, Gpu), 2-tick mirror, edits as commands; shared tick-order array with a headless-equals-window hash test; residency timer off `Instant`; integer, `Instant` and `rand` grep tests. Retire the legacy water, sand and thermal paths after same-pose captures. Record the lowend calibration ratio.
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
2. Accept the milestone table in §11 and the M1–M2 sessions in §14, with M1 S2 as the next session.
3. Confirm that the legacy simulation is retired in M1 S2, after captures, rather than kept in parallel.
4. Confirm the 250k correctness exit and the 500k `lowend` performance exit.
