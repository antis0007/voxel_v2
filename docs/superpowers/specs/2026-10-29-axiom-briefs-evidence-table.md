# Axiom briefs: evidence table and first Stage B loop

Status: audit plus one implemented slice (uncommitted, see end). Companion to
`2026-10-29-axiom-expansion-briefs-gap-analysis.md`, which covers conflicts,
save bumps and owner questions for the living-world, coop and spaceflight
briefs. This file adds the per-feature evidence the living-world brief §1 asks
for, and the fourth brief (`AXIOM_Reference_Worldgen_And_Bounded_Physics_Prompt.txt`).

Working tree read 2026-10-29: `SAVE_VERSION = 44`, `CURRENT_GENERATOR_VERSION = 79`.

## Evidence

| Feature | Owner | Works | Missing | Persists | Tests |
|---|---|---|---|---|---|
| Sheep, goat herds | `adventure/wildlife/husbandry.rs` | Feed joins herd, breed, grow, area cap, mount | Renewable product (now shearing, below) | `farming.vxf` herd section | 6 + 4 new |
| Night hostiles incl. satyr | `adventure/night.rs`, `mobs.rs` | Dark-cell spawns, refuge suppression | Satyrs as residents (gardener role exists) | No | night, mobs |
| Habitat residents (spider, sentinel, matron) | `world/habitats.rs` | Conditions, feed + mana, product every 20 s, crate swap | Unit tests | Save v26+ | Review harness only |
| Villagers, households, fernwake | `world/travel/`, `worldgen/src/households.rs` | Homes, trades, daily services, follow, fernwake routines | Names, routines beyond ids 3-6, workplace gate for households | `travel.vxt` v2 | Trades only |
| Valley hamlet | `worldgen/src/valley/settlement*` | Plots, homes, hall, workshop geometry | Any inhabitant | Generated | Worldgen |
| Crops, saplings, fishing | `world/farming*`, `travel/fishing.rs` | Light/water conditions, catch-up growth, deterministic catch | none | `farming.vxf` | 11 + 2 |
| Facility query | none | Block predicates in habitats, farming, `trades::workplace`, `night::roofed` | Shared cached query | none | none |
| Corridors, booking | `world/orbital.rs` | Seeded sites, 3-day service phases, one booking reschedules a corridor | Carriage is scenery, manifest is text | v24 | 4 |
| Receiver contract | `world/orbital/receiver.rs` | Paid order, due time, collect once into inventory, relay recovery | Arrival event, map marker, shipment id, container destination | v25, v30 | 0 (native fixture) |
| Talon ledger | `world/orbital/dynamics.rs` (untracked) | Cusp-timed lift/lower, momentum balance checked on decode | Cargo other than iron, completion notice, id | v39 | 0 |
| Radio | `world/orbital/radio.rs` | Levels, dish, link budget, uplink gate | Broadcasts, report log | v28 | 0 |
| Map | `egui_ui/map*` | Waypoints, objective, destination | Any orbital or delivery producer | Sidecar | 24 |
| Portals | `world/portals*` (untracked) | Library: link, trace, transit | Runtime instance, cargo | No | 0 |
| Containers + scanner | `world/containers.rs`, `world/scanner*` | Chests and storage machines, aggregated index, deposit, consolidate | Scanner block, change-driven index | Sidecar / save | 22 new |

## Fourth brief: reference worldgen and bounded physics

Conflicts with approved decisions, in addition to the gap analysis §3:

1. Client-local physics for every relevant body contradicts the current
   authority (only integer `engine_sim` is lockstepped; bodies stay
   host-authoritative until fixed-point bodies, decision 0022 §5). Owner decision.
2. Rapier, Salva and iroh appear as candidates; no physics or transport crate is
   approved (decision 0036).
3. Default erosion matches the dormant drainage-solve generator (ADR 0038,
   gate 73); enabling it is a generator bump, gates 80+.
4. The four site briefs add new names (Ternwater, Rainbreak, Ambercut,
   Hearthglass); none are approved.
5. Its bounded edit pipeline (admission, coalesced dirty records, stale-mesh
   rejection) overlaps work already tracked under the sim budget spiral and
   streaming perf items; no new ADR yet.

## Stage B loop 1: shearing (implemented)

A grown sheep sheared with held Shears yields 1-3 Lumen fibre (the game's
string/wool role: bed, bookcase, fence, sign, fishing rod). Fleece regrows over
one in-game day. Shearing a wild sheep adds it to the herd, so the timer
persists. The yield rolls from herd id and tick, so reloads cannot reroll it.

- Data: `shear` row on Sheep in `assets/bestiary.ron`; `Shears` recipe (two iron
  ingots, diagonal) in `assets/recipes.ron`.
- Code: `husbandry::shear`, `Member::fleece_at`, dispatch in
  `farming::interact`; `Item::Tool(11)` name, category, icon, creative list,
  decode range `1..=11`.
- Save: no main-save change. The herd section in `farming.vxf` gains a trailing
  fleece list; herds written before it decode with full fleece (tested).
- Not done: shorn sheep look the same (needs a mesh state and A/B captures);
  shears take no wear; no villager wool trade; fenced pasture is not yet a
  night-spawn refuge.

Verification: built in a scratch copy of the tree with three peer compile
breaks stubbed (`ShadowRenderer::new` arity in two GPU tests,
`WorldType::Planet` arm in a test seed table). Four new husbandry tests pass;
the 9 failures in the touched areas fail identically before the change.
Uncommitted: HEAD lacks the peer item work these edits extend (pickaxe ids,
fishing rod arms), so the slice lands with the whole-tree landing.

## Next slices, smallest first

1. Receiver arrival: saved announced flag, report log entry, map waypoint kind
   for the receiving pad, unit test for order then collect once.
2. Household workplace gate from the real home, reusing farming predicates.
3. Resident service feeding a blocked habitat source within 12 m.
