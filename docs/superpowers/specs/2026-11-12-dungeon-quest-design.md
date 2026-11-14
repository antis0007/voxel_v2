# Dungeon quest layer: locks, traps, secrets, verticality

Status: proposed 2026-11-12. Owner review pending.

## Problem

Dungeon v2 (`crates/worldgen/src/dungeon/v2`) builds a spanning tree of rooms
over up to three 6 m levels, seals the Core behind one puzzle room, and drops
caches in dead ends. It has no progression: every door is open, nothing is
locked, nothing bites back, nothing is hidden except `MirageBrick` false walls
in labyrinths. Rooms read as shoeboxes; corridors repeat a `rem_euclid` bay to
the vanishing point; loot is a hard-coded Rust table.

Reviewed captures (owner, 2026-11-12) show: terraced dome caverns, brick
pasted on raw rock with no transition, flat ceilings, floating torches,
periodic sawtooth ceilings, tile-scale bricks, no wear, no life.

## Goal

A dungeon a player maps in their head: a locked door seen early and opened
late, a key glimpsed from a balcony, a shortcut kicked open from the far side,
a floor that drops you a level, a chest that bites, a wall that is not there.
Every rule visible before it hurts (hint first, trap second).

## Architecture

Three layers, each a pure function of the seed and the plan, plus one sidecar.

1. **Plan** (worldgen, gated). Extends `Complex` with a `Quest`:
   - *Mission graph* over the existing room tree. Critical path entry → Core by
     BFS hops. Place `k = 1..3` locks on critical-path corridors; for lock `i`
     put its key in a room reachable before lock `i` (a side branch, a
     different level, or behind a secret). Big lock on the Core seal; big key
     in the deepest side branch. Keys are dungeon-bound (Zelda small keys).
   - *Cycles* (Unexplored-style): reuse the gen-88 loop shortcuts as one-way
     doors that open only from the far side, and as pit drops to a lower level
     (one-way down, stairs back).
   - *Secrets*: `MirageBrick` walls on loop corridors and cache niches, each
     with a tell (crack line, draft particles, a bloodstain trail, mismatched
     course).
   - *Traps* per room from the room kind and depth tier (table in RON):
     pressure-plate dart volley, retracting floor spikes, pit trap (floor
     gives into the level below or a water pit), chest trap (gas, darts or a
     mimic), proximity rune (fire jet), ceiling drop (rubble).
   - *Posts*: per-room enemy posts (skeleton ossuaries, roper gardens, spider
     cells), replacing the two global anchors.
   - *Hints*: lectern and wall-sign lines from a RON hint table, chosen by the
     next obstacle on the critical path ("the third flagstone is worn").

2. **Masonry pass** (worldgen, same gate). Breaks the periodicity:
   domain-warped room outlines, irregular course heights and bond shifts,
   chipped and missing stones, floor wear paths along the critical route,
   rubble at wall feet, damp gradients near water, roots and moss from the
   ruin pass driven by context (wall foot, ceiling drip, doorway draft)
   instead of per-cell hashes. Brick module 0.5 m at fine scales, not 1 m.

3. **Runtime** (app). `world/dungeon_quest.rs` (new) reads the plan near the
   player and runs triggers on the fixed step: plate pressed → volley; chest
   opened → trap; key picked → counter; locked door used with key → plug
   cleared through ordinary block edits. Trap effects reuse
   `tactics::Hazard` and adventure projectiles.

4. **State**: a `dungeon` entry kind in the `block_data` sidecar keyed by the
   complex anchor: keys held, locks opened, traps sprung, plates cooldowns,
   secrets found. No save-version bump, no new `BlockId`s: plates, slits,
   spikes and locks are drawn with existing blocks plus the plan (a plate is
   a `StoneBricks` flagstone the plan marks; its tell is wear and a seam).

## Enemies

- Skeleton (kind 19) placed by posts in ossuaries and crypts; some rise from
  `Remains`-style bone piles when the player passes (ambush tell: bones).
- Roper (new kind): rooted stalagmite body, long sticky tongue that pulls the
  player in; disguised as a stalagmite until it moves (tell: a damp sheen).
- Burrowers (landed 2026-11-12, kinds 80-86) for cave stretches between
  dungeon rooms.

## Phases

1. Quest plan + sidecar + small keys/locked doors + hints (one dungeon
   playable end to end).
2. Traps (plates/darts, spikes, pit drop, chest traps) with tells.
3. Secrets and one-way shortcuts; pit drops between levels.
4. Masonry pass (A/B captures required before default).
5. Posts, skeleton risers, roper.
6. Cave reshaping between dungeons (warped chamber roofs, speleothems,
   strata, no concentric terraces) under its own gate.

Each phase: dormant generator gate claimed in
`docs/worldgen/GENERATOR_GATES.md`, A/B captures from fixed review poses,
historical gates untouched.
