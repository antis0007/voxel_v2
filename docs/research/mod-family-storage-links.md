# Physical storage and linked inventories

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include): Sophisticated Backpacks/Storage, Storage Drawers, Iron Chests, Ender Storage, Flux Networks. Claims: [C-MC1/4/5](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers).

Primary evidence checked 2026-10-24: [Sophisticated Backpacks](https://github.com/P3pp3rF1y/SophisticatedBackpacks) describes placed/portable upgradeable bags; [Sophisticated Storage](https://github.com/P3pp3rF1y/SophisticatedStorage) establishes its source. [Storage Drawers](https://github.com/jaquadro/StorageDrawers) identifies compartmental storage and [Iron Chests](https://github.com/progwml6/ironchest) chest variants. [Ender Storage](https://github.com/TheCBProject/EnderStorage) documents color-linked chests/pouches/tanks and tank I/O modes; [Flux Networks](https://github.com/SonarSonic/Flux-Networks) identifies wireless energy. Exact capacities and upgrade limits were not reverified.

Mapping proposal: shared inventory records and UI schemas, Modifier upgrades, common filters and LinkNetwork keyed by owner/frequency. M3 local storage, M4 links; transactional transfer/persistence are new services.

## What we adopt

Legible specialized storage, portable organization and linked access with a clear identity.

## What we reject

Separate inventories or energy implementations per container style, silent overflow deletion and ambiguous private/public frequencies.

## What we improve

Expose physical capacity, transfer cost and queued reservations; preserve contents across destruction, packing and interrupted transfers. Conservation and ownership are planned invariants, not verified runtime results.
