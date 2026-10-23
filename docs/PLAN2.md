# Phase 2 plan: foundations (approved 2026-10-22 with the changes in §1a)

Scope: the six-phase mission (A simulation, B rendering, C automation, D rules, E text, F cheap changes) mapped onto the code as it is after the revamp (`docs/PLAN.md`). This file names crates, files and recipes that do not exist yet; it is exempt from the docs-path test like `AUDIT.md`.

Baseline (2026-10-22): ≈115k Rust lines, 9.3k WGSL, 441 tests, 3 scenarios, 0 benches, 6 crates, `just verify` ≈25 s warm.

## 0. What the code is today (facts the plan is built on)

| Area | Today | Consequence for the plan |
|---|---|---|
| Cell storage | `ChunkData` dense 32³ `BlockId` (`repr(u8)`, 83 of 256 used) + light; water volume 1–7 in a sparse per-chunk map, missing = full | Material id fits a byte; water volume must move into the cell record to be GPU-resident |
| CA | CPU, in place, sequential, 1 thread: `cellular.rs` 461 + `thermal.rs` 309 + `activity.rs` 416 + `water.rs` 206. ≤8192 cells/tick, ≤128 chunks/tick, interval `0.08·m` s | 1M active cells is ~120× today's cap; a rewrite, not a port |
| Wake | 1 bit per cell, ≤4096 active chunks, overflow to 8³ region bitsets | 8³ is already the natural dirty unit |
| Thermal | f32 temperature + fuel in a sparse map capped at 32,768 cells | Not GPU-ready, not deterministic across GPUs as f32 |
| Reactions | Hand-coded: 6 in `thermal.rs`, 7 in `spells.rs`; ≈40 `BlockId` match sites in the sim | Every new material costs code in several files |
| `FluidTraits` | In `materials.ron`, never read by the sim | Property-driven CA is half-declared already |
| Rigid | CPU, ≤64 bodies × ≤128 voxels, voxelised back into the grid each substep | GPU collider extraction has no load to justify it yet |
| GPU compute | Live only in atmosphere; `fluid_compute.wgsl` is dead (non-conserving) | Start fresh; delete the dead shader |
| Render | One `draw_indexed` per visible chunk (+5 shadow cascades each), per-chunk buffers in a `HashMap`, CPU frustum cull, 56-byte vertices (248 B/quad), no storage buffers in runtime shaders, `Limits::default()` | Large, measurable headroom for GPU-driven draws |
| Shaders | 31 files, string concatenation in `shader_sources.rs`, naga-validated in `build.rs`; 15 ad hoc bind-group layouts | One include system and one binding table are mechanical wins |
| GPU tests | Contract tests hard-code `Backends::VULKAN`; no full-scene offscreen render; capture copies the swapchain | Golden images need a window-free `Gpu` |
| Headless | `--headless` runs the full tick order with no GPU; scenarios hash world state | Must stay GPU-free, so CPU reference kernels are a requirement |
| Scale | One `VoxelScalePreset` per world; no mixed-scale storage | "Variable scale in one world" is new capability, not a refactor |

## 1. Decisions needed before any code (each has a >1-week consequence)

I recommend the first option in each; nothing in Phase A starts until these are answered.

**D1. Brick size.** The mission specifies 64³ bricks.
- (a) **Recommended:** keep the 32³ chunk as the stream/save/authority unit (region files, `ChunkData`, saves unchanged in shape); add an **8³ brick** as the GPU allocation, dirty-flag and dispatch unit (64 bricks per chunk, 512 cells, one workgroup-sized tile). The pointer table maps brick coords → slot in one GPU pool.
- (b) 64³ bricks with 8³ dirty tiles inside: no save alignment with 32³ chunks/8³-chunk regions; needs a region-format migration.
- (c) 64³ as written: a single falling grain keeps 262,144 cells awake, which defeats "idle regions cost zero".

**D2. What "variable scale" means.**
- (a) **Recommended first:** simulation/render LOD of a single-scale world. Far regions are held at coarser octree depths; matter crossing a depth boundary is converted with conservation.
- (b) Mixed authored scales in one world, for example a 0.25 m workshop next to 1 m terrain. (b) builds on (a) and changes worldgen, saves and player physics; schedule it after Phase B if wanted.

Boundary rules, to be written once in ADR 0007 and enforced by proptests:
1. A depth-d cell covers 2^d finest cells per axis.
2. Neighbouring bricks differ by at most one depth (2:1 balance).
3. Mass is counted in finest-cell units, so a depth-1 cell holds up to 8 × 8 units. Volume fields widen with depth; u8 covers depths 0–1 and u16 covers 2–4.
4. A depth-d brick ticks every 2^d fine ticks. This matches today's `0.08·m` interval rule.
5. Transfers across a boundary happen only in a dedicated boundary pass. It aggregates fine faces into the coarse neighbour and splits coarse outflow by a positional hash with a remainder carry.

**D3. Determinism policy.**
- (a) **Recommended:** all CA state is integer:
  - material u8;
  - volume u8/u16;
  - temperature u16 in 0.1 K;
  - pressure i16;
  - charge i16.

  Every random choice is `hash(seed, tick, cell)`, and reductions go through no floating-point atomics. The GPU and CPU kernels are then byte-identical on any adapter, and equivalence tests compare bytes, not tolerances. Floats remain only in rendering and rigid bodies, which stay CPU-authoritative, so cross-GPU determinism is guaranteed for the CA and documented as not applying to rendering.
- (b) f32 fields with tolerance tests: simpler shaders, but CPU/GPU drift, cross-vendor divergence and no hash snapshots.

**D4. Authority and readback.**
- (a) **Recommended:** a `CaBackend` trait with `Cpu` (reference, used headless and in tests) and `Gpu` implementations producing identical bytes.
  - Gameplay (player collision, factory, adventure) reads a CPU mirror fixed at exactly **L = 2 ticks** behind the CA.
  - The GPU backend blocks if a readback misses its tick, and the CPU backend emulates the same lag, so headless and windowed runs stay identical.
  - Gameplay writes are edit commands applied at the start of a tick, in submission order.
