# Spells, wand tiers and spell discovery

This documents how spells are wired, the twelve spells added with the discovery
update, how a player finds spells, and the save-format change that persists it.

## How a spell is wired

A spell is a plain `u32` id (`0..=MAX_SPELL_ID`, currently 95). Everything that
describes it lives in **one row** of the table in
`src/world/adventure/spell_defs.rs`: name, flavor, hover hint, badge, colour,
mana cost, cooldown, cast-delay / recharge / spread deltas, base damage and
speed, sound family and pitch, its `Class` (projectile, modifier, effect,
other) and its `Unlock` rule. The old per-field `match` functions
(`spell_name`, `spell_cost`, ...) are now lookups into that table; the
migration was proved value-for-value over ids 0..=100.

To add a spell:

1. Add a row to `SPELLS`, keeping it sorted by id.
2. If it needs new behaviour, add it where the behaviour lives:
   - bolts and utility casts: the cast `match` in `wands.rs` `cast_wand`;
   - stat-only modifiers: `paint_shot` in `wands.rs`; control-flow cards in
     `evaluate_programmed`; new `ShotMods` fields if needed;
   - flight and impact effects: the projectile loop in `adventure.rs` `update`.
3. Put it on a palette tab in `src/egui_ui/wands.rs` (`BOLTS`, `EFFECTS`,
   `SHOT_MODIFIERS`, `PROGRAM_MODIFIERS`). A spell on no tab can never be
   placed; a test fails if you forget.
4. Add icon art in `tools/gen_spell_icons.py` and run it to regenerate
   `src/egui_ui/icons_spells.rs`.

Tests check that rows are sorted and unique, complete, inside the id space,
that classes agree with the wand predicates, that every spell is reachable from
the palette and from the top wand, and that unknown ids fall back to documented
defaults.

Unknown ids read "Unknown spell". IDs in use: `0..=27`, `28..=32`, `34..=63`,
`64..=70`. Free: `33`, `71..=95`.

### Aiming helper

`crate::world::raycast_block` takes **voxel** coordinates. Spell code holds the
eye in **metres**. Always use `wands::aim_cell(world, eye_m, dir, max_m)`, which
converts. Raw calls aim from the wrong place at 0.5 m and 0.25 m voxel scales.

## Spells added with discovery

Wand tier is the lowest wand that can hold the spell.

| Id | Spell | Kind | Mana | Tier | Effect |
| ---: | --- | --- | ---: | :-: | --- |
| 28 | Ember seed | bolt | 9 | 1 | Lobbed seed; ignites flammable ground on landing. |
| 29 | Meteor | bolt | 40 | 3 | Falls from above onto the aimed block; wide crater, fire, shove. Lowered under roofs. |
| 30 | Time snag | utility | 24 | 2 | Foes within 9 m slowed 3.5 s; hostile bolts within 12 m crushed to 35% speed. |
| 31 | Grapple | utility | 12 | 1 | Hooks the aimed block (32 m) and hauls you toward it. Free if nothing is in reach. |
| 32 | Leap | utility | 10 | 1 | Launches you up and forward; works in mid-air. |
| 64 | Reverse | modifier | 4 | 2 | Next bolt fires behind you. |
| 65 | Gravity | modifier | 3 | 1 | Bolt arcs downward; stacks to 2. |
| 66 | Phase | modifier | 12 | 2 | Bolt passes through the next wall (up to 12 voxels thick). |
| 67 | Reckless | modifier | 8 | 3 | +60% damage; each cast costs 4 health, never lethal. |
| 68 | Blood price | modifier | 0 | 3 | Pay half the mana cost in health instead; refuses when it would leave you under 4 health. |
| 69 | Fork | modifier | 10 | 2 | On impact, splits into three weak sparks that cannot fork again. |
| 70 | On timer | trigger | 7 | 2 | Like Trigger bolt, but the next spell fires 0.6 s into flight. |

## Discovery

A wand tier decides what it **can** hold (`spell_available`). A persistent
`SpellLog` (`src/world/adventure/discovery.rs`) decides what the player has
**found**. A spell is usable only when both are true (`Adventure::can_wield`,
`View::can_wield`). Spells 6 and 7 stay gated by the rare-magic bits.

- A fresh adventure (`Adventure::begin_journey`, called from the new-world path
  in `main.rs`) starts with Spark, Mend, Frost lance and Mirror ward.
- **Guardian drop:** the first time a guardian falls there is a deterministic
  roll (28% + 9% per menace level) keyed on site and slot, so reloading cannot
  reroll it. The chance thins as the wand's own tier empties, never below 35% of
  base.
- **Site clear:** always teaches one spell.
- **Wand upgrade:** teaches two spells; new slots are seeded only from found
  spells.
- Finds come mostly from spells the wand can use now. One in six peeks a tier
  ahead, so there is a reason to upgrade without a flood of unusable spells.
- Default state, every pre-19 save and every test is "everything found", so
  nothing a player owned is ever taken away.

### Pacing (menace 1, tier-1 wand, measured by `discovery::pacing` tests)

About 6 spells in the first 12 kills, 20 by 56 kills, and the tier drains in
roughly 170 kills. These are tested bands, not guarantees of feel: tune the drop
chance and the 24-spell slowdown reference in `discovery.rs` after playing.

### Palette states

- **Ready**: found and holdable.
- **Undiscovered**: the wand can hold it but it has not been found yet.
- **Locked**: the wand tier (or rare magic) is not high enough yet.

## Save format

`SAVE_VERSION` is 19. The found-spell log is three little-endian `u32` words
(96 bits) written right after the companion block. Saves before 19 have no such
block and load with every spell found. Spell ids above 95 would need a wider log
and another version bump.

## Not done yet

Identify-by-use runes, limited spell charges, shuffle wands, overheat, wand gem
mods, and oil and acid fluids.
