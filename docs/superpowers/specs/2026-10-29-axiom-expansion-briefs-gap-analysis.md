# Axiom expansion briefs: gap analysis

Status: analysis only, no code changed. Owner decisions listed in section 5.

Inputs (untracked owner staging, 2026-10-29):
- `docs/specs/AXIOM_Science_Spaceflight_And_Planetary_Expansion.txt`
- `docs/specs/AXIOM_Planetary_Simulation_And_Coop_Architecture.txt`
- `docs/specs/AXIOM_Living_World_Expansion_Prompt.txt`

The briefs state they were written without repo access; names and tuning are proposals.

## 1. Stale facts in CLAUDE.md

- Save: `VXWSAV44`, `SAVE_VERSION = 44` (`crates/app/src/world/storage.rs:14`, `:56`). CLAUDE.md says 34.
- Generator: `CURRENT_GENERATOR_VERSION = 79` (`crates/voxel/src/config.rs:5`). CLAUDE.md says 53.
- `docs/TRACKING.md:49` says save 39.
- Decisions 0022 and 0036 live in `docs/decisions/`; `docs/adr/0022` and `docs/adr/0036` are unrelated ADRs.

## 2. Already present

| Brief area | Code |
|---|---|
| Orbital rails, home/moon/giant | `crates/space` (`lib.rs`, `kepler.rs`, `system.rs`), `assets/star_systems.ron` |
| Skyhooks, Talon freight, momentum ledger | `crates/app/src/world/orbital/{skyhook,dynamics,receiver,radio,passes}.rs`, `docs/TRACKING.md:43-56` |
| Power capacity vs delivery | `world/factory/induction.rs:16-19` |
| Chip design and fab | `crates/chip` |
| Cube-sphere cells and seams | `crates/core/src/cube_sphere/`, `crates/app/src/planet.rs` |
| Lockstep foundation (not live) | `crates/netcode/src/lockstep.rs`, `docs/design/MULTIPLAYER_LOCKSTEP_SIM.md` |
| Delivery lanes, authority tiers | `docs/superpowers/specs/2026-10-26-multiplayer-design.md` §3, §7 |
| Revision-checked containers, edit acks | `crates/netcode/src/{container,edits}.rs` |
| Households, trades, satyr, dryad roles | `crates/worldgen/src/households.rs`, `world/travel/trades.rs`, `adventure/folklore.rs` |
| Herds (feed, breed, grow) | `adventure/wildlife/husbandry.rs` |

Absent: vessels and reference frames, operation ids with receipts, coarse settlement/caravan/faction records, generation manifest, renewable animal products, shipment custody beyond Talon crates.

## 3. Conflicts

1. Scope. Decision 0022 §1 froze the inputs of record and parked Axiom revision 4 for M1-M3; M1 is the `dam_break` slice. Decision 0036 §2 adopted only the audit's reordered plan. All three briefs reopen full Axiom scope and the living-world brief rejects audit-only results.
2. Authority. The briefs want every client to run local physics with command replication. The approved plan locksteps only integer `engine_sim` matter; rigid bodies, players and projectiles stay host/owner authoritative (ADR 0011) until fixed-point bodies land (0022 §5, B03).
3. Transport. The coop brief suggests iroh, Renet or Quinn. Owner decision: own keys and UDP relay first, then Steam.
4. Floats. Ship rigid bodies, frame transforms and patched conics on f64 rails clash with fixed-point sim state. `world/orbital/passes.rs:47` already uses `Instant` in simulation code.
5. Lore. Roughly 70 new proper nouns and peoples across the briefs. Several collide with existing unrelated uses: Cinder, cairn, Vesper, aureole, Pale Reach.
6. Text rules. "Ancient" appears often (banned in player text). Example broadcasts and NPC dialogue are prose, against UI_RULES 12 and 14.
7. IP. Living-world encounters follow Fablehaven and Percy Jackson closely.
8. Art. A Blender rig pipeline has no ADR; current direction is authored voxel and `.vox`.
9. Decision 0034 (player freedom) is still proposed; the living-world brief assumes it.

## 4. Save bumps, gates, ADRs

Save bumps: vessel assemblies (topology, mass, fuel, modules), frame ids, ore lots with provenance, research journal, shipments with custody, operation receipts, resident and companion records, settlement/caravan/faction records, generation manifest, herd product timers (unless the farming sidecar carries them), new item ids.

Generator gates (80+, check the cross-branch registry first): landable moons, settlement and city generation, site grammars, fauna habitat placement, megastructure realisation.

ADRs or owner decisions: authority model; float vs fixed vessel bodies (amends 0022 §5); fictional cruise drive; patched-conic boundaries; coarse/local handoff; durable-commit semantics; shipment state machine; cohort simulation vs ADR 0018; any physics, transport or asset-pipeline dependency; adopting the briefs (amends 0022 and 0036).

## 5. Owner decisions needed

1. Adopt the briefs as inputs of record, or keep them parked behind M1.
2. Authority model: keep the lockstep plan, or move to client-local physics after fixed-point bodies.
3. Approve, rename or reject the new names (per-brief lists in section 3 sources).
4. Whether the coop brief's transport crates are on the table.

## 6. Candidate first slices

Each fits current decisions and needs no new names.

| Slice | Brief | Files | Save |
|---|---|---|---|
| Talon catch, ground side: radio reports a pending consignment from the ledger, player repairs the receiver, the same consignment lands carrying a controller usable as a factory part | Spaceflight §8.8/9.1 | `world/orbital/{receiver,radio,dynamics}.rs`, `world/factory/chips.rs`, `assets/{recipes,machines,descriptions}.ron`, one scenario | none if the id fits `receiver.rs` encode, else 45 |
| Idempotent operations: `OperationId` with bounded dedup table, applied to the last-shared-item transfer, two-bot duplicate test | Coop Milestone B, §7.6 | new `crates/netcode/src/operation.rs`, `multiplayer/{inventory,host_commands}.rs` | none unless receipts persist |
| Shearing: fleece regrowth on herd sheep, wool item and recipe, one villager trade, night spawns skip fenced pasture | Living world Stage B loop 1 | `adventure/wildlife/husbandry.rs`, `adventure/night.rs`, `assets/{bestiary,recipes,fernwake_trades}.ron`, one scenario | none if sidecar carries it |

Prerequisites: commit the untracked `world/orbital/dynamics.rs` that tracked code depends on; remove the `Instant` use in `passes.rs`.

## 7. Risks

1. Determinism. Client-local float physics would silently diverge; lockstep is not live.
2. Identity handoff. Caravans, recruits and stock must exist once across coarse record, live entity and container, through save and reconnect. Nothing models this yet.
3. Save and gate churn. Every brief area adds save sections or gates while gates 54-79 already collide across branches.
4. Perf debt. Orbital scene rebuild ~10 ms CPU per frame, sim budget spiral, horizon builds 21-41 s.