- (b) CPU authoritative with the GPU accelerating only: no lag, but every dirty brick is uploaded each tick, which caps scale at the CPU.

**D5. ECS or not.**
- (a) **Recommended:** no ECS crate. Entity kinds become SoA tables with stable `u32` handles inside the existing `World`. 88 adventure functions take `&mut World`, so an ECS migration would be a rewrite of the whole game layer.
- (b) Adopt `bevy_ecs` or `hecs`: one mechanism for entities, but a several-week migration and a large dependency.

**D6. Dependencies.** Each gets an ADR with the lines it removes.

| Dependency | Use | What it removes or costs |
|---|---|---|
| `naga_oil` | Phase B shader includes | Removes the concatenation glue (≈300 lines in `shader_sources.rs`). Risk: its naga version must match wgpu 27's naga. Fallback: a ≈80-line `#import` resolver in `shader_sources.rs`, no dependency. |
| `proptest` (dev) | Brick, coordinate and scale-transition tests | — |
| `criterion` (dev) | Benches | Already approved in P2 |
| `cargo-mutants` | Tool, installed on the machine | — |

**D7. Water and save format.** Partial water moves into the brick cell record as a dense u8 volume. This bumps the save to v29 with a v≤28 loader (the sparse map converts on load). Old generator versions are untouched, since storage is not generation. Scenario hashes change once and are re-recorded with the reason.

## 1a. Decisions taken (2026-10-22, user-approved)

D1–D7 were accepted with the recommended option, plus these changes. They override the phase text below where the two differ.

- **D1:** 32³ chunks for storage, 8³ bricks on the GPU. Each chunk also carries a `u64` dirty mask, one bit per brick, so idle bricks skip dispatch entirely. The overlay shows the active-brick count. ADR 0007.
- **D2:** Level of detail first. `Scale` is reserved now, unused, in the brick header and the brick coordinate type, so mixed scales never force a save migration. ADR 0007.
- **D3:** All CA state is integer and every random choice is a positional hash. The hash function and the Margolus phase order are pinned in one `const` block. A test fails if the hash of a canonical CA scenario changes. ADR 0008.
- **D4:** A `CaBackend` trait, with gameplay reading state 2 ticks behind. The lag goes into the CLAUDE.md invariants when A6 makes it true; it is not written there before the code exists. ADR 0009.
- **D5:** No ECS. ADR 0010.
- **D6:** Try `naga_oil`. If its naga version does not match wgpu 27's, use the ≈80-line resolver and do not fight it.
- **D7:** Save v29 with a v28 loader. Loaders older than v28 are deleted when no save file in `assets/` or the scenarios needs them.
- **Rigid bodies:** GPU collider extraction is deferred. ADR 0011.
- **Phase A exit criteria:** Phase A ships when 250k active cells reach 60 ticks/s on the CPU reference and the GPU kernel matches it byte for byte. The 1M-cell bench stays, but closing the gap to 1M is Phase B optimisation.
- **Before Phase 0:**
  - free disk (`target/` and spike build dirs);
  - cap sccache at 10 GB;
  - add `just clean`.
- **Perf gate:** the median of 3 runs, failing above 8%, until a quiet baseline machine exists. This replaces the two-strike 5% rule in Phase 0 item 7.

ADR numbers:

| ADR | Subject |
|---|---|
| 0006 | Benches, perf gate and session report |
| 0007 | Bricks and scale |
| 0008 | CA determinism |
| 0009 | CA authority and lag |
| 0010 | No ECS |
| 0011 | Rigid bodies stay on the CPU |
| 0012 | Shader includes |
| 0013 | Terrain renderer |

## 2. Phases

Each phase lists its sessions, what it adds or removes, its acceptance commands and why it makes later changes cheaper. Line estimates are ranges; actual figures are appended when a phase lands.

### Phase 0: harness (prerequisite) · +1.5k to +2.5k lines · risk: low

Every later phase needs these. Today they are the first entries in the debt list.

1. **Library target:** split `crates/app` into a lib plus a thin `main.rs`, so `criterion` benches and integration tests can link it. Moves only.
2. **New crates:**
   - `engine_gpu`: device creation for any backend, a window-free `Gpu` constructor, readback helpers, one bind-group table.
   - `engine_sim`: an empty shell that Phase A fills.
3. **`just test-gpu`:** runs `#[cfg(feature = "gpu-tests")]` tests on any adapter and fails, rather than skipping, when none is found. The existing Vulkan-only contract tests move to it.
4. **`just bench <name>`:** criterion. Baselines are committed under `benches/baselines/`. Machine noise is handled by interleaving A/B runs and taking the best of each side (lesson already recorded for this repo).
5. **Benches:** the first is the ignored terrain-generation timing test. It becomes a bench and the test is deleted.
6. **`just session-report`:**
   - LOC delta vs `main`;
   - test count delta;
   - bench deltas vs baselines;
   - scenario count;
   - wall time since the branch point.
7. **Perf gate:** `just verify` gains `bench-check`. It fails when a committed bench is more than 5% worse than its baseline in two consecutive interleaved runs; a single run never fails the gate. This is pulled forward from Phase F because every later phase must report bench numbers.

Accept: `just verify`; `just test-gpu`; `just bench`; `just session-report` prints all five numbers.

**Phase 0 actuals (2026-10-22).**

