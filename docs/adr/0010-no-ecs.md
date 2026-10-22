# 0010 No ECS crate; entity kinds are SoA tables in World

Date: 2026-10-22. Status: accepted.

Machines, enemies, projectiles and other entities are struct-of-arrays tables with stable `u32` handles, owned by `World`. Systems are plain functions run in the order of one scheduler table.

An ECS crate (`bevy_ecs`, `hecs`) was rejected for two reasons:
- 88 adventure functions, 18 simulation functions and the factory take `&mut World`, so adopting one is a rewrite of the game layer, not a migration.
- The hot data, the cellular world, lives on the GPU anyway.

Revisit only if a system needs dynamic component composition that a table plus a mode field cannot express.
