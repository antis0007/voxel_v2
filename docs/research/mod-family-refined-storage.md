# Refined Storage

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include). Claims: [C-MC4/5](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers).

Primary evidence checked 2026-10-24: [official documentation](https://refinedmods.com/refined-storage/) describes item/fluid storage on an expandable network, a unified Grid, import/export/world interaction and autocrafting. It offers version selectors; exact capacity, controller and channel rules must be tied to a selected version. Earlier matrix claims assigning ordinary autocrafting to an addon need correction against base documentation.

Mapping proposal: the same inventory index, recipe planner and link service as AE2; storage shapes and access UI are content/configuration choices, not another implementation. M4; reservations, missing-resource diagnosis and failure recovery are code work.

## What we adopt

A straightforward unified inventory view and approachable automation requests.

## What we reject

Separate storage back ends per inspiration or treating a convenient terminal as infinite physical storage.

## What we improve

Make quantities, capacity, pending reservations and transfer costs explicit; show why crafting is blocked and recover conservatively from interrupted jobs. These are targets, not verified engine behavior.
