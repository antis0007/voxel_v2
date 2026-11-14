# Villager civilization slice: plan

2026-11-06. Plan, not a claim: nothing here has landed. It executes the settlement and
civilization direction already approved and does not reopen intake
([decision 0022](../decisions/0022-master-plan-review.md) §1). Every source fact below is a
read-only audit of the working tree on this date; line numbers move as peers edit, file
names are the stable reference.

Inputs it follows (not inputs of record):

- [Civilization on solved geography](../superpowers/specs/2026-11-05-civilization-on-solved-geography-design.md) (the seven anchors, one city across simulation levels, bounded history).
- [Axiom: from solved geography to persistent consequences](2026-11-05-axiom-causal-world.md) (the province and its six ordered deliverables).
- Decisions [0022](../decisions/0022-master-plan-review.md), [0023](../decisions/0023-worldgen-spec-input.md), [0034](../decisions/0034-player-freedom-addendum.md) (demoted to reference), [0039](../decisions/0039-axiom-expansion-briefs.md).

Reuse words: `L` = legacy/partial substrate exists, `P` = design only.

## 1. Goal

A region where people live, work, trade, grow, and can be governed, manipulated, conquered
and killed: not decorative villages with doors that never open. The player can spend the
whole game travelling and building, or take a holding and grow it into a settlement that
neighbouring powers deal with.

This reframes the original brief ("villagers build cities of their own accord"). The engine
direction is one authoritative record per place, shared by first-person, the overhead camera
and the planetary map, with compact plans and voxels generated on demand. Two consequences
fix the scope:

- Cities come from settlement records and parametric plans. They are not discovered by
  simulating every villager placing every block. This matches the design anti-goal
  "planning a city by generating its voxels".
- Autonomy means delegated work and emergent outcomes (growth, migration, rerouting, revolt)
  driven by stocks and jobs, not free-form block-by-block construction.

## 2. Current implementation (audited 2026-11-06)

Generation (worldgen; pure functions of `WorldSampleConfig`):

| Piece | Where | State |
|---|---|---|
| village lanes, household parcels | `crates/worldgen/src/households.rs`, `crates/worldgen/src/households/settlement.rs` | L: four parcels per hub, roles 0-3, version gates 79 and 92; lane, door approach and route connectivity are surveyed |
| resident identities and homes | `crates/worldgen/src/households.rs` (`role`, `home`, `nearby`, `refuge`) | L: stable ids from `BASE`, home position only |
| place fixtures | `crates/worldgen/src/fernwake.rs`, `crates/worldgen/src/sites.rs`, `crates/worldgen/src/architecture.rs`, `crates/worldgen/src/ruins.rs`, `crates/worldgen/src/zones.rs`, `crates/worldgen/src/valley.rs`, `crates/worldgen/src/lore.rs` | L: authored or generated sites and fixtures |

Runtime (`app`):

| Piece | Where | State |
|---|---|---|
| villager trades, stock, absences | `crates/app/src/world/travel.rs`, `crates/app/src/world/travel/trades.rs` | L: per-villager trade pages, one daily reset, homes, up to `MAX_GUARDS` guards |
| authored people by persistent id | `crates/app/src/world/travel/residents.rs` | L: conversation reads world state; rows in `assets/fernwake_residents.ron` |
| guards, wildlife, encounters | `crates/app/src/world/adventure/settlement_guard.rs`, `crates/app/src/world/adventure/wildlife.rs`, `crates/app/src/world/adventure/encounters.rs` | L |
| plots, freight, jobs | `crates/app/src/world/farming/` | L: one district crossing (per the province plan) |

Absent on the inspected paths (searched, not found): a runtime settlement, district,
building, resident or caravan record; a regional economy or population model; migration;
builders. The province plan and the civilization design record the same gap.

## 3. The gap and the one design decision to take now

Everything above is either generation-time (static) or per-villager runtime state with no
shared settlement record. The goal needs one record per place plus a rule for which
simulation level owns each quantity ("one owner per quantity").

Owner decision required: the autonomy model.

- (a) **Generated-plus-simulated (recommended).** Settlements are records with compact
  plans; voxels appear on demand; people act through jobs and stocks; growth, migration and
  conflict are flows on the record. Emergent feel at urban scale, bounded cost, fits "one
  city across simulation levels".
- (b) **Emergent builders.** Villagers choose each building, then place blocks. Highest
  agency, in conflict with the anti-goals (compact plans, no per-voxel city planning) and
  with the bounded-progress rule.

The slices below assume (a) with a small bridge to (b): villagers choose *projects* from a
settlement ledger, and a parametric plan builds them.

## 4. Settlement record model

