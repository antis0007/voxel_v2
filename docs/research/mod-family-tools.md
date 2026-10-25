# Modular tools and armor

Scope: [Addendum A.2](../specs/MOD_ADDENDUM.md#a2-tinkers-construct--plustic--tinkers-complement-constructs-armory-silent-gear-materialis). Claims: [C-MC3/4](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers).

Primary evidence checked 2026-10-24: [Tinkers' Construct](https://github.com/SlimeKnights/TinkersConstruct) documents assembled, modifiable tools and metal parts made using its multiblock smeltery. [Silent Gear](https://github.com/SilentChaos512/Silent-Gear) documents blueprint crafting and data-pack materials, parts and gear recipes. PlusTiC, Tinkers' Complement, Construct's Armory, Materialis and TiC Tool Leveling are scoped references; their version-specific behavior was not separately verified.

Mapping proposal: material/part tables, assembly evaluation and the shared A.15 modifier evaluator; actual molten cells and casting regions consume thermal/fluid/reaction services. Assembly is a new system; recipes are data. Target M3; do not label the whole feature data-only before those services exist.

## What we adopt

Parts, visible material choices, repair, casting and explicit modifier tradeoffs.

## What we reject

Separate armor/tool trait engines, arbitrary per-item physics, and assuming all historical addons share the same slots or traits.

## What we improve

Derive conductivity, burning and other effects from material rules; explain each part's contribution in tooltips. Validate new-material coverage and conservation when liquid spills or casting fails; neither claim is yet proven.
