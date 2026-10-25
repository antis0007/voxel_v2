# Applied Energistics 2

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include), Applied Energistics 2. Claims: [C-MC4/5](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-S7](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: [official guides](https://guide.appliedenergistics.org/) separate Minecraft versions. The [official archived channels page](https://appliedenergistics.org/ae2-site-archive/Channels/index.html) documents network-service capacity; [archived P2P documentation](https://appliedenergistics.org/ae2-site-archive/P2P-Tunnel/index.html) describes typed input-to-many-output transport and forbids recursive tunnel nesting. Those are historical mechanics; exact present-version limits were not checked. Spatial-storage page fetch failed.

Reference checklist: cells, drives, terminals, storage buses, import/export buses, crafting patterns, assemblers, crafting processors, channels, controllers, P2P, quantum links, spatial storage. Items beyond the cited channel/P2P mechanisms remain study leads here.

Mapping proposal: one inventory index/planner, capacity-limited network ports, LinkNetwork and shared pocket-world serialization; M4 storage/crafting, M6–M7 world transfer. These are new systems.

## What we adopt

Unified inventory access, explicit capacity, reusable recipes and diagnosable network bottlenecks.

## What we reject

Copying historical channel counts without a purpose or treating region packing as ordinary item serialization.

## What we improve

Expose missing ingredients, reserved items and blocked topology; preserve every entity, program and ownership record during world transfer. Nested-world acceptance is our requirement, not an AE2 source claim.
