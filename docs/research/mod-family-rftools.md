# RFTools family

Scope: [Addendum A.4](../specs/MOD_ADDENDUM.md#a4-rftools--rftools-dimensions-control-builder-storage-power-utility): RFTools, Dimensions, Control, Builder, Storage, Power, Utility. Claims: [C-MC4](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-S3/7](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: [McJty's docs](https://mcjty.eu/docs/mods/rftools) link storage/spawner tutorials; [Dimensions](https://mcjty.eu/docs/mods/rftools-dimensions) documents dimension descriptors built from dimlets and package-defined creation/maintenance costs; [Control](https://mcjty.eu/docs/mods/rftools-control) is the author's controller reference. Exact historical shape-card and screen-module inventories are not reverified here.

Mapping proposal: builder region jobs emit budgeted Actions; screens read existing queries; field effects use FieldEmitter; dimensions and packed regions share world persistence; Control is a front end for the shared VM. M4 machines, M6 worlds, M7 controllers; region jobs/world lifetime are new systems.

## What we adopt

Shape-directed construction, configurable worlds, reusable screens and discoverable machine control.

## What we reject

Separate teleporters/virtual worlds per feature and copying version-specific RF prices as physical constants.

## What we improve

Expose quarry support and liquid consequences, stable cancellation/resumption, world-interface costs and nested-world ownership. A configurable dimension is not evidence of working orbital or nested-world physics.
