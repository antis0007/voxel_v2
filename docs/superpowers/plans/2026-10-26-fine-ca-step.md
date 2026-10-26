# Coordinated block/fine CA tick Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One `MatterStore::step` that runs the block kernel on uniform matter, a fine Margolus pass inside `R_fine` on awake fine sub-bricks, promotes neighbours that change, demotes settled uniform ones, and reports a budget cut so a governor can narrow `R_fine`.

**Architecture:** The block kernel gains a skip predicate so no 2×2×2 block that touches a fine block runs at 1 m. The fine pass reuses `block_rule` on 2×2×2 fine cells; anchors of one fine block are `4B + {0,2}³` in the even phase and `4B + {-1,1,3}³` in the odd phase, so odd-phase blocks straddle into neighbours, which are read as 64 identical cells and promoted only on change. Child modules of `matter` (`matter/fine_kernel.rs`, `matter/step.rs`, `matter/governor.rs`) see its private fields, as `store/access.rs` does for the brick store.

**Tech Stack:** Rust 2024, `engine_core::Budget`, criterion bench, nextest.

**Spec:** `docs/specs/VARIABLE_SCALE_UNDERGROUND.md` §1.2, §1.4 (fine CA ≤ 3 ms, ≤ 2000 awake sub-bricks, promotion 2000 blocks/tick, hysteresis 60, governor 24 → 16 → 12 m); plan step 2 in `docs/plans/2026-10-25-variable-scale-and-underground.md`; ADR 0023.

## Global Constraints

- `step` on `BrickStore` must produce byte-identical results before and after (GPU parity, scenario hashes).
- No `unwrap`/`expect` outside tests; no `Instant` in simulation (budget only through `Budget`); randomness from `Dice` only.
- Every new file under 300 lines; no comment that restates code; banned words list in CLAUDE.md.
- Fine units: one block volume level = 64 fine units; nothing is destroyed on promotion, stall or demotion.
- `Budget::UNBOUNDED` never truncates: tests and headless stay deterministic.
- Shared tree: commit own files through a private index (`scripts` not needed; see memory recipe). Peer fine-scale files are adopted in Task 0 as one verbatim commit.

## Review Focus

1. A Margolus block that straddles a fine block in the odd block-scale phase but whose anchor brick has no fine bit: the halo set must contain neighbours of fine bricks or the block kernel moves a fine block's stale coarse value (Task 2 test `the_block_pass_leaves_a_fine_block_alone` uses a fine block in the brick next to the water column).
2. Fine pool exhausted mid-block: no partial write, block stays awake, volume conserved (Task 4 test).
3. A fine block outside `R_fine` whose uniform neighbour inside the radius moves at block scale: the neighbour is in a skipped Margolus block, so it freezes too; acceptable, documented in ADR (Task 6).
4. Negative block coordinates: `split_fine` uses Euclidean division; Task 1 anchor test uses a negative block.
5. Heat at fine scale diffuses 16× faster per metre than at block scale (same rate, 4× smaller cells); documented as deferred in ADR 0023 (Task 6), no code.

---

### Task 0: Adopt the parallel session's fine-scale files

**Files:**
- Commit verbatim: `crates/sim/src/fine.rs`, `crates/sim/src/fine_tests.rs`, `crates/sim/src/matter.rs`, `crates/sim/src/matter_jobs.rs`, `crates/sim/src/lib.rs` (working copy), `crates/sim/src/store.rs` (working copy), `docs/adr/0023-fine-sub-bricks.md`, `docs/plans/2026-10-25-variable-scale-and-underground.md`.

- [ ] **Step 1: Confirm `git diff crates/sim/src/lib.rs crates/sim/src/store.rs` holds only the fine/matter hunks seen on 2026-10-26**
- [ ] **Step 2: `cargo test -p engine_sim`** Expected: 63 passed.
- [ ] **Step 3: Commit through a private index** with message `sim: adopt fine sub-brick storage from the parallel session (ADR 0023)`.

### Task 1: Fine anchors and the fine/block split

**Files:**
- Create: `crates/sim/src/matter/fine_kernel.rs`
- Modify: `crates/sim/src/matter.rs` (add `mod fine_kernel;` and `#[cfg(test)] mod step_tests;`)
- Test: `crates/sim/src/matter/step_tests.rs`

