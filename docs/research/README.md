# Research index

2026-10-24 Session-1 research package for the [bible](../specs/GAME_BIBLE.md).
Notes are at most one page and end with adopt/reject/improve decisions.
They separate primary-source findings from proposed mechanisms and unresolved
reference details. An inaccessible source is a research gap, not proof of its
contents. These notes establish design inputs; none passes a game claim.

## Engine topics

| Topic | Note |
|---|---|
| Noita dirty regions/body coupling | [Noita engine](noita-gdc2019.md) |
| Toffoli/Margolus partitions | [Block CA](margolus-ca.md) |
| Gallant and cellular liquid levels | [Fluid models](fluid-flow-liquid-levels.md) |
| Brickmap/ESVO/GigaVoxels | [Sparse storage](brickmap-esvo-gigavoxels.md) |
| Teardown destruction/save/rendering | [Teardown](teardown-destruction.md) |
| Greedy/binary meshing | [Meshing](greedy-meshing.md) |
| GPU-driven pipelines | [Submission](gpu-driven-rendering.md) |
| HPA*/Recast/connectivity/goals | [Navigation](pathfinding-nav.md) |
| Factorio belts/fluids/update boundaries | [Factorio](factorio-updates.md) |
| AoE commands/determinism | [Lockstep](lockstep-aoe.md) |
| Minecraft terrain and world identity | [World generation](worldgen-noise.md) |
| Mark/Sims/Rain World and utility proposals | [Utility AI](utility-ai.md) |

## Bible reference topics

- [Heavily modded Minecraft](reference-minecraft.md)
- [Noita systems, secrets and composition](reference-noita.md)
- [FTL](reference-ftl.md)
- [Zork](reference-zork.md)
- [Stephenson's cited works](reference-stephenson.md)
- [Five working-title proposals](naming-proposals.md), all unselected

Zork's original hints document alternative Loud Room solutions; FTL preserves
unlocks; existing mod ecosystems have integrations. The bible's criticisms are
qualified by those findings while all C-* target obligations are retained.

## Mod families

A.1–A.13 each have a note; A.14 is divided into coherent families, including
separate notes for all eight large systems. BYG/Oh The Biomes You'll Go is one
reference, rather than two independent mods. Original-only Mana and Artifice
is included under authored spells. A.15 is a shared-mechanism contract in the
master inventory, rather than another mod.

| Family | Note |
|---|---|
| Applied Energistics 2 | [Research](mod-family-ae2.md) |
| Authored and composable spell systems | [Research](mod-family-authored-spells.md) |
| Blood Magic | [Research](mod-family-blood-magic.md) |
| Botania | [Research](mod-family-botania.md) |
| Building variants and useful decoration | [Research](mod-family-building.md) |
| Computing, logic and programmable spells | [Research](mod-family-controllers.md) |
| Create | [Research](mod-family-create.md) |
| Adventure dimensions and pocket worlds | [Research](mod-family-dimensions.md) |
| Recipe discovery, accessories and affixes | [Research](mod-family-discovery-gear.md) |
| Draconic Evolution | [Research](mod-family-draconic.md) |
| Trees, seasons and survival climate | [Research](mod-family-ecology.md) |
| Ender IO | [Research](mod-family-enderio.md) |
| Endgame sinks and transmutation | [Research](mod-family-endgame-transmutation.md) |
| Explosives, missiles and sensors | [Research](mod-family-explosives.md) |
| Factorio-style factory progression | [Research](mod-family-factorio.md) |
| Fauna, bosses and elite creatures | [Research](mod-family-fauna-bosses.md) |
| Farming, cooking and kitchens | [Research](mod-family-food.md) |
| Forestry and genetics | [Research](mod-family-forestry.md) |
| GregTech CEu | [Research](mod-family-gregtech.md) |
| Immersive Engineering | [Research](mod-family-immersive-engineering.md) |
| Industrial power, pipes and quarries | [Research](mod-family-industrial-quarries.md) |
| Mekanism family | [Research](mod-family-mekanism.md) |
| Mapping and wayfinding | [Research](mod-family-navigation.md) |
| Reactors, radiation and accelerators | [Research](mod-family-nuclear.md) |
| Obscuria Collection | [Research](mod-family-obscuria.md) |
| PneumaticCraft | [Research](mod-family-pneumaticcraft.md) |
| Refined Storage | [Research](mod-family-refined-storage.md) |
| Resource crops and compact cultivation | [Research](mod-family-resource-crops.md) |
| RFTools family | [Research](mod-family-rftools.md) |
| Celestial and ritual magic | [Research](mod-family-ritual-magic.md) |
| Force fields and security | [Research](mod-family-security.md) |
| Colonies, villages and trade | [Research](mod-family-settlements.md) |
| Space exploration and life support | [Research](mod-family-space.md) |
| Physical storage and linked inventories | [Research](mod-family-storage-links.md) |
| Terrain, biomes and generated structures | [Research](mod-family-terrain-structures.md) |
| Thaumcraft 4 and 6 | [Research](mod-family-thaumcraft.md) |
| Thermal Series and powered equipment | [Research](mod-family-thermal.md) |
| Modular tools and armor | [Research](mod-family-tools.md) |
| Utility automation and channel routing | [Research](mod-family-utility-automation.md) |

## Feature-level accounting and open evidence

[Canonical A.16 matrix](MOD_FEATURE_MATRIX.md) preserves all 153 earlier feature
identities, adds the addendum scope, and names system dependencies, cost, pillars,
paths, milestones, risk and intended improvement. The older five-column view is
[preserved verbatim](MOD_FEATURE_MATRIX_LEGACY_20261024.md). Legacy numeric stats
and derivative attribution remain unverified unless a note explicitly traces them.
A source name or author repository alone does not verify every listed mechanic.

Unresolved engine evidence includes the maintenance-blocked GDC talk, full
Margolus book derivations, inaccessible HPA*/Gallant/GigaVoxels material, exact
Minecraft router internals and utility/Sims/Rain World internals. Mod notes flag
historical derivatives, ambiguous variants and incomplete per-feature tracing.
Stephenson Feed/Seed and causal-domain specifics are not engineering evidence.
Resolve any detail needed for an algorithm before its SDD passes; broad identity
checks do not satisfy the critic's mechanism requirement.

The [spike register](../plans/2026-10-24-bible-spikes.md) retains all ten required
experiments. The later Downloads intake supplied
[the hardware addendum](../specs/ADDENDUM_B_HARDWARE.md); H0-H5 targets now have
a source, while fallback/device acceptance remains open. The
[hardware reconciliation](../plans/download-coverage/hardware.md) records
remaining contracts/conflicts. No code, test, benchmark or hosted CI was added
or run in research.
