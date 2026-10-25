# Mapping and wayfinding

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include): Waystones, JourneyMap, Xaero's Minimap/World Map. Claims: [C-MC4/5](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-Z2](../specs/GAME_BIBLE.md#zork-worldbuilding-not-text-interaction).

Primary evidence checked 2026-10-24: [JourneyMap's author page](https://www.curseforge.com/minecraft/mc-mods/journeymap) describes real-time exploration maps in minimap, full-screen and browser forms. [Xaero's author page](https://www.curseforge.com/minecraft/mc-mods/xaeros-minimap) documents waypoints, cave mapping, configurable radar and server restrictions. [Waystones' author page](https://www.curseforge.com/minecraft/mc-mods/waystones) was reached; exact travel/unlock/cost rules were not traced.

Mapping proposal: known-chunk summaries and discovered landmarks drive map UI; travel nodes use the one LinkNetwork. M6 world/map, linked transport M4–M6; discovery persistence and cave slices need code.

## What we adopt

Useful landmarks, readable vertical context, remembered routes and customizable waypoint presentation.

## What we reject

Accidental disclosure of secret terrain, a second teleport engine and default radar that defeats intended exploration.

## What we improve

Tie labels to generated names/functions, show known versus inferred information, and price travel through shared infrastructure. Map legibility and discoverability still require player-facing review.
