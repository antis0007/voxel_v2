# 0022: Part-based tools (foundation)

Date: 2026-10-25. Status: accepted; library only, no save or UI integration.

Source: docs/specs/MOD_ADDENDUM.md section A.2.

## Decision

- `crates/tools` (`engine_tools`) owns the tool model as pure data and functions:
  part kinds and tool schemas, tool materials, trait rules, assembly stat
  aggregation, XP levelling, repair and a byte codec. It depends on
  `engine_core` (hash), `engine_content` (`BlockId` names), `serde` and `ron`.
  No new external dependency.
- Content lives in three RON files embedded with `include_str!`, each kept
  canonical by a `*_ron_round_trips` test and rewritten by an ignored
  `dump_tool_*_ron` test (ADR 0002 pattern):
  - `assets/tools/parts.ron`: part kinds with a role (head, binding, handle,
    reinforcement, string) and a material cost; tool schemas listing required
    parts in assembly order with stat weights and a modifier slot count; the
    progression curve and repair rate.
  - `assets/tools/materials.ron`: one row per tool material with durability,
    mining level and speed, attack, handle modifier, conductivity,
    flammability, density, hardness and the magnetic, living and crystalline
    flags. `block` names the `BlockId` variant when the material exists as a
    block; load fails on an unknown name.
  - `assets/tools/traits.ron`: trait rules.
- Rules, not lists. A trait rule is a predicate over material properties
  (`Gt`, `Lt`, `Flag`, `Not`, `All`, `Any`) plus an effect and a stacking
  mode (`Once`, `PerPart`, `HeadOnly`). No rule names a material, so a new
  alloy row gets its traits from its numbers alone; a test adds an unnamed
  high-conductivity alloy and checks it comes out conductive. The effect
  vocabulary (`TraitEffect`) stays a Rust enum because the game systems that
  react to an effect branch on it, the same reason `BlockId` stays in Rust.
- Stats are weighted sums over parts in schema order: durability, mining
  speed and attack from per-part weights, mining level as the maximum over
  parts flagged `mining_level`, durability multiplied by the handle modifier
  of parts flagged `handle`, mass from part cost times density. Repair takes
  units of the first head-role part's material only.
- Levelling: total XP to hold level n is `n * base_xp + step_xp * n(n-1)/2`;
  each level adds one modifier slot to the schema's count, capped at
  `max_level`.
- Codec: `ToolAssembly::encode` and `decode` write a format byte, strings
  with u16 length prefixes and little-endian integers. Decoding does not
  consult the tables, so a tool from a removed schema loads and reports its
  error on use.

## Deferred

- Save integration waits for a future `SAVE_VERSION` bump; nothing in
  `world/storage.rs` changes now. Inventory items, crafting from cast parts,
  the smeltery and modifier effects are later slices.
- Modifier ids are plain strings until a modifier table exists.

## Rejected

- Traits as a per-material list in `materials.ron`: every new alloy would
  need hand-picked traits, and a changed threshold would need every row
  touched.
- A Rust enum for part kinds and schemas: adding a tool would need code; the
  code only branches on `PartRole`.
