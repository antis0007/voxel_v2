# Computing, logic and programmable spells

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include): Integrated Dynamics/Tunnels, ComputerCraft, OpenComputers, Hexcasting. Claims: [C-MC4](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-S3/6](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: [CC: Tweaked](https://github.com/cc-tweaked/CC-Tweaked) documents programmable computers/turtles and its ComputerCraft lineage; [OpenComputers](https://github.com/MightyPirates/OpenComputers) is its maintainer source. [Integrated Dynamics](https://github.com/CyclopsMC/IntegratedDynamics) and [Tunnels](https://github.com/CyclopsMC/IntegratedTunnels) identify automated control and networked item/fluid/energy transfer. The [Hex Book](https://hexcasting.hexxy.media/v/latest/main/en_us/) explicitly covers patterns, stacks, media and mishaps, and warns its latest-code documentation may exceed released features. OpenComputers suspension persistence was not traced.

Mapping proposal: one deterministic VM, capability-limited I/O and Action outputs; text/node/glyph front ends share saved program state. M7; VM, debugging and persistence are new systems.

## What we adopt

Composable programs, inspectable state, useful world peripherals and magical notation with real computational meaning.

## What we reject

Multiple incompatible runtimes, unrestricted host I/O and assuming a Lua interpreter automatically meets deterministic replay constraints.

## What we improve

Expose instruction budgets and faults, persist resumable state, and make encryption a disclosed toy cipher taught by the Primer. Scheduler, privilege and nested-world behavior remain design obligations.