Propose a new runtime module, `crates/app/src/world/settlement/`, holding plain structs and
stable indices (ADR 0010, "no ECS"). The `sim` crate stays a separate cellular world and owns
none of this.

- `Settlement { id, pos, tier, owner_faction, treasury, districts[], population_by_cohort, stock_summary, ledger, history_ref }`
- `District { id, settlement, kind, parcel_ids[], buildings[] }`
- `Building { id, district, plan_id, owner, condition, occupants, stock, jobs }`
- `Resident { id, settlement, home, work, needs, cohort, named }`
- `Caravan { id, route, cargo, party }`

Identity maps to what already exists: household ids are stable from `BASE` in
`crates/worldgen/src/households.rs`; a settlement keys off `settlement::hub`. Two simulation
levels share the one record (civilization design): the coarse level (overhead, map) owns
population, ownership, stock, routes and growth; the fine level (first person) owns voxels,
condition and occupancy detail. Detail is materialised when a region loads, from the plan
plus committed deltas, so an unloaded settlement still answers map queries.

Determinism and saves: fixed logical intervals, positional hashing from the world seed, no
`rand`, no wall clock selecting an outcome. New persistent state gets a versioned sidecar
(the `travel/sidecar.rs` precedent) or a main save bump chosen against then-current source.
Read `crates/app/src/world/storage.rs` and `crates/voxel/src/config.rs` before any migration.

## 5. Sequenced slices

Each slice is one writer, small, and verified by a headless scenario plus a test.

| Slice | Deliverable | Acceptance |
|---|---|---|
| 0 record skeleton | the structs above and a registry on `World`; load/save wiring; an id map to worldgen households and hubs; no behaviour | a save with two settlements round-trips; a scenario asserts record ids equal generator ids |
| 1 bind one village | bind Fernwake or a households hub to a `Settlement`; districts from existing parcel geometry; read-only map pins in both views; no new generation | walk the village and list districts, buildings and residents from the record; ids match the generator; before/after capture of the pins |
| 2 residents as data | `Resident` rows from existing ids; population as cohorts keyed (settlement, race, role, age band); named figures keep their `fernwake_residents` rows; migration as a bounded flow | move N people on an explicit trigger; net change conserves (no vanish, no double count); named figures keep their conversation |
| 3 connect one livelihood | one settlement's harvest to workshop stock to market inside the record; empty stock limits output; a daily offer never creates inventory (province deliverable 3) | control, blocked input and restored supply on one start state; inspectable stop reason; save and reload mid-shortfall |
| 4 growth and decline | occupied homes and workplaces track stocks and jobs; growth and decline write physical traces (a new parcel, an abandoned district) through parametric plans, voxels on demand | run to growth and to decline; occupied counts and traces match the ledger |
| 5 player governance | the player sets priorities and targets on a settlement; residents respond through the same jobs and stocks | two priority sets diverge on one start state; capture |
| 6 conflict and occupation | a raid or siege damages buildings, captures stock, changes owner; civilians flee or submit; recovery follows | per the province "Conflict" check; save and reload carries ownership, damage and losses |
| 7 bridge to autonomy | the ledger opens a build request from a deficit; residents pick it; a parametric plan (shape grammar plus validator) builds one building family | with a housing deficit, villagers build a house unaided on the same start state; before/after capture |

Slice 0 through 3 are the near-term path and align with the province plan's first four
deliverables. Slice 7 is the bridge to the original brief and should stay parametric.

## 6. Rules, gates and risks

- One owner per quantity. A summary is derived or owns a distinct declared stock.
- Determinism: fixed intervals, positional hashing, logical work limits. Wall time may yield
  to the next pass; it never selects a committed outcome.
- Content rows live in `assets/*.ron`; no content tables in Rust; no new dependency without
  an ADR line.
- Saves: version new records and define compatibility. Read the source constants before
  choosing; never edit a historical version gate.
- One writer per slice. This tree is shared; confirm no overlapping session before editing.
- Gates: `just check`, `just clippy`, `just test <name>`, a headless scenario under
  `assets/scenarios/`, then `just verify` before commit; captures for any visual change.
- Top risk: slice 7 drifting into per-block construction. Keep plans parametric and geometry
  on demand, or it breaks the anti-goals and the budget rules.

## 7. Open questions for the owner

1. Autonomy model (a) or (b) in section 3.
2. First bound fixture: Fernwake, or a households hub.
3. Record home: `crates/app/src/world/settlement/`, or a new `crates/civ` crate.
4. Save shape: a sidecar (the `travel/sidecar.rs` precedent) or a main save bump.
5. Does this pass include combat (slice 6), or is that left to the province plan.

