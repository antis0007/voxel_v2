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

Material properties for the cellular rules: a block's `sim:` row in `assets/materials.ron` (cohesion, thermal conductivity and capacity, melt/boil/ignite/cool points and what they turn into, fuel, electrical conductivity), in real units. Rows equal to the generic inert solid omit it. `engine_content` packs every row into the 28-byte integer `engine_core::material::SimMaterial` (`BlockId::sim()`, `SIM_TABLE`); movement class comes from `matter_state` plus cohesion, never from the block. To add a reactive material: a `BlockId`, its row with a `sim:` block, then `cargo test -p engine_content dump_materials_ron -- --ignored`. Errors cite `assets/materials.ron:LINE`. Nothing reads these columns until Phase A3 (`docs/PLAN2.md`); the current sim still uses `thermal.rs` constants.

Test: `just test water` and `just test simulation` for the harness tests (they call `cellular::step` directly with an empty `Residency`); scenarios `assets/scenarios/water_falls.ron` and `sand_falls.ron` run the full tick order headlessly (`just run ...`). A changed snapshot hash in `just test headless` means behaviour changed.

Pitfalls: placing water through `World::set(Water)` clears the volume map; use `set_water_volume(p, 8)`. The wall-clock timing test for terrain generation is ignored on purpose (see `docs/PLAN.md` debt).