Done:
- **Bench:** `crates/worldgen/benches/terrain.rs` (`terrain_generate_8_chunks`, baseline 90.3 ms), with the gate `scripts/bench.py` and the recipes `just bench`, `bench-check`, `bench-save`.
- **Gate result:** median of 3 runs, +8%. Warm runs agree within ≈1.5%, and a 15% injected regression fails with exit 1.
- **Session report:** `just session-report` (`scripts/session_report.py`).
- **Clean:** `just clean` deletes the dev, release and release-fast outputs only. ≈7 GB of review evidence also sits under `target/`.
- **GPU tests:**
  - `engine_gpu` with `test_adapter()`.
  - The 24 GPU tests are renamed `gpu_*` and the 15 hand-built Vulkan setups are removed.
  - `just test-gpu` passes 24 of 24 in 7.6 s, and fails loudly with no adapter (checked by forcing `WGPU_BACKEND=metal`).
  - A layout test keeps `request_adapter` inside `engine_gpu` and the renderer.
- **Docs test:** now reads hyphenated recipe names.
- **ADRs:** 0006–0011.

Numbers:
- Rust: +142 / −152 lines, about −10 net. The deleted timing test and duplicated device setup outweigh the new code; Python adds 208 lines of scripts.
- `just verify`: green, 381 tests, 65 s warm (it was ≈25 s; the perf gate adds ≈15 s and peer builds load the machine).

Not done, open:
- **Lib split of `crates/app` deferred.** It is a pure move (no module refers back to `main.rs` items), but 4 peer sessions were editing the tree. Phase A does not need it, because `engine_sim` carries its own benches.
- **`engine_sim` is created in A1 with its first code**, not as an empty shell.
- **sccache** is not installed, so the 10 GB cap does not apply.
- **`just clean` has not been run**, because peers were building into `target/`; 58 GB is free.

Token rationale: today an agent cannot measure performance at all, so perf claims cost a manual capture session. After this, a perf question is one command with a numeric answer.

### Phase A: GPU-resident, scale-aware cellular world · +6k to +9k / −1.5k lines · risk: high · 5–7 sessions

**A1. Material table v2** (data only, plus loader and validation).
- Add columns to `assets/materials.ron`:
  - `thermal: (conductivity, heat_capacity, melt_k, boil_k, ignite_k)`;
  - `electric: (conductivity, …)`;
  - movement class, derived from `matter_state`, `density` and `viscosity` (already present, unread).
- The loader builds a packed 256-row `GpuMaterial` table: one struct, uploaded once, shared by the CPU and GPU kernels.
- The validator reports file:line for parse errors (ron positions) and for cross-reference errors (it finds the row by `id:`).

**A1 actuals (2026-10-22).**
- Rows in `assets/materials.ron` gain an optional `sim:` block (24 of 83 rows carry one; +408 lines, since generic rows omit it).
- `engine_core::material` holds `MaterialId`, `MovementClass` and the packed 28-byte `SimMaterial`. This is a deviation from the plan: the types live in core, not a new `engine_sim` crate, so the future sim crate depends only on core and cannot name `BlockId`. `engine_sim` is created in A2 with the brick store.
- `engine_content` builds the 256-row `SIM_TABLE` and validates the file at startup and in tests, citing `assets/materials.ron:LINE`.
- **Finding:** Dirt, Clay and Mud are `Powder` in the table, and today's sim never moves them. Deriving movement from `matter_state` alone would make terrain collapse, so cohesion is a real property (cohesive powders resolve to Solid).
- **Ambient problem:** values are game-tuned to `thermal.rs` (ice melts at 308.15 K), because the ambient is 293.15 K. The science profile replaces them in Phase D.
- Packing uses only IEEE basic operations, and 373.15 K is exactly half a step (3731.5 tenths), so it rounds to 3732 on every machine.
- Nothing reads the new columns yet; scenario hashes are unchanged, 389 tests pass.

**A2. Brick store** in `engine_sim::bricks`.
- Cell record: `material u8 | volume u8 | temperature u16 | pressure i16 | charge i16` (8 B/cell, 4 KiB per 8³ brick; the earlier `aux u16` summed to 6 bytes).
- Sparse brick pointer table, a pool allocator, and conversion to and from `ChunkData` plus the water map.
- Proptests: round-trip, coordinate conversions, pool reuse.

**A2 actuals (2026-10-22).**
- New crate `engine_sim` (`crates/sim`, deps: `engine_core`, `bytemuck`): `Cell`, `Scale`, `BrickCoord`, `BrickStore` with a per-chunk `u64` dirty mask, a pointer table and a fixed-capacity pool. A layout test enforces that it depends on no engine crate but core.
- Chunk conversion is `engine_voxel::{load_chunk, read_chunk}`, round-tripped with proptest over random chunks including partial water. The gas amounts in `ThermalCell` are not part of the cell yet.
- `crates/sim/benches/bricks.rs`: `brick_fill_chunk` ≈ 70 µs, `brick_set_32k_cells` ≈ 0.75 ms. Baselines committed.
- The perf gate flaked while peers compiled (+16% to +118% on a bench that reads +0.9% when quiet), so a failing pass is now measured once more before it fails (ADR 0006).
- Not done: neighbour wake across brick faces (A3, with the kernel that needs it); `Scale` is stored but only `FINEST` is produced; the mirror lag (A6).
- 403 tests pass; scenario hashes unchanged (nothing in the game reads bricks yet).

**A3. CPU reference kernels.** (Requirement carried from the A2 review: a cell change on a brick face must wake the neighbouring brick or chunk that reads across it, because a clear dirty bit means "not dispatched at all". A3 tests a boundary change next to an idle brick.) These are what agents debug against.
- Margolus 2×2×2 block update with an offset alternating per tick, so each block is updated by one thread and mass conservation is a per-block invariant.
- Movement rules derived from properties: density orders swaps; viscosity becomes a hash-threshold flow chance; state picks the class (powder, liquid, gas).
- Integer heat diffusion on a ping-pong temperature field.
- Pressure as a relaxed scalar field.
- The engine crate never names `BlockId`: it depends on `MaterialId(u8)` and the table only, so the compiler enforces that the CA is property-driven.

