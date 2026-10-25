# Adventure dimensions and pocket worlds

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include): Twilight Forest, Aether, Deeper and Darker, Alex's Caves (including acid/nuclear biomes), Compact Machines, Dimensional Doors. Claims: [C-N3](../specs/GAME_BIBLE.md#noita-lore-worldbuilding-magic-secrets), [C-Z2/3](../specs/GAME_BIBLE.md#zork-worldbuilding-not-text-interaction), [C-S7](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: author repositories for [Twilight Forest](https://github.com/TeamTwilight/twilightforest), [Aether](https://github.com/The-Aether-Team/The-Aether), [Alex's Caves](https://github.com/AlexModGuy/AlexsCaves), [Dimensional Doors](https://github.com/DimensionalDevelopment/DimDoors) and [Compact Machines](https://github.com/CompactMods/CompactMachines) establish the sources; Compact Machines explicitly describes rooms inside blocks. [Deeper and Darker's author page](https://www.curseforge.com/minecraft/mc-mods/deeperdarker) was reached. Exact progression gates, cave chemistry and region-transfer semantics were not traced.

Mapping proposal: biome/structure data, shared boss/encounter rules and one world-lifecycle/serialization service with explicit parent interfaces; M6, substantial world persistence work.

## What we adopt

Distinctive sites, memorable local hazards and compact interior spaces with useful entrances/interfaces.

## What we reject

Duplicating dimension code, silently losing entities during transfer and confusing underground biomes with separate dimensions.

## What we improve

Make pockets and planets share ownership, costs and save behavior; use physical multi-solution gates and contextual lore. Two-level nesting and chemistry are our unverified targets.
