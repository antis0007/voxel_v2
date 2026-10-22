---
name: adventure
description: Spells, wands, enemies and bosses in crates/app/src/world/adventure — the spell table, wand deck evaluation, enemy kinds, how to test a cast headlessly.
---

# Adventure (magic and combat)

Where: `crates/app/src/world/adventure.rs` (`Adventure` state, `update` and `tick`), `adventure/wands.rs` (Noita-style deck evaluator, `cast_wand`), `adventure/spells.rs` (effects, particles), `adventure/spell_defs.rs` (loader and accessors for the spell table), `adventure/bestiary.rs` (species rows, rigs), bosses in `hart.rs`, `tollkeeper.rs`, `matron.rs`, `bosses.rs`, discovery in `discovery.rs`, statuses in `status.rs`.

Data: `assets/spells.ron` (one row per spell id; ids 0–95 fit the save; `class` and `unlock` are enum names) and `assets/bestiary.ron` (`(enemy kind, Species)` rows; kinds without a row keep older hard-coded presentation). Accessors such as `spell_cost(id)` read the table; adding a spell is a new row plus any new behaviour in `spells.rs`/`wands.rs`. The found-spell log is save version 19+.

Casting: `Input::cast_spell` → `adventure::tick(world, player, dt, cast, cycle, melee)` → `wands::cast_wand` evaluates the held wand's deck (modifiers change the next projectile, triggers deliver effects) → projectiles live in `Adventure::projectiles` and resolve in `update`. `raycast_block` works in voxel units; use the aim-cell helpers at fine scales.

Test headlessly: a scenario `Hold(ticks: N, cast: true)` casts the held wand; assert blocks or `Finite`. `just test spell`, `just test wand`, `just test adventure` cover the evaluator and table invariants (sorted unique ids, every row complete, classes agree with the wand predicates).

Pitfalls: `Adventure::update` is ~1,500 lines (debt); `growth.rs` is in-progress work, leave its dead code alone; review harnesses for bosses (`VOXEL_HART_*`, `VOXEL_TOLLKEEPER_*` env vars) still live here until scenarios replace them.
