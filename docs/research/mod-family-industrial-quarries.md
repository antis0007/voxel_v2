# Industrial power, pipes and quarries

Scope: [Addendum A.12](../specs/MOD_ADDENDUM.md#a12-industrialcraft-2--advanced-solar-gravisuite-tech-reborn-buildcraft--additional-pipes-logistics-pipes-quarries-bc-quarry-quarry-plus-digital-miner-rftools-builder-ender-quarry-quantum-quarry): IC2, Advanced Solar, GraviSuite, Tech Reborn, BuildCraft, Additional Pipes, Logistics Pipes, BC Quarry, Quarry Plus, Digital Miner, RFTools Builder, Ender Quarry, Quantum Quarry. Claims: [C-MC1/2/4](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers).

Primary evidence checked 2026-10-24: [BuildCraft's repository](https://github.com/BuildCraft/BuildCraft) establishes the author source. IC2's [official wiki](https://wiki.industrial-craft.net/index.php?title=Main_Page) returned a fetch error, so voltage loss, overvoltage and component-grid details are not independently confirmed here. Derivative projects and individual quarry behaviors remain version-specific, unverified leads; RFTools and Mekanism have separate family notes.

Mapping proposal: one electric graph, machine/reaction tables, shared item/request planner, blueprint region jobs and rail-following drill bodies. Real extraction/support/fluid consequences are our design. M4; controller, reservation and recovery paths need design.

## What we adopt

Visible extraction progress, power tiers, processing chains, per-face pipe rules and request-driven delivery.

## What we reject

Quarry variants backed by duplicate extraction code, offscreen mining that silently bypasses conservation, and unverified exact voltage numbers.

## What we improve

Expose heat loss and failure causes, recover safely from interruption, and show excavation, tailings and water ingress. These effects are targets, not demonstrated runtime behavior.
