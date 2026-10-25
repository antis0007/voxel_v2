# PneumaticCraft

Scope: [Addendum A.14](../specs/MOD_ADDENDUM.md#a14-and-more--additional-references-the-matrix-must-include). Claims: [C-MC1/2/4](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-S1/3](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: [Repressurized's maintainers](https://github.com/TeamPneumatic/pnc-repressurized) identify the port and its original author. The [original author's in-game text](https://github.com/MaartenKok8/PneumaticCraft/blob/master/resources/assets/pneumaticcraft/lang/en_US.lang) describes pressure instruments, compressors, heat-dependent refining, puzzle-piece drone programming and remote controls. Exact current pressure equations and drone limits remain unverified.

Mapping proposal: conserved gas inventories/pressure, thermal recipes, shared BodyController and instruction-budgeted VM; M4 pressure machines, M7 programming front ends. Pressure transport and robot navigation need new code, not only configuration.

## What we adopt

Pressure as a useful engineering constraint, visible gauges, heat management and approachable drone programs.

## What we reject

A separate drone language runtime, unbounded programs or pressure values with no stored gas/volume meaning.

## What we improve

Teach leaks, overpressure and thermal coupling with the same gas physics used in habitats; identify a stopped program's instruction, blocked target and resource deficit. These are proposed acceptance behaviors.
