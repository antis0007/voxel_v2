# 0023: Fine sub-bricks at three resolutions

Date: 2026-10-25. Status: accepted; storage and coordinated ownership in `engine_sim`, no app wiring yet.

Source: owner brief "Variable scale + underground" (2026-10-25), Parts 1 and 3.

## Context

The game is a 1 m block world: placement, mining, machines, collision and most
rendering all assume one cell per metre, and both cellular implementations
(`engine_sim`, and the legacy `World` water/sand pass) step 1 m cells. Noita-style
matter at 0.25 m inside the same world needs the fine cells to be *rare*. Simulating
every block at 0.25 m multiplies cells by 64 and the brick pool by the same factor,
which the H1 memory and tick budgets cannot pay.

ADR 0007 already reserves `Scale` in `BrickCoord` and says a brick is the unit the
GPU allocates. This ADR fixes how a finer scale attaches to that unit.

## Decision

Three resolutions coexist; a block is uniform until something makes it uneven.

| Level | Cell | Owns |
|---|---|---|
| Block | 1 m | Build grid, mining, machines, structures, navigation, most rendering |
| Fine | 0.25 m (4³ per block) | Sand, water, gas, fire, acid, debris, cracks, partial blocks |
| Coarse | 2-16 m | Far terrain, rivers, smoke plumes, biome exports |

- `engine_sim::fine::FineStore` owns fine sub-bricks: `[Cell; 64]`, 512 B each,
  in a separate pool indexed by the block coordinate. `FineStore` is Phase 0
  because the eight-byte `Cell` has no spare slot index; moving that index into
  a future block-record header changes no rule below.
- Promotion splits a uniform `Cell` into 64 copies of itself. One block volume
  unit is 1/64 m³ and one fine unit is 1/4096 m³, so copying the same `volume`
  into every fine cell conserves mass exactly. Promotion of an existing fine
  sub-brick never overwrites authored partial cells.
- Demotion has two modes. `demote_uniform` returns the single `Cell` when all 64
  cells are equal and frees the slot. `demote_averaged` is the budget-forced
  path: it picks the material with the largest total volume, means the volume,
  volume-weights temperature and returns `Collapse { cell, remainder }`. The
  remainder is fine units that do not fit and must be carried into a neighbour;
  it is never discarded.
- Hysteresis: a sub-brick stays fine for a configured number of ticks after its
  last write, so a settling surface cannot promote and demote every tick.
- Every CA pass takes a `Budget` (ADR 0003). Fine work is capped separately
  from block work; the governor lowers the fine radius before any block radius.
- Machines, conduits and multiblocks stay block-scale records. They reach fine
  matter only through planes (heat, charge, gas), so the machine layer never
  branches on fine cells existing.

## Implementation status

- Landed: `crates/sim/src/fine.rs` (pool, promote, `set`, both demotions,
  `collapsed`) and `crates/sim/src/fine_tests.rs`.
- Landed 2026-10-26: the coordinated tick below (`matter/step.rs`,
  `matter/fine_kernel.rs`, `matter/governor.rs`, `kernel::step_skipping`).
- Deferred: wiring into `World` and the headless CPU region, fine masks on
  `ChunkData`, template meshes for slabs and stairs, mob capsule sweeps against
  fine cells, save format (`SAVE_VERSION` bump) for the pool, a fine-scale heat
  rate, and the H1 budget test.
- The legacy `World` path does not read `engine_sim` yet (`docs/PLAN.md` Phase
  A3+), so nothing in play changes with this ADR.

## Storage ownership refinement (2026-10-25)

`MatterStore` now owns private `BrickStore` and `FineStore` instances plus sparse
512-bit headers keyed by world brick coordinate. Its block reads return either
a uniform cell or a borrowed fine sub-brick; fine reads repeat a uniform value
without allocation. No mutable pool accessor or block-only kernel adapter is
exposed. Coordinates in this API are 1 m blocks, not legacy scale-preset cells.

Promotion sets the mask only after pool allocation succeeds. Fine writes promote
on change; identical writes neither allocate nor reset hysteresis. Explicit
whole-block replacement commits the coarse write before releasing the fine slot
and mask. Uniform demotion uses that same transaction after checking hysteresis
and equality of every cell field. A failed coarse allocation keeps the fine state.

This supersedes budget-forced averaged collapse in the decision above for the
coordinated owner: heterogeneous cells remain fine. The standalone legacy helper
is retained for existing callers, but loses material identity and thermal energy;
it is not available through `MatterStore`. Conservative heterogeneous collapse
needs material and energy ledgers before it can be enabled.

Memory reporting separates live/reserved payload from estimated index and mask
storage. Index estimates exclude hash-table control bytes and allocator overhead;
they are not a total heap measurement. Masks are CPU sidecar headers, not a GPU
layout or save-format change. Scheduling, fine CA, gameplay and persistence remain
deferred. Library compilation checked; tests deliberately not run or added.

