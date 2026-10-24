# Research notes (v2 spec, Part II)

Each note is at most 8 lines: what we adopt, what we reject, and why, with its sources. Claims a note could not confirm from a primary source are marked unverified. `docs/ENGINE_CORE.md` and `docs/GAME_DESIGN.md` cite these.

## Engine
| Spec topic | Note |
|---|---|
| Purho, "Exploring the Tech and Design of Noita" (GDC 2019) | `noita-gdc2019.md` |
| Toffoli & Margolus, block cellular automata | `margolus-ca.md` |
| Gallant fluids; Terraria and Minecraft liquid levels; compression | `fluid-flow-liquid-levels.md` |
| Brickmap, ESVO, GigaVoxels | `brickmap-esvo-gigavoxels.md` |
| Teardown destruction, sleep, debris, ray marching | `teardown-destruction.md` |
| Greedy and binary greedy meshing | `greedy-meshing.md` |
| GPU-driven rendering, visibility buffer | `gpu-driven-rendering.md` |
| HPA*, Recast, Dwarf Fortress connectivity, Minecraft goal selector | `pathfinding-nav.md` |
| Factorio Friday Facts: belts, fluids, update order, circuits | `factorio-updates.md` |
| "1500 Archers on a 28.8" (lockstep) | `lockstep-aoe.md` |
| Minecraft 1.18 noise routers, Terralith, Tectonic | `worldgen-noise.md` |
| Dave Mark utility AI, Sims needs, Rain World | `utility-ai.md` |

## Gameplay and mods
`MOD_FEATURE_MATRIX.md`: 153 features from vanilla Minecraft, about 40 mods, Noita, Terraria, Dwarf Fortress and Zork. Each row gives the feature, its source, how it maps onto our physics and data model, a cost (data-only, small code, new system) and a milestone (M1–M7).
