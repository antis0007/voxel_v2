---
name: physics
description: Cellular water/sand, rigid bodies and thermal in crates/app/src/simulation — budgets, activity wake rules, how to test without a window.
---

# Physics

Where: `crates/app/src/simulation/` — `world.rs` (`WorldSimulation::update`, the per-tick driver), `cellular.rs` (water and sand), `rigid.rs`, `thermal.rs`, `residency.rs` (chunk generation workers the sim reads from), `sandbox.rs` (rigid body state). Water volumes live in `crates/app/src/world/water.rs` (0–8 per cell); wake bits in `world/activity.rs`.

Invariants:
- Fixed step 1/120 s; the cellular loop runs at its own interval (`0.08 * scale`, 30 Hz floor) inside the step.
- Every bounded loop checks a `Budget`; never add `Instant::now()` to a step. The window passes `WorldSimulation::live_budget()`, headless passes `Budget::UNBOUNDED`.
- A cell only steps when its chunk is awake (`activity`) and resident in `Residency`; edits wake neighbours (`World::wake_acting`). The scheduler takes a snapshot of pending bits per tick so a front cannot starve.
- Mass is conserved per transfer (`water_tests.rs` asserts it on ledges and drains).

Cellular kernel (Phase A, not wired into the game yet): `crates/sim/src/` (`engine_sim`). `step(store, rules, seed, tick)` runs two Margolus phases over 2×2×2 blocks of the 8³ brick store; `rules.rs` is the block rule (density order, same-liquid fill and levelling, powder slides, all from the packed material table), `pinned.rs` the hash and phase order that fix every result. `just test sim` runs its scenarios; `just bench ca_` times a 262k-cell tick. Cells hold 64 volume levels (`VOLUME_FULL`); heat conducts along block edges in energy with stochastic rounding (`conduct`) every `HEAT_TICK_INTERVAL` ticks, and empty space is a heat sink. On heat ticks `react.rs` then applies pair reactions, single-cell rows, burning and the table's phase changes (`SimRules` = materials + compiled reactions, `engine_content::SIM_RULES`). Add a behaviour as a rule over properties or a row in `assets/reactions.ron`, never over a block.

Material properties for the cellular rules: a block's `sim:` row in `assets/materials.ron` (cohesion, thermal conductivity and capacity, melt/boil/ignite/cool points and what they turn into, fuel, electrical conductivity), in real units. Rows equal to the generic inert solid omit it. `engine_content` packs every row into the 32-byte integer `engine_core::material::SimMaterial` (`BlockId::sim()`, `SIM_TABLE`); movement class comes from `matter_state` plus cohesion, never from the block. To add a reactive material: a `BlockId`, its row with a `sim:` block, then `cargo test -p engine_content dump_materials_ron -- --ignored`. Errors cite `assets/materials.ron:LINE`. To add a reaction: a row in `assets/reactions.ron` (reactants, optional temperature bounds on `a`, rate per heat tick, products), then `cargo test -p engine_content dump_reactions_ron -- --ignored`; errors cite `assets/reactions.ron:LINE`, and `just test chemistry` checks the real tables. The live game still uses `thermal.rs` until A6 wires the kernel (`docs/PLAN2.md`).

GPU kernel: `crates/sim_gpu` (`engine_sim_gpu`). The WGSL in `pinned.wgsl`, `rules.wgsl` and `react.wgsl` ports the Rust modules of the same names and must stay byte-identical: change both sides in one patch and run `just test-gpu gpu_` (per-block, offload and resident equivalence, 64 seeds). `GpuWorld` is the resident backend (pool, directory and awake bitmaps on the GPU, nine dispatches per tick, no readback); `Snapshot::overflow` is nonzero when matter left its window or the pool ran out. `just test-gpu gpu_resident_timing -- --nocapture` prints CPU against GPU tick time.

Test: `just test water` and `just test simulation` for the harness tests (they call `cellular::step` directly with an empty `Residency`); scenarios `assets/scenarios/water_falls.ron` and `sand_falls.ron` run the full tick order headlessly (`just run ...`). A changed snapshot hash in `just test headless` means behaviour changed.

Pitfalls: placing water through `World::set(Water)` clears the volume map; use `set_water_volume(p, 8)`. The wall-clock timing test for terrain generation is ignored on purpose (see `docs/PLAN.md` debt).
