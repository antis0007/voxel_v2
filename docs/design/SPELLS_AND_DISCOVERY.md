# Spells, wand tiers and spell discovery

This documents how spells are wired, the twelve spells added with the discovery
update, how a player finds spells, and the save-format change that persists it.

## How a spell is wired

A spell is a stable `u32` ID. The sorted table in
[assets/spells.ron](../../assets/spells.ron) defines names, colors, mana costs,
base timings, sound, class, wand tier and Gear category. Hover hints and Shift codex copy live
in [assets/descriptions.ron](../../assets/descriptions.ron).

Gear and the Field Guide Spell Index discover spells from this catalog. Add a
row in ID order and keep IDs within the four-word save log. New behaviors
belong in the casting, projectile or deck evaluator. Author a glyph in
[gen_spell_icons.py](../../tools/gen_spell_icons.py) for a custom icon.

Use Spell Wiring in Axiom Dev or the shared command interface to tune balance
and library settings. See the [Dev Panel guide](../../tools/axiom-dev/README.md).

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

## Don't Panic

A tier-I utility (ID 100) from the ordinary spell discovery pool. Equip it under
Effects as a single spell card. It costs 18 mana and has a 5-second base cooldown.
For 4.2 seconds after the cast, the ward absorbs the next damaging landing.
A harmless landing leaves the ward armed until it expires.

The cover of Douglas Adams's
[The Hitchhiker's Guide to the Galaxy](https://www.panmacmillan.com/authors/douglas-adams/the-hitchhikers-guide-to-the-galaxy/9780330513081)
inspired the name and the field-guide glyph. A scuffed teal cover, brass clasp
and folded towel use Axiom's existing pixel materials and lighting roles.
The codex places the joke in the local transit office's paperwork.

## Noita-style spells and the Chaotic chassis

| Id | Spell | Kind | Mana | Tier | Effect |
| ---: | --- | --- | ---: | :-: | --- |
| 71 | Chainsaw | bolt | 1 | 2 | A tiny rapid slicing bolt with about 5 m of reach and 7 damage, so it is nearly free to spam. |
| 72 | Teleport bolt | bolt | 24 | 2 | On impact you swap to where it landed, backed off the wall until your body fits. |
| 73 | Flood bolt | bolt | 16 | 2 | On impact it pours up to 18 water cells and soaks foes within 3.5 m, setting up shock, frost and steam. |
| 74 | Venom nova | bolt | 22 | 2 | On impact a toxic burst poisons every foe within 3.4 m that it can see. |
| 75 | Ice wall | utility | 20 | 2 | Raises a 3 m wide, 2.5 m tall ice barrier 3 m ahead (empty cells only, capped at 400). Heat melts it. |
| 76 | Bounce orb | bolt | 8 | 1 | A slow light orb that always ricochets four times. |
| 77 | Mana leech | modifier | 6 | 2 | Each hit restores 4 mana. |
| 78 | Critical plus | modifier | 7 | 2 | One hit in four lands for 2.5x damage (deterministic from the clock and target). |
| 79 | On expire | trigger | 7 | 2 | Like Trigger bolt, but the next spell also fires if the bolt times out without hitting anything. |

Chainsaw and Bounce orb are the only spells allowed to deal less than the legacy
22-damage floor (`spell_hit_damage`).

**Chaotic chassis (wand mode 3).** The chassis picker already offered "Shuffle"
but nothing shuffled. It now reorders the occupied slots, within the wand's
capacity, each time the deck reloads (the cast that wraps the deck). Empty slots
never move and the set of spells never changes. The order is a seeded
Fisher-Yates pass; the seed is not saved, so a loaded game deals a fresh order.
Hover cards show Shuffle: Yes for this chassis only.

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

## Bookcase study and grimoire casting

Interact with an ordinary bookcase to open the field guide and choose a lesson.
Hydromancy costs 20 XP and teaches Water Transfer. Protective Magic costs 35 XP
and teaches Ice Wall and Aegis Field. Celestial Magic costs 60 XP and teaches
Meteor and Plasma Cross. Each lesson is independent. Existing Magic Affinity
ranks are carried forward as the corresponding completed lessons.

Threshold Survey costs 20 XP. It adds frame diagnostics to the guide and shows
sanctuary coordinates in the Hills after reading a porter's record. Horticulture
costs 30 XP and adds one ripe crop per harvest.

Studied spells appear in the Gear palette. Place them on an instrument of the
required tier. Channel Aegis Field or Plasma Cross from a single card by holding
Cast. A Focus Grimoire reduces their base upkeep from 8 to 6 mana/s and from
16 to 12 mana/s respectively. A Reservoir Grimoire increases capacity by 25%.

## Circle and gate travel

- **Circle link** · Gear → Effects → Teleportation Circle. Starting spell,
  tier I wand. Sneak-cast alone at each endpoint, then stand inside either
  circle and cast to travel. One saved pair per world. Sneak-cast elsewhere
  to move the far end, or inside to clear the pair.
- **Quick return** · Cast outside a linked circle, release, then cast again to
  return. One mark, 24 m range, 30 s lifetime. Successful casts cost 18 base mana.
- **Home escape** · With a circle drawn, hold the cast 3 s anywhere in the same
  realm to land on the first (home) circle. One way, no range limit; a hit
  restarts the hold. Circles are stored per realm, so it never crosses realms.
- **Crystal Gates** · Build two upright Resonant Crystal frames with matching
  clear openings. Interact with the first to attune it and the second to link.
  Walk through either opening. Gear → Teleportation Circle · Guide links to
  the Crystal Gates diagram and recipe.

## Learning from materials

Thunder charge visibly discharges when it enters a pool. When the discharge
spreads across connected water, the caster can discover Storm Surge. The
observation belongs to that player and travels with their progression.
Storm Surge requires a tier III instrument, costs 42 mana, and has a stronger,
wider discharge. Dry breaks interrupt it. The machine charge budget stays fixed.

## Save format

`SAVE_VERSION` is 19. The found-spell log is three little-endian `u32` words
(96 bits) written right after the companion block. Saves before 19 have no such
block and load with every spell found. Spell ids above 95 would need a wider log
and another version bump.

## Not done yet

Identify-by-use runes, limited spell charges, overheat, wand gem mods, and oil
and acid fluids.
