# Generator 66: roof bearings and the depths entrance

New worlds overlap cottage roof courses by pitch times voxel size and fill
side-wall bearings up to the roof underside. This addresses gaps from sampling
sloped roofs above fixed wall heights. The market well receives the same roof
overlap and continuous supports.

The Undercrown well gains a surface landing, an open stone apron and two lit
posts. The existing descending spiral and underground mechanisms are retained.
Generator 65 and earlier keep their geometry; saved worlds are not migrated.

The world map's `Depths entrance` button creates a waypoint at the authored
well mouth and centres the map there, including in existing supported worlds.
It does not reveal terrain tiles or teleport the player.

Spell progression changes also apply to existing adventures: guardian first-kill
drop chances are 65/75/85/95 percent by menace, without a second rejection roll.
Discoveries prefer currently usable spells; future tiers are selected only once
that pool is empty. Wand upgrades teach four spells. Existing learned spells,
rare-reliquary gates and defeated-guardian records are preserved.

Validation: offline application `cargo check` passed. Tests were not run.
Roof continuity, entrance traversal and progression pacing still need in-game
review at the supported voxel scales. This is a focused repair, not a completed
review of all generated structures.
