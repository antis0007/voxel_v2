# Terrain parameters and historical identity

Reviewed 2026-10-24; W01/W03/W04. [Mojang's 21w44a notes](https://www.minecraft.net/en-us/article/minecraft-snapshot-21w44a) are a historical official source for the 1.18 transition. Exact noise-router equations were not verified here; Terralith/Tectonic mechanisms and scope are covered in the mod-family notes. No biome-count claim is required for adoption.

## What we adopt

Use shared world-space climate/geology inputs for landforms, biome palettes and stratum placement. Current generator code and immutable historical version gates are the actual migration baseline.

## What we reject

Do not infer that published biome aesthetics prove a particular router implementation, or that floats in rendering/worldgen automatically violate the narrower authoritative-state rule. Never silently regenerate existing saves with a newer generator.

## What we improve

Name each generation stage, its units, seed/hash inputs, cache ownership and output revision in W01. Civilizations and functional landmarks consume geology and maintain coherent supply/water routes rather than being independent noise decorations.
