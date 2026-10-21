# Parity audit: Noita, Tinkers' Construct, Mekanism

Counts are from the code at the time of writing (`src/world/adventure*`,
`src/world/factory*`). Nothing here was playtested; behaviour is covered by unit
tests only.

## Enemies

| | Noita | Here before | Here now |
|---|---|---|---|
| Regular enemy kinds | 60+ | 6 (ids 0-5) + 1 elite (9) | unchanged |
| Bosses | ~10 | 3 | unchanged |
| Elemental variants | per-creature, many | none | 9 affixes, up to 2 per enemy |
| Status effects on enemies | burn, wet, poison, frozen, electrified, many | frost slow only | burn, poison (stacking), shock, wet + reactions |

Variety now comes from combination, not new models: 6 kinds x 44 affix
combinations. Affixes: Fiery, Frostbound, Static, Toxic, Volatile, Armored,
Regenerating, Swift, Warded. Odds rise with encounter menace, and underground
zones bias the roll (Ember layers breed Fiery and Volatile, Frost layers
Frostbound and Armored). Rules and tests: `src/world/adventure/status.rs`.

Reactions implemented: wet + shock conducts (x1.8), wet + frost flash-freezes,
fire on wet boils to steam, water douses burning, poison stacks to five.

**Still missing vs Noita:** new silhouettes and behaviours (shamans that heal,
splitters, snipers, summoners), enemies that interact with terrain materials,
and boss variety. Splitting was deliberately skipped: enemy slots are tied to
per-site kill records, so children need a separate summon path.

## Spells and modifiers

Before: 11 projectile spells, 28 modifiers/multicasts, 3 effects. Now 8 more
modifiers (ids 56-63): Ignite, Static charge, Venom coat, Drench, Cold snap,
Chain arc, Shrapnel, Overcharge. Noita has roughly 380 spells.

**Still missing:** material-spawning spells (the heart of Noita's falling-sand
play), passive and always-cast perks, wand shuffle, spell upgrades, wand
generation with random stats, and most projectile varieties.

## Tinkers' Construct

Weapons remain tiered chassis (`sword_name`), but their fitted parts now use a
real material table: head, binding and grip each accept Copper, Bronze, Steel,
Cinder, Rime or a PlusTiC-style Polymer fitting. Material traits include fire
and frost edges, and the existing five modifier bits are exposed as named
enchantments with costs. The **Forge** tab in the inventory previews damage,
sweep and swing multipliers, generated names and trait chips. Save bytes stay in
the existing adventure block; the decoder now accepts 0–5 and rejects anything
else. Details and verification: `docs/TINKERING_FORGE_AND_CATALOGUE.md`.

**Still missing:** durability and repair, part melting/refunds, per-tool
inventory instances (fittings are currently shared by the equipped melee tier),
tool leveling, and a 3D assembled preview.

## Mekanism / tech

16 machines, 19 resources, power cables, item pipes, fluid pipes (water only).
Mekanism has 100+ machines, gases and chemicals, multiblocks, tiered cables and
a fission reactor.

Added: **Arcane pile**, a Generator mode. Burns arcane cores (which the
Fabricator already makes) for 4x coal's power. Heat is stored in the node,
coolant is piped water (the washer's supply), and an overheated pile melts
down with a blast, a shove and fire, then stays disabled and hot until cooled.
Demand is the control rod: a full buffer stops fission. Tests in
`src/world/factory/pile.rs` cover meltdown, stable cooling, idle cooling and a
save round trip.

Added: **item transit and extractor pipes** (`src/world/factory/transit.rs`).
Items now travel the pipes as packets at 4 m/s instead of teleporting, and are
drawn at their real position inside translucent glass tubes. Pipes have two
modes: Conveyor (machine outputs flow to machines that need them, as before)
and Extractor (also pulls from adjacent crates and machine outputs, with an
optional single-resource filter). Destination capacity is reserved at launch, so
a delivery never overflows; pulling a pipe out refunds what was inside; saving
mid-flight saves the items as delivered. A pipe path is the shortest route
through the network. Extractor pipes carry a glowing cyan hub and collars.

Review fixture: `VOXEL_TORCH_REVIEW_MODE=factory` with `--benchmark-torch-review`
builds an extractor line, a climbing line and a powered line and captures frames.

**Not done for pipes:** per-side pull/push/off configuration (Mekanism's
configurator), pipe speed tiers, round-robin versus nearest-destination modes,
and priority.

**Blocked on a decision:** more machines, more resources, or chemicals. The
factory save is versioned and strictly validated (`Factory::decode`), kit counts
follow `Kind::ALL`, and resources follow `RESOURCE_COUNT`. Growing either needs
a new save version and a migration, which is a change to existing players'
worlds and should be a deliberate step.

## Underground zones

Layered environments with archetypes and sub-aspects live in
`src/world/zones.rs`. They drive the screen grade, spell particles, the entry
banner and enemy affix odds.
