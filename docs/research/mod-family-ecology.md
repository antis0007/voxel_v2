# Trees, seasons and survival climate

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include): Dynamic Trees, Serene Seasons, Tough as Nails. Claims: [C-MC2/4/5](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-S1](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: [Dynamic Trees](https://github.com/DynamicTreesTeam/DynamicTrees) describes trees growing progressively from seed to maturity; [Serene Seasons](https://github.com/Glitchfiend/SereneSeasons) identifies seasonal color/temperature shifts; [Tough as Nails](https://github.com/Glitchfiend/ToughAsNails) identifies thirst and body temperature. Their detailed growth, greenhouse and heat-transfer algorithms were not traced.

Mapping proposal: shared growth/material rules, seasonal world forcing, moisture/light/temperature queries and player needs; genetics reuses Forestry's evaluator. M3 survival, M6 world seasons; plant structural growth and climate coupling require code before species are data-only.

## What we adopt

Visible long-term growth, seasonal preparation and survival needs with understandable environmental causes.

## What we reject

Separate ambient-temperature systems, arbitrary hidden seasonal crop failure and growth that creates unaccounted mass.

## What we improve

Make shelter, irrigation and greenhouses work through shared physics; show why a plant is stalled or a player is cold. Cost, biological simplifications and seasonal pacing remain acceptance questions.