### Budgeted representation jobs

`FineJobs` adds a capacity-bounded FIFO with one latest intent per block.
Callers submit background promotion or demotion requests; a full queue returns
false so its caller can retry. Successful promotions retain their queue entry as
uniform-demotion maintenance. Direct fine edits must also request maintenance.
The queue is session-local and must stay with the same `MatterStore`.

Each tick permits one pass, no more than 2000 attempted blocks, and no more than
one visit per initial queue entry. Deferred and pool-blocked jobs rotate to the
back and consume attempts too. A `Budget` clips the 0.5 ms wall slice to its
parent; unbounded headless budgets retain deterministic count-limited work.
Individual jobs cannot be interrupted, so this is a cooperative deadline rather
than measured H1 performance evidence.

Promotions use a player/hazard focus with a default 24 m radius (callers can lower
it to 16 or 12). Demotions can reclaim uniform matter anywhere after 60 quiet
ticks; radius reduction never collapses mixed cells. Player edits use the direct
storage API, not this queue. Fine CA, automatic load-governor integration and
gameplay adoption are still deferred. Library compilation checked; no tests run.

## Coordinated tick (2026-10-26)

`MatterStore::step(rules, seed, tick, region, fine_budget)` runs the block
kernel first, then the fine pass; both apply `block_rule`, so one rule table
governs both scales.

- The block kernel skips every 2×2×2 block whose eight cells include a fine
  block (`kernel::step_skipping`; `step` is the same call with a constant
  false, so GPU parity and the scenario hashes are untouched). The predicate
  costs one set lookup per anchor (bricks holding fine blocks plus their 26
  neighbours) and eight mask bits only near those bricks.
- The fine pass visits awake fine blocks with the same two Margolus phases in
  fine coordinates: the even phase tiles a 4³ block with eight blocks, the odd
  phase's 27 start one cell early and reach one cell into each neighbour. A
  uniform neighbour reads as 64 identical cells and is promoted only when one
  of its cells changes; pool slots for every such neighbour are counted before
  any write, so a full fine pool stalls the whole block and keeps it awake.
  Fine dice are salted (`FINE_SEED_SALT`) so a fine block and the 1 m block at
  the same anchor roll differently.
- Boundary: a uniform block beside fine matter is stepped by both passes in one
  tick, by the block kernel in the phase where its block holds no fine cell and
  by the fine pass through the straddling odd-phase blocks. Each transfer
  conserves volume and the fine pass reads the block kernel's result, so nothing
  is counted twice; the block sees two interactions per tick instead of one. A
  fine block outside `R_fine` freezes, and so does the block-kernel block that
  contains it.
- Sleep follows the brick rule: a fine block stays awake while any block it lies
  in changed or is pending, and only a heat tick may put it to sleep. Sleepers
  join a dormant queue; up to 256 per tick are tested, uniform ones demote after
  the 60-tick hysteresis (`FineJobs::HYSTERESIS`), heterogeneous ones leave the
  queue until they next sleep. There is no averaged collapse.
- Limits: at most `MAX_FINE_AWAKE` (2000) fine blocks step per tick and the
  rest wait awake; the wall-clock `Budget` is checked every 32 anchors and a
  cut keeps everything the pass was stepping awake. `Budget::UNBOUNDED` never
  cuts. `FineGovernor` narrows `R_fine` 24 → 16 → 12 m on a cut or an
  overflowed cap and widens one level after 120 quiet ticks.
- Cost: `fine_step_2000_sub_bricks_idle` measures 7.0 ms on the desktop (2000
  settled fine water blocks, 35 blocks each, nothing moving), about 3.5 µs per
  sub-brick: the kernel's own per-block cost times 35. The brief's 3 ms on H1
  therefore means a few hundred awake sub-bricks on H1, which the governor
  holds by radius; the H1 budget test waits on an H1 measurement.
- Heat at fine scale runs the block rule unchanged: the same energy per edge
  across four times as many edges per metre, so heat diffuses sixteen times
  faster through fine matter than through uniform blocks. A fine-scale
  `HEAT_RATE` divisor waits for thermal ledgers.
- `MatterStore::state_hash` folds every fine block's cells into the brick hash
  so a replay can be pinned.

## Rejected alternatives

- Always simulate at 0.25 m: 64× cells and 64× pool for matter that is uniform
  almost everywhere.
- A per-cell variable scale inside one pool: every kernel and mesher would
  branch on scale at every cell.
- Storing the fine slot in `Cell` now: the record is full and the save format
  would change before the owning system exists.
- Destroying the rounding remainder on demotion: it is real water or real
  material and would leak the volume ledger.

## Tests

`crates/sim/src/fine_tests.rs`: uniform promotion round trip, full-block round
trip with zero remainder, heterogeneous refusal, hysteresis, averaged collapse
carrying every unit, pool overflow and slot reuse, out-of-range writes, authored
partial block preservation.