**A3 actuals (2026-10-22).** Movement, finer volume levels and heat conduction are done and verified on the CPU reference. The pressure field is re-scoped: see the last bullet, which needs a decision.
- `engine_sim::step` is the CPU reference tick: two Margolus phases over 2×2×2 blocks, driven only by the packed material table (`rules.rs`: density order, same-liquid fill-down and levelling, loose-powder slides; `pinned.rs`: the hash and phase order). Cohesive powders are `Solid`, so dirt never moves.
- Tests, on a synthetic table and on the real one: mass conserved per material, sand piles and the world then costs nothing, sand sinks through water, smoke rises, dirt stays, matter crosses negative chunk faces, dirty-driven stepping equals waking every brick every tick (random scenes), every block is listed once (oracle), and a pinned canonical hash that fails when the hash or phase order changes (both mutations checked).
- `SimMaterial` grew to 32 bytes: density is a `u32` in 0.01 kg/m³ so smoke (0.6) and steam (0.4) order against air. Air's density in `materials.ron` is now its real 1.2 kg/m³ (was 0; nothing else reads it for air).
- **Exit criterion, measured (benches pinned to one thread per performance core):** one tick over 262,144 awake cells takes 16.3 ms (about 61 ticks/s; target 16.7 ms), so the movement kernel meets the 250k-cells-at-60-ticks/s criterion with about 2% to spare, and an average of 18.1 ms when every cell also conducts heat (28.8 ms on the one tick in four that conducts). Getting there took an in-brick fast path, a per-chunk pointer cache for blocks that straddle faces, lazy dice, a duplicate-free block list, `codegen-units = 1` in the bench profile, and pinning the benches (below). The CPU reference is single-threaded; the heat-everywhere case is what the GPU kernel is for.
- **Water levelling, fixed:** a cell's volume is now 64 levels, not 8 (`VOLUME_FULL`, still one `u8`; the old water map's eighths scale by exactly 8). Liquids move only when neighbours differ by two levels, so a settled tank keeps at most one level (1/64 of a cell) of slope per column: 8 levels, an eighth of a cell, across ten columns, settling in 50 ticks. At eight levels that was about a cell of tilt.
- **Heat conduction, built** (`rules.rs` `conduct`, the 12 edges of each block, after matter moves): energy-based, so a cell's energy is its volume levels × `thermal_mass` × temperature, and the energy `q = g·d` that leaves the hotter cell enters the colder, with `g` limited by the harmonic-mean conductivity and by half of what would equalise the pair (they cannot pass each other). A cell stores temperature as a `u16` in 0.1 K and a water cell holds ~3,000× the heat of an air cell, so a transfer is a fraction of a step: each cell takes the whole steps plus one more with exactly the remainder's probability, from the positional hash. Energy is therefore conserved in expectation, bit-reproducibly, and a closed foam-wrapped bar drifts under 0.2%. Empty space is the environment, a sink at its stored temperature that does not warm up, so a hot object cools without spreading heat through an ever larger volume of air and a world of air costs nothing. A difference under 0.2 K does not conduct (a deadband), so a thermal field goes idle. The arithmetic is `u32` plus two 64-bit-intermediate primitives (`mul_div_rem`, `mulhi`) that the GPU port must reproduce from 32-bit halves. Heat is not computed every tick: it conducts every `HEAT_TICK_INTERVAL` = 4 ticks (0.32 s of simulated time at 1 m cells) and each step carries the four ticks' energy, so the rate in simulated time is unchanged; a tick without heat leaves whatever was awake awake, and the sleep decision is made on a heat tick. Cost, measured pinned: a uniform-temperature block skips heat entirely; with a temperature spread in every cell the 262k-cell tick averages 18.1 ms against 16.3 ms without, but the work lands on every fourth tick (28.8 ms there, 16.3 ms between), so a caller with a hard frame budget should spread it (A6); dense thermal fields still belong on the GPU.
- **Pressure: not built, and it needs your decision.** The reason to have a pressure field was levelling, and finer volume levels fixed that. What genuinely needs pressure is communicating vessels (a U-tube), where water must be pushed up a column by a head elsewhere. That needs a weakly compressible liquid, in which a cell can hold more than `VOLUME_FULL` under pressure (the table's `compressibility` and `pressure_response` already describe it), which breaks the invariant that a cell never exceeds full. A hydrostatic field computed today would have no consumer. Options: (a) build the compressible liquid as its own step (A3c) with the `pressure` field as the compression; (b) leave pipes and pumps to Phase C's entity machinery and drop pressure from the cellular rules. I recommend (a) only if U-tubes matter for the science sandbox; otherwise (b).
- **For A6:** gas rises for ever in an unbounded store (a lid in the test), so the live domain needs a ceiling or smoke needs a lifetime (A4 reactions).
- **For A6:** `step` takes no `Budget` and always runs every dirty brick (CLAUDE.md: bounded passes take one). Fine for tests and headless; the windowed caller needs a budgeted wrapper that stops between whole ticks, not inside a phase.
- **Independent review of the kernel** (read-only, 7 categories): no defect at confidence 80 or above. Taken from its lower-confidence items: the equivalence test now wakes every brick of the 3×3×3 chunks around the scene (it woke one chunk, so a missed cross-chunk wake would have hit both arms alike); a block that needs more new bricks than the pool has now allocates none (it used to take the free ones and leak them empty), with a test and a mutation check; `step` takes `&[SimMaterial; 256]`; the fall uses a saturating subtraction; the slide shift cannot overflow a 32-bit `usize`. The pinned hash did not move.

**A4. Reactions.**
- `assets/reactions.ron` rows: `A + B [+ temp/pressure/field condition] -> products + energy`.
- The rows compile to a 256×256 pair-index table plus a condition list, evaluated in the same Margolus pass. This replaces the 6 thermal reactions; the spell reactions move in Phase D.

**A4 actuals (2026-10-22).** Phase changes, burning and contact reactions run in the CPU reference kernel. Nothing new is in Rust per material.
- **Two sources, one pass** (`engine_sim` `react.rs`), on heat ticks after conduction:
  - The material table drives melting, boiling, condensing (deterministic at the tabled point) and burning (at or above `ignite`, spent into `burns_into` with chance 1/`fuel` per heat tick, so `fuel` is the expected burn time in heat ticks).
  - `assets/reactions.ron` holds pair and single-cell rows: reactants (a block, or `Burnable` = every material with an ignition point), optional temperature bounds on `a` (kelvin or `Ignition`), a rate per heat tick, products and product temperatures.
- **One roll per heat tick:** each lattice edge lies inside a block of exactly one of a heat tick's two Margolus phases, so a pair rolls once. Every cell lies in a block of both, so single-cell rows and burning roll on the first phase only (an independent review caught them rolling twice, which made fire last half as long as written; a test over 1,024 flames now pins it). Phase changes need no roll and are checked on both phases.
- **Order inside a block:** pair reactions along the 12 edges first, then single-cell rows, burning, and phase changes. Contact wins: lava touching water becomes basalt even though conduction has already brought the water past boiling. A cell's material changes at most once per pass, but a row that only re-heats a cell (a burning log lighting the air) does not use up its change.
- **Shared types:** `engine_core::reaction` holds the packed 16-byte `Reaction`, the `ReactionTable` (pair index stored in both orders, with the condition kept on the cell it was written for) and `SimRules` (materials + reactions + a per-material "can change" flag, so a block of inert matter skips the pass). `step` and `block_rule` now take `&SimRules`, and `engine_content::SIM_RULES` is the real one.
- **The compiler** (`engine_content` `reactions.rs`) validates every row and cites `assets/reactions.ron:LINE`:
  - rate must be in (0, 1]; temperatures in range; `Ignition` needs an ignition point;
  - a single-cell row has no `b_into`; `Burnable` is allowed as `a` only; bounds must not cross;
  - duplicate reactants are refused, counting both orders;
  - a rate that rounds to zero, a row that changes no material (its condition would never clear and the brick would never sleep), a self-pair, and empty space reacting on its own are refused;
  - a `Burnable` row reports each problem once.
- **Shipped rows:** lava + water → basalt + steam at 383 K; fire + water → air + steam; a burnable at its ignition point next to air → fire at 1,100 K in the air cell, which holds the fuel at 900 K so it burns until spent; fire → smoke (rate 0.1); smoke → air (rate 0.02). The last gives smoke the lifetime A3 asked for. The 5 rows compile to 18.
- **Tests:**
  - synthetic chemistry in `engine_sim` (12, including the once-per-tick rate): tabled points with hysteresis, volume kept, nothing between heat ticks, lava/water, quench, a burn that ends in ash with no fire or smoke left, cold fuel asleep, seed-reproducible;
  - the real tables in `engine_voxel` (4);
  - the compiler (7, plus the round trip).
- The canonical scenario hash did not move: its synthetic table has no thresholds and no rows.
- **Cost, measured pinned before peers loaded the machine:** `ca_step_262k_cells_all_awake` +2.9% and `…_with_heat` +6.0% against baseline (gate 8%). The bench table has no reactive material, so this is the price of the per-block "can change" test.
- **Finding, steam does not survive open air.** A gas cell sheds half its temperature difference to empty space per exposed edge on every heat phase (the A3 environment sink, capped at half the cell's capacity). So steam born at 383 K condenses back to water within the same tick, and fire + water effectively just removes the flame. A pinned test (`steam_in_open_air_condenses_within_a_tick`) fails when this changes. Persistent steam needs either latent heat (condensing releases heat) or a gas-to-air exchange that scales with what the gas displaces. That is a heat-model decision for you, alongside the A3 pressure question.
- **For A6:**
  - World lava loads at ambient temperature (the old sim forced 1,200 °C), so lava needs a spawn temperature, a table column or the loader.
  - Reactions are not energy-conserving (combustion and reaction temperatures are set, not derived), so the A7 ledger needs a source term per reaction.
  - Fuel units moved from 30 Hz steps to heat ticks: a log now burns about 160 × 0.32 s ≈ 51 s at 1 m cells, against about 5 s before. Retune `fuel` in `materials.ron` once it is visible in game.
  - `thermal.rs` and the spell reactions are still the live path until A6 wires the kernel.

**A5. GPU kernels.**
- A WGSL port of A3/A4.
- Per-brick dirty bits written by the kernel, compacted into an active list, with `dispatch_workgroups_indirect` (core wgpu, no feature flag), so an idle brick costs zero dispatch.
- Equivalence: for 64 seeds of random bricks, run N ticks on CPU and GPU and compare bytes (`just test-gpu ca_equivalence`).

**A5 actuals (2026-10-22).** The kernel runs GPU-resident and matches the CPU reference byte for byte.
- **New crate `engine_sim_gpu`** (`crates/sim_gpu`). It depends on `engine_core`, `engine_sim`, wgpu, bytemuck and glam; the dev-dependencies are naga and pollster, already in the workspace, so no new dependency.
- **The WGSL port** is `pinned.wgsl`, `rules.wgsl` and `react.wgsl`, matching the Rust modules one for one:
  - u32 arithmetic only;
  - the 64-bit product in the heat rule is built from 16-bit limbs, with a 64/32 long division;
  - the material table and reactions are packed into one `u32` buffer by `pack_rules`, field by field;
  - a test checks the shader's offset constants against the packer.
- **Shared scheduler:** `engine_sim::step_with` is the tick driver both backends share. `step` passes the CPU phase, and `gather`/`scatter` (`engine_sim` `offload.rs`) split a phase for an accelerator. A CPU-only test proves gather → rule → scatter equals `step` over 12 seeds × 24 ticks.
- **Two backends:**
  - `GpuBlockRule`, the offload path: the CPU gathers each phase's blocks and scatters the results. It is kept for the per-block test.
  - `GpuWorld`, the resident path: the pool, a directory over a window of chunks, and five awake bitmaps (dirty, A, S, W0, W1) live on the GPU. A tick is one submission of nine dispatches with no readback:
    - `bits` START;
    - `compact` (active list), `finalize` (2-D indirect arguments) and `blocks` (indirect, one workgroup per active brick; a block that overlaps several active bricks is stepped by the lowest-indexed one) for phase 0;
    - `bits` MID;
    - `compact`, `finalize` and `blocks` for phase 1;
    - `bits` END.
- **How the resident path reproduces `step_with`:** the bitmap passes apply the same set algebra, with "present" read from the directory. A brick a block writes into for the first time takes a slot from a free stack (compare-and-swap); the loser of a race for one brick returns its slot, recycled between phases, so no invocation ever waits on another. Waking only present bricks at the end of a phase equals the CPU's wake-at-the-time, because a brick only becomes present when a changed block writes it, and that block wakes it anyway.
- **Equivalence, run on an RTX 4090 (Vulkan):**
  - `gpu_block_rule_matches_the_cpu_per_block`: 200k random blocks × 7 phase counters, hot and cold, on the real tables;
  - `gpu_ca_equivalence` (offload) and `gpu_resident_equivalence`: 64 seeds × 40 ticks of random worlds across negative chunks, comparing awake bricks every tick and every cell at the end. All pass.
  - Mutation checks: a changed dice salt fails the per-block test at block 29; dropping the no-heat re-wake fails the resident test at seed 0, tick 17.
- **Limits, flagged rather than diverging:**
  - `Snapshot::overflow` is a bitmask: bit 0 when matter tried to leave the window, bit 1 when the pool ran out. The CPU's stall-on-full order cannot be reproduced in parallel, and a slot that lost an allocation race returns only between phases, so equivalence needs a little pool headroom. `gpu_resident_overflow_is_reported` drives both cases.
  - the window and pool must fit `max_storage_buffer_binding_size` (`UploadError::TooLargeForDevice`).
- **Independent review** (allocator CAS and ABA, scheduling algebra, owner rule, index codec, indirect arguments, port fidelity line by line): no blocking issue. Fixed from it:
  - a false pool overflow when a block lost the last free slot to another block claiming the same brick (it now re-reads the directory);
  - `overflow` as a bitmask, so neither cause hides the other;
  - empty store entries outside the window no longer refuse the upload;
  - size checks cover every buffer and reject an empty or overflowing window.
- **Speed (`gpu_resident_timing`, every brick awake, mixed materials on the real tables):**

  | Cells | CPU reference | GPU resident | Speed-up |
  |---|---|---|---|
  | 262,144 | 15.5 ms | 0.41 ms | about 38× |
  | 1,048,576 | 51.5 ms | 0.98 ms | about 52× |

  - Both pairs were measured on a quiet machine; under peer builds it was 27–40 ms against 0.5 ms, and 88–124 ms against 1.7 ms.
  - The 1M-active-cells target is met: about 17× headroom inside a 60 Hz tick.
  - The offload path is slower than the CPU (46 ms against 17 ms at 262k), because its gather, scatter and bus round trips dominate. It is a test vehicle, not a backend.
- **For A6:** `GpuWorld` is the `CaBackend` candidate. It still needs edits as commands (writes into the GPU pool), the 2-tick-lagged CPU mirror (an async `read_back` of dirty bricks only, not the whole pool), and the window following the player (re-centring moves directory entries, not cells).

**A6. Integration.**
- `WorldSimulation` drives the `CaBackend`; edits become commands; the L = 2 mirror feeds gameplay and the mesher.
- Delete `cellular.rs`, `thermal.rs`, the water map, cell-level `activity.rs`, `fluid_compute.wgsl` and the dead sandbox thermal map: one mechanism.
- Rigid bodies stay CPU on the mirror. **Deviation from the mission:** GPU collider extraction is deferred until bodies exceed today's 64 × 128 cap (ADR). Today's load does not pay for a GPU connected-components pass.

**A7. Scale boundaries** per D2: the boundary pass, depth-aware tick rates, and the conservation ledger. The ledger tracks thermal energy as well as mass (ADR 0007 rule 6): merges sum `capacity × volume × temperature`, they never average temperatures.

**A8. Ballistic particles** (added 2026-10-22). This is Noita's splash technique. An impulse (explosion, rigid body or spell) lifts cells out of the grid into flying particles, and they re-enter the grid when they land.
- `engine_sim::particles`: a CPU SoA table with fixed-point positions and velocities (i32, 1/256 voxel). Floats are kept out so ADR 0008 still holds for anything that carries mass. The table has a hard cap and is ticked within the CA `Budget`.
- Leaving and landing both go through the edit-command queue (A6). A particle lands in the first cell along its path that has room; if none has room, it stays in flight and tries again the next tick.
- The A7 ledger counts in-flight particles, so mass is conserved across the whole cycle.
- Render: instanced quads, drawn from the same mirror the mesher reads.
- Out of scope: momentum inside the CA, and particles colliding with each other.

Accept:
- `just run assets/scenarios/<s>.ron` for `sand_pile`, `water_pool`, `fire_spread`, `gas_rise`, `lava_water_steam`, `lod_crossing` and `splash`. `lod_crossing` asserts mass and energy totals across the boundary through new `Expect(Mass{material, region, eq})` and `Expect(Energy{region, eq})` checks. `splash` asserts mass totals before and after an impulse, with particles included.
- `just test-gpu ca_equivalence`.
- `just bench ca_1m_active`: 1M active cells, CPU reference and GPU numbers both recorded.
- `just verify`.
- Before/after captures of water and sand, since the visible behaviour changes.

Token rationale:
- **Today**, a new reactive material (say, acid dissolving stone) means a `BlockId` variant, a RON row, match arms in `thermal.rs` and `cellular.rs`, and spell code. That is about 5 files and 100–200 changed lines, with roughly 3k lines read for context.
- **After**, it is a material row and a reaction row plus a scenario: about 30 lines in 2–3 data files, no Rust, no recompilation of the kernels.
- Debugging a sim bug becomes a CPU reference test on an 8³ brick, never a GPU capture.

### Phase B: rendering that doesn't fight the sim · +4k to +7k / −3k to −6k lines · risk: high · 4–6 sessions

**B1. Shared infrastructure.**
- One include system (`naga_oil`, or the fallback per D6).
- One bind-group table in `engine_gpu`: a Rust table generates the WGSL group declarations, and a test checks every program's bindings against it, replacing 15 ad hoc layouts.
- A pooled vertex/index arena replacing per-chunk buffers.
- Storage buffers and indirect draws enabled.

**B2. Spike, on a `spike/render` branch.** One `TerrainRenderer` trait over the brick pool with two implementations:
- (a) GPU meshing: a compute face/greedy mesh into the arena, packed 8-byte quads instead of 248 B, GPU frustum and Hi-Z cull writing `draw_indexed_indirect` args (multi-draw where the adapter supports it, a loop of indirect draws where it does not);
- (b) brick-map ray-marching: DDA over the 8³ pointer table, with a visibility buffer.

**B3. Decision rule, fixed before benching.**
- Pick the lower p95 frame time on the `render_*` bench scenes at 1 m and 0.25 m with view distance 28, at equal golden image (diff below threshold).
- On a tie (within 5%), the one with fewer lines wins.
- Numbers go in ADR 0013, the loser is deleted, and the winner merges as one commit.
- Known cost asymmetry, stated now so the numbers decide and not the effort: ray-marching must re-host the 3,264-line `world_shader.wgsl` lighting model, while GPU meshing keeps it.

**B4. Wiring.**
- GPU LOD selection; shadow cascades draw from the same indirect lists.
- No per-chunk CPU work in steady state, enforced by a bench counter that asserts zero CPU mesh jobs in a static scene.
- The horizon backdrop stays as is in this phase.

**B5. Debug overlay** fed from GPU counters: active bricks, dispatches, draws and per-pass timestamps. The existing `GpuProfiler` is the source. It also draws a world-space view of the dirty-brick bitmap, one wire box per set bit, so players and agents can see which regions are being simulated. Today the overlay shows only the number `active_chunks`.

Accept:
- `just test-gpu golden`: offscreen renders of each Phase A scenario, compared against PNGs per adapter family with a perceptual threshold;
- `just bench frame_time` with a committed baseline;
- before/after captures (`docs/design/VISUAL_DIRECTION_EXECUTION.md`);
- `just verify`.

Token rationale:
- Shader edits stop requiring knowledge of the concatenation order.
- A new pass is one table row instead of a new layout plus three call sites.
- Golden tests turn "did I break the look?" from a manual capture into a test result.

### Phase C: automation and electronics on the same substrate · +3k to +5k / −2k lines · risk: medium · 3–4 sessions

1. **Power and signals.**
   - Connectivity comes from material `electric.conductivity`: wires are cells, not entities.
   - The network is solved as a graph: union-find over conductive cells, rebuilt only for dirty bricks.
   - **Not** as a diffused charge field, which propagates one cell per tick and makes long wires visibly slow. Heat stays a field. ADR.
2. **Machines** stay entities, as discrete semantics, in SoA tables (D5). Each reads and writes cells through the same edit-command queue as the player.
3. **Scheduler.** One explicit tick scheduler: an ordered list of `(system, budget)` built from one table that `main.rs` and `headless::Session::tick` both call. This replaces the hand-maintained duplicate order, so "headless and main must agree" becomes a single call site.
4. **Inventories** become flat arrays; recipes stay in `assets/recipes.ron`.
5. **Save format.** New machine behaviours prefer modes and fields on existing kinds (the save-format rule), and kinds are appended only with a version bump.

Accept:
- Debug-build conservation invariants for items and energy run after every scenario tick: a test hook, not a comment.
- `just bench factory_n` with N ∈ {100, 1k, 10k}.
- Scenarios `belt_line`, `powered_smelter`, `logic_gate`.
- `just verify`.

Token rationale: a new machine is a RON row plus a mode in most cases. The scheduler table removes the most common cross-file mistake: changing tick order in one loop and not the other.

### Phase D: unified rules model · +3k to +5k / −3k to −5k lines · risk: high · 4–5 sessions

**D-1. One schema in `assets/rules/*.ron`:** `Rule { trigger, conditions, effects }`.
- Effect primitives, a fixed and documented set: `SetCell`, `ApplyField`, `SpawnEntity`, `EmitEvent`, `PrintText`, `GiveItem`.
- Magic is a rule with a `Mana` condition; chemistry is a rule with a `Temperature` condition.

**D-2. Two evaluators selected by locality, one schema:**
- Cell-tier rules (trigger `Contact`/`Tick`; cell-local conditions; effects `SetCell`/`ApplyField` on the pair) compile into the Phase A reaction table and run on the GPU.
- Event-tier rules (`OnEvent`, `OnEnter(place)`, `OnCast`) run on the CPU event bus.
- The validator rejects a cell-tier rule whose effects are not cell-local, so the split is a load-time error, not a convention.
- Phase A's `reactions.ron` becomes the cell tier of this schema; there are never two reaction formats.

**D-3. Spells.**
- The wand deck evaluator (`wands.rs`) stays code: it is the deck semantics.
- Spell *effects* (`spells.rs`, including its 7 hard-coded reactions) become rules.
- `growth.rs` and `wand_xmb.rs` are not touched (CLAUDE.md).

**D-4. Profiles.**
- `assets/profiles/{science,fantasy}/` overlay a shared core. Science uses real constants, real names, units shown and no mana; fantasy uses different tables.
- Scenario checks are written against properties and tags (`Expect(State{offset, is: Liquid})`) rather than names, so one suite runs under both: `just run-all science` and `just run-all fantasy`.

**D-5. Validation.** Every asset file is schema-checked and cross-referenced at startup and in a test; errors cite file:line.

Accept: `assets/scenarios/data_only.ron` plus rule files demonstrating a reaction, a machine, a spell and a narrative event, with zero Rust in the diff (a test asserts that the scenario's rules reference only loaded data). Both profiles pass the suite. `just verify`.

Token rationale: this is the phase that moves most future features from Rust to data. An agent adding a spell, reaction or quest beat reads one schema doc and writes rows; the validator's file:line errors replace compile-and-guess loops.

### Phase E: text layer · +2k to +3k lines · risk: medium · 2–3 sessions

**E-1. One `Action` enum**, serde, the single input surface:
- GUI input maps to actions;
- scenario steps (`Place`, `Hold`, …) become actions;
- the REPL parses into actions.

A JSON schema of `Action` is generated, by a test-checked hand table or `schemars` per ADR, so an LLM can call the same API.

**E-2. World query.**
- `look(region)` returns nouns at or near a position.
- `verbs(position)` returns the legal actions.
- `describe(...)` builds text from material properties through `assets/descriptors.ron`: property ranges map to words, for example `temp > 330 K → "steaming"` and `tag sulfur → "smells of sulfur"`. No hand-written strings per object.

**E-3. Places** are named regions: `assets/places.ron` plus worldgen sites exported as places. Narrative events are Phase D event-tier rules.

**E-4. Running it.** `just play [scenario]` opens a headless REPL. `just run` accepts `.txt` transcripts (commands plus expected output) and snapshots them with `insta`.

Accept: `assets/scenarios/text_walkthrough.txt` mutates the voxel world (dig, pour, cast). It is verified by the same world-hash snapshot as the RON scenarios, and an `Action` round-trips through JSON. `just verify`.

Token rationale: the agent's playtesting interface becomes text in, text out, with no capture session. Every bug report can be a transcript.
- `inspect(cell)` returns material, state, temperature, pressure and the reaction rows that could fire. The GUI shows it as an inspect tool, through the same `Action`, so a player can see why something burned, melted or flooded instead of guessing.

### Phase F: make the next thousand changes cheap · +1k to +2k lines · risk: low · 1–2 sessions

1. **Scaffolds:** `just new-material`, `just new-machine`, `just new-spell` and `just new-scenario` write the data row, a scenario and a test stub, with no Rust.
2. **Skills:** each `.claude/skills/*/SKILL.md` gains "add X in ≤N lines", checked by a test that runs the scaffold and counts the diff.
3. **Proptests** for bricks, coordinates and scale transitions (started in A2, completed here).
4. **Mutation testing:** `cargo-mutants` on `engine_core`, `engine_voxel` and `engine_sim`, with the score reported in `just session-report`.
5. **Roadmap:** `docs/ROADMAP.md`, one page, ranked by player value ÷ cost with a data-vs-code estimate per item.

The perf gate and `session-report` already landed in Phase 0.

Accept: each scaffold produces a passing `just verify` with no manual edits; mutation score recorded.

Token rationale: the common cases (new material, machine, spell, scenario) become a single command plus value edits.

## 3. How each design constraint is enforced

| Constraint | Mechanism |
|---|---|
| CA is property-driven | `engine_sim` has no dependency on the `BlockId` enum (compiler) |
| CPU and GPU agree | `ca_equivalence` byte-compare test |
| Idle costs zero | Bench counter: dispatches in a static scene = 0 |
| No per-chunk CPU render work | Bench counter: CPU mesh jobs in a static scene = 0 |
| Headless = windowed order | One scheduler table, one call site (Phase C) |
| One binding table | Test compares every program's bindings to the table |
| Conservation | Debug invariant hook after each scenario tick; the ledger counts mass, thermal energy and in-flight particles |
| Rules are tier-correct | Load-time validator error |
| Perf | `bench-check` in `just verify` |
| Docs name real things | Existing `agent_docs_name_only_existing_paths_and_commands` |
| Brick format, scale rules, determinism, authority lag, no-ECS, rigid on CPU, renderer choice | ADRs 0007–0011, 0013 (not enforceable in code) |
| Hash and Margolus order pinned | Canonical CA scenario hash test (ADR 0008) |

## 4. Risks

- **Size.** Roughly 20–28 sessions in total. A and B are each larger than the whole revamp. Mitigation: each sub-step lands green on its own, and old systems are deleted only after their scenarios pass on the new one.
- **Disk.** ~99% full, with 14 GB cold target dirs. Spike branches share `./target` and are not worktrees; benches avoid a second target dir.
- **Concurrent sessions edit the tree.** Each phase confirms single-writer before starting.
- **GPU availability.** `just test-gpu` is local only, with no CI. Goldens are per adapter family, and a new adapter records new goldens rather than failing.
- **Visible behaviour change.** Water flow, sand piles and fire look different after A6. Captures before and after; the user signs off before the old CA is deleted.
- **Ray-march spike cost.** It must re-host a 3,264-line shading model; the spike may only reach a lighting-reduced parity. The decision rule accounts for that by comparing at equal golden images.
- **5% perf gate noise** on a shared workstation. Mitigated by interleaving and two-strike failure. If it still flakes, the gate moves to GPU timestamps and counters, which are steadier than CPU wall time.
- **Spell migration** touches 2,370 + 1,060 lines that players feel. Per-spell parity scenarios come before deletion.

## 5. Order and stop points

Phase 0 → A (A1–A8) → B → C → D → E → F, one phase per session group, with `/compact` between sessions. Each phase ends with LOC delta, bench numbers, scenarios added, open questions and `just verify` green, appended here.

Stops for the user:
- D1–D7 before Phase A;
- capture sign-off before deleting the old CA (A6);
- the renderer ADR before deleting the losing spike (B3);
- the save v29 bump;
- each new dependency.
