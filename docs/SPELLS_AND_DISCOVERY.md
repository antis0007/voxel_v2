# Spells, wand tiers and spell discovery

This documents how spells are wired, the twelve spells added with the discovery
update, how a player finds spells, and the save-format change that persists it.

## How a spell is wired

A spell is a plain `u32` id (`0..=MAX_SPELL_ID`, currently 95). There is no
single registry yet, so a new spell touches these places. A test guards the ones
marked (T).

| Where | What |
| --- | --- |
| `src/world/adventure.rs` | `spell_name`, `spell_flavor`, `spell_hint`, `spell_cost`, `spell_cooldown`, `spell_cast_delay_delta`, `spell_recharge_delta`, `spell_spread_delta`, `spell_base_damage`, `spell_base_speed`, `spell_kind_tag`, `spell_color`, `spell_available` (T: complete, unique, reachable) |
| `src/world/adventure/wands.rs` | `projectile()` / `is_modifier()` / `is_effect()` classification (T: must agree with the kind tag), `paint_shot` for stat-only modifiers, control-flow cards in `evaluate_programmed`, `spell_audio`, and the cast `match` in `cast_wand` |
| `src/world/adventure.rs` `update` | projectile flight and impact effects |
| `src/egui_ui/wands.rs` | the palette tab lists `BOLTS`, `EFFECTS`, `SHOT_MODIFIERS`, `PROGRAM_MODIFIERS` (T: every defined spell must be on a tab, or it can never be placed) |
| `tools/gen_spell_icons.py` | icon art; run it to regenerate `src/egui_ui/icons_spells.rs` (T: readable icons) |

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
mods, oil and acid fluids, and a single data-driven `SpellDef` table replacing
the parallel `match` tables above.