**Interfaces:**
- Produces: `pub(super) fn fine_anchors(blocks: &[IVec3], offset: IVec3) -> Vec<IVec3>` (fine coordinates, sorted by `(z, y, x)`, deduped); `pub(super) fn split_fine(fine: IVec3) -> (IVec3, IVec3)` (block, local in `0..4`); `pub(super) fn cell_pos(anchor: IVec3, i: usize) -> IVec3` (same order as `rules::idx`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn fine_anchors_cover_a_block_with_8_even_and_27_odd_blocks() {
    let b = IVec3::new(1, -2, 3);
    let even = fine_anchors(&[b], PHASE_OFFSETS[0]);
    assert_eq!(even.len(), 8);
    assert!(even.iter().all(|a| (*a - b * 4).cmpge(IVec3::ZERO).all() && (*a - b * 4).cmple(IVec3::splat(2)).all()));
    let odd = fine_anchors(&[b], PHASE_OFFSETS[1]);
    assert_eq!(odd.len(), 27);
    assert!(odd.iter().all(|a| (*a - b * 4).cmpge(IVec3::splat(-1)).all() && (*a - b * 4).cmple(IVec3::splat(3)).all()));
    let pair = fine_anchors(&[b, b + IVec3::X], PHASE_OFFSETS[1]);
    assert_eq!(pair.len(), 45, "nine shared face anchors listed once");
    assert!(pair.windows(2).all(|w| (w[0].z, w[0].y, w[0].x) < (w[1].z, w[1].y, w[1].x)));
}

#[test]
fn split_fine_floors_negative_coordinates() {
    assert_eq!(split_fine(IVec3::new(-1, 4, 7)), (IVec3::new(-1, 1, 1), IVec3::new(3, 0, 3)));
}
```

- [ ] **Step 2: Run `cargo test -p engine_sim step_tests`** Expected: compile error, functions missing.
- [ ] **Step 3: Implement the three functions in `matter/fine_kernel.rs`** (`split_fine` via `div_euclid`/`rem_euclid` by `FINE_SIZE`).
- [ ] **Step 4: Run the tests** Expected: PASS.
- [ ] **Step 5: Commit** `sim: fine Margolus anchors for sub-bricks`.

### Task 2: Block kernel skip predicate

**Files:**
- Modify: `crates/sim/src/kernel.rs` (`run_phase`, `step`), `crates/sim/src/lib.rs` (export)
- Test: `crates/sim/src/tests.rs` or `scenario_tests.rs`

**Interfaces:**
- Produces: `pub fn step_skipping(store: &mut BrickStore, rules: &SimRules, seed: u32, tick: u32, skip: impl Fn(IVec3) -> bool) -> StepStats`; `step` becomes `step_skipping(store, rules, seed, tick, |_| false)`. A skipped anchor is not counted in `stats.blocks` and wakes nothing.

- [ ] **Step 1: Write the failing test** (in `scenario_tests.rs`, reusing the tank scenario)

```rust
#[test]
fn step_skipping_nothing_is_step_and_a_skipped_block_never_moves() {
    let (t, mut a) = (table(), BrickStore::new(2000));
    tank(&mut a, IVec3::ZERO, IVec3::new(12, 10, 6));
    fill(&mut a, IVec3::new(1, 1, 1), IVec3::new(4, 7, 5), water(VOLUME_FULL));
    let mut b = a.clone();
    for tick in 0..50 { step(&mut a, &t, 3, tick); step_skipping(&mut b, &t, 3, tick, |_| false); }
    assert_eq!(a.state_hash(), b.state_hash());
    let mut c = b.clone();
    let frozen = |anchor: IVec3| anchor.x <= 4; // every block touching the water
    for tick in 50..60 { step_skipping(&mut c, &t, 3, tick, frozen); }
    assert_eq!(c.state_hash(), b.state_hash(), "skipped blocks change nothing");
}
```

- [ ] **Step 2: Run `cargo test -p engine_sim step_skipping`** Expected: FAIL, `step_skipping` missing.
- [ ] **Step 3: Implement** `run_phase` takes `skip: &impl Fn(IVec3) -> bool` and `continue`s before `run_inside_brick`; add `step_skipping`; `step` delegates.
- [ ] **Step 4: Run `cargo test -p engine_sim`** Expected: all pass (scenario hashes unchanged).
- [ ] **Step 5: `just bench ca_step`** Expected: both `ca_step_*` within +8% of baseline.
- [ ] **Step 6: Commit** `sim: block kernel can skip blocks that fine matter owns`.

### Task 3: One fine Margolus block through `MatterStore`

**Files:**
- Modify: `crates/sim/src/matter/fine_kernel.rs`, `crates/sim/src/matter.rs` (fields `awake: Vec<IVec3>`, `dormant: Vec<IVec3>`; `promote` pushes to `awake`; `pub fn fine_free_slots(&self) -> usize`; `pub fn wake_fine(&mut self, block: IVec3)`; `pub fn fine_awake(&self) -> usize`; `pub fn active_bricks(&self) -> usize`)
- Modify: `crates/sim/src/fine.rs` (`pub(crate) fn blocks(&self) -> impl Iterator<Item = IVec3> + '_` over slots)
- Test: `crates/sim/src/matter/step_tests.rs`

**Interfaces:**
- Produces: `pub(super) const FINE_SEED_SALT: u32 = 0x5EED_F1CE;` `pub(super) struct FineOutcome { pub changed: bool, pub pending: bool, pub stalled: bool, pub promoted: u32 }`; `impl MatterStore { pub(super) fn run_fine_block(&mut self, rules: &SimRules, seed: u32, phase_counter: u32, tick: u32, anchor: IVec3, woken: &mut Vec<IVec3>) -> FineOutcome }`.
- Rule: read 8 cells via `fine_cell`; `block_rule(rules, &mut after, seed ^ FINE_SEED_SALT, phase_counter, anchor)`; if changed, count distinct non-fine blocks among changed cells; more than `fine_free_slots()` → `stalled`, write nothing; else `set_fine` each changed cell (a `PoolFull` here also counts as stalled). When `changed || pending || stalled`, push every fine block the eight cells lie in onto `woken`.

- [ ] **Step 1: Write the failing test** (helpers in `step_tests.rs`: `fine_units(store, lo, hi, material) -> u64` counting a uniform block as `64 × volume` and a fine block as the sum of its cells; `stone_floor(store, lo, hi)`)

```rust
#[test]
fn an_odd_phase_block_spills_fine_water_into_the_uniform_air_beside_it() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    stone_floor(&mut m, IVec3::new(-4, 0, -4), IVec3::new(4, 1, 4));
    let b = IVec3::new(0, 1, 0);
    m.replace_block(b, water(VOLUME_FULL)).unwrap();
    m.promote(b, 0).unwrap();
    let before = fine_units(&m, IVec3::new(-4, 0, -4), IVec3::new(4, 4, 4), WATER);
    let mut woken = Vec::new();
    let anchor = IVec3::new(3, 4, 1); // straddles x = 3 (block 0) and x = 4 (block +X)
    let out = m.run_fine_block(&t, 7, 1, 0, anchor, &mut woken);
    assert!(out.changed && !out.stalled && out.promoted == 1);
    assert!(m.is_fine(b + IVec3::X) && !m.is_fine(b - IVec3::Y));
    assert_eq!(fine_units(&m, IVec3::new(-4, 0, -4), IVec3::new(4, 4, 4), WATER), before);
    assert_eq!(woken, vec![b, b + IVec3::X]);
}
```

- [ ] **Step 2: Run it** Expected: FAIL, `run_fine_block` missing.
- [ ] **Step 3: Implement** per the Interfaces rule.
- [ ] **Step 4: Run `cargo test -p engine_sim step_tests`** Expected: PASS.
- [ ] **Step 5: Commit** `sim: one fine Margolus block reads uniform neighbours and promotes on change`.

### Task 4: `MatterStore::step` with region, cap, budget, sleep and demotion

**Files:**
- Create: `crates/sim/src/matter/step.rs`
- Modify: `crates/sim/src/matter.rs` (`mod step;`, `pub fn state_hash(&self) -> u64` = brick hash folded with fine cells in `(z, y, x)` block order), `crates/sim/src/lib.rs` (export `FineStepStats`, `MatterStepStats`, `MAX_FINE_AWAKE`, `DORMANT_CHECKS_PER_TICK`)
- Test: `crates/sim/src/matter/step_tests.rs`

**Interfaces:**
- Produces:
```rust
pub const MAX_FINE_AWAKE: usize = 2000;
pub const DORMANT_CHECKS_PER_TICK: usize = 256;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FineStepStats { pub awake: u32, pub frozen: u32, pub deferred: u32, pub blocks: u32, pub changed_blocks: u32, pub stalled_blocks: u32, pub promoted: u32, pub demoted: u32, pub truncated: bool }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatterStepStats { pub blocks: StepStats, pub fine: FineStepStats }
impl MatterStore { pub fn step(&mut self, rules: &SimRules, seed: u32, tick: u32, region: FineRegion, fine_budget: Budget) -> MatterStepStats }
```
- Order inside `step`: (1) block pass `step_skipping` with `skip(anchor) = halo.contains(anchor.div_euclid(8)) && any of the 8 blocks is fine`, halo = fine bricks plus 26 neighbours, built from `masks`; (2) `awake` sorted `(z, y, x)`, deduped, non-fine dropped; split into `frozen` (outside `region`), `first` (at most `MAX_FINE_AWAKE`), `deferred`; (3) even phase over `fine_anchors(first, PHASE_OFFSETS[0])`, odd phase over `first ∪ woken`, each loop checks `fine_budget.has_time()` before an anchor and sets `truncated` on a cut; (4) next `awake` = woken ∪ frozen ∪ deferred, plus all of `first ∪ woken` when truncated, plus `first` when `!is_heat_tick(tick)` (same rule as `step_with`); (5) blocks of `first` that fell asleep go to `dormant`; (6) up to `DORMANT_CHECKS_PER_TICK` dormant blocks: not fine → drop; `demote_uniform(block, tick, FineJobs::HYSTERESIS)` true → `demoted += 1`; cells all equal but hysteresis pending → push back; heterogeneous → drop.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn fine_water_spills_sideways_and_the_stone_floor_stays_uniform() {
    // water block on a stone floor, air around: after one tick the four side blocks are fine,
    // the floor and the block above are not, fine_units conserved, stats.fine.promoted == 4.
}
#[test]
fn the_block_pass_leaves_a_fine_block_alone() {
    // uniform water column (0, 1..5, 0) on stone; fine EMPTY block at (1, 1, 0) (promote after replace_block).
    // After one tick: m.blocks.get(split_voxel(IVec3::new(1,1,0))) == Cell::EMPTY (coarse backing untouched),
    // some fine cell of (1,1,0) holds water, (0,1,0) is fine, fine_units conserved.
}
#[test]
fn an_enclosed_fine_block_sleeps_then_demotes_after_hysteresis() {
    // full water block promoted, stone on all six sides. Tick 0: stats.fine.awake == 1, promoted == 0.
    // settle_matter(..) returns; m.fine_awake() == 0; keep stepping: before tick 60 is_fine, by tick 61 + 1 dormant pass
    // m.fine_blocks() == 0 and stats.fine.demoted == 1 on that tick.
}
#[test]
fn fine_blocks_outside_the_region_freeze_and_stay_awake() {
    // fine water at (40, 1, 0), region around ZERO radius 24: frozen == 1, blocks == 0 on two ticks;
    // radius 48: blocks > 0.
}
#[test]
fn at_most_2000_fine_blocks_step_per_tick() {
    // 2001 promoted stone blocks: tick 0 awake == 2001, deferred == 1; tick 1 awake == 1, deferred == 0.
}
#[test]
fn a_full_fine_pool_stalls_the_move_and_keeps_the_block_awake() {
    // MatterStore::new(64, 1): stalled_blocks > 0, promoted == 0, conserved, fine_awake() == 1 after the tick.
}
#[test]
fn a_zero_budget_cuts_the_fine_pass_and_keeps_everything_awake() {
    // Budget::wall(Duration::ZERO): truncated, blocks == 0, awake unchanged next tick; UNBOUNDED: not truncated.
}
#[test]
fn one_seed_gives_one_history() {
    // fine sand block on a stone ledge with a drop beside it; 40 ticks twice with seed 11: equal state_hash.
}
#[test]
fn a_spill_promotes_spreads_settles_and_demotes() {
    // stone tank, interior 4×2×4 air; set_fine all 64 cells of the block at (2, 2, 2) to water (promotes it);
    // settle_matter; conserved; step 70 more ticks; !is_fine(source); every fine block is at y == 1 and
    // heterogeneous; fine_blocks() <= 16.
}
```

- [ ] **Step 2: Run `cargo test -p engine_sim step_tests`** Expected: FAIL, `step` missing.
- [ ] **Step 3: Implement `matter/step.rs`** per the Interfaces order; `settle_matter(m, rules, seed, limit)` helper in tests loops until `active_bricks() == 0 && fine_awake() == 0`.
- [ ] **Step 4: Run `cargo test -p engine_sim`** Expected: PASS.
- [ ] **Step 5: `just clippy`** Expected: clean for `engine_sim`.
- [ ] **Step 6: Commit** `sim: coordinated block/fine tick with R_fine, awake cap, budget cut and demotion`.

### Task 5: Governor and bench

**Files:**
- Create: `crates/sim/src/matter/governor.rs`
- Modify: `crates/sim/src/matter.rs` (`mod governor;`), `crates/sim/src/lib.rs` (export `FineGovernor`, `FINE_RADII`, `RECOVER_TICKS`), `crates/sim/benches/bricks.rs`, `crates/sim/benches/baselines.json`
- Test: `crates/sim/src/matter/step_tests.rs`

**Interfaces:**
```rust
pub const FINE_RADII: [u32; 3] = [24, 16, 12];
pub const RECOVER_TICKS: u32 = 120;
#[derive(Clone, Copy, Debug, Default)]
pub struct FineGovernor { level: u8, quiet: u32 }
impl FineGovernor { pub fn new() -> Self; pub fn radius(&self) -> u32; pub fn region(&self, center: IVec3) -> FineRegion; pub fn observe(&mut self, fine: &FineStepStats) }
```
`observe`: `truncated || deferred > 0` → level up to at most 2, quiet reset; else quiet += 1 and at `RECOVER_TICKS` level down one, quiet reset.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_governor_narrows_on_a_cut_and_widens_after_quiet_ticks() {
    let mut g = FineGovernor::new();
    assert_eq!(g.radius(), 24);
    let cut = FineStepStats { truncated: true, ..Default::default() };
    g.observe(&cut); assert_eq!(g.radius(), 16);
    g.observe(&cut); g.observe(&cut); assert_eq!(g.radius(), 12);
    let quiet = FineStepStats::default();
    for _ in 0..RECOVER_TICKS { g.observe(&quiet); }
    assert_eq!(g.radius(), 16);
    g.observe(&FineStepStats { deferred: 1, ..Default::default() });
    assert_eq!(g.radius(), 12, "an awake cap overflow narrows too");
}
```

- [ ] **Step 2: Run it** Expected: FAIL.
- [ ] **Step 3: Implement `matter/governor.rs`.**
- [ ] **Step 4: Run `cargo test -p engine_sim`** Expected: PASS.
- [ ] **Step 5: Add bench `fine_step_2000_sub_bricks`** in `bricks.rs`: `MatterStore::new(2048, 4096)`, stone floor 64×1×64 at y = 0 by `replace_block`, 2000 blocks at y = 1 on a 50 × 40 grid each `replace_block(water(32))` with a hashed temperature spread then `promote`; per iteration `wake_fine` every block and `step(&t, 1, tick, FineRegion { center, radius: 64 }, Budget::UNBOUNDED)`.
- [ ] **Step 6: `just bench-save fine_step`** then `just bench-check`** Expected: new baseline written, gate passes; record the median in the commit message. The spec's 3 ms is for H1; note the desktop number.
- [ ] **Step 7: Commit** `sim: fine radius governor and a 2000 sub-brick fine pass bench`.

### Task 6: Documents

**Files:**
- Modify: `docs/adr/0023-fine-sub-bricks.md` (status section: what landed, the skip rule, the odd-phase boundary overlap, heat rate at fine scale deferred, halo lookup cost), `docs/plans/2026-10-25-variable-scale-and-underground.md` (move step 2 to Landed; next is step 3), `docs/PLAN.md` only if it lists sim phases (check first).

- [ ] **Step 1: Edit the ADR and the plan.**
- [ ] **Step 2: `python -I scripts/workflow.py status`** and run what it requires for own files.
- [ ] **Step 3: Commit** `docs: ADR 0023 and the variable-scale plan record the coordinated tick`.
