# Cellular liquids and pressure

Reviewed 2026-10-24; C06/C07/M11 and C-N5. The earlier Gallant title was not confirmed; the [author's liquid-CA article](https://www.jgallant.com/2d-liquid-simulator-with-cellular-automaton-in-unity/) returned 403. Exact Minecraft/Terraria update algorithms remain unverified. [Factorio's developer fluid study](https://www.factorio.com/blog/post/fff-416) deliberately chooses aggregate pipeline behavior, not free-surface cell pressure.

## What we adopt

Compare pressure-column, relaxation and SPH-lite in S2 using identical u_tube/dam_break geometry and mass. Transfer integer quantities through a deterministic face ownership scheme and retain all rounding residuals.

## What we reject

Delete the earlier thin-film-vanishing proposal: sleeping, aggregation or a residual reservoir must retain mass. Do not copy pipeline-global pressure into an open flood or squeeze compression values into a field that cannot represent them.

## What we improve

Record equalization ticks, mass error, cost and visible artifacts. Define mixed-resolution units and bounded pressure overflow before C06 can pass; choose a model from measurements, without adding a parallel legacy writer.
