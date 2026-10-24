---
name: voxel
description: Chunks, world generation, regions and saves — engine_voxel, engine_worldgen and the app's World hub; generator-version rules and how to test generation.
---

# Voxel world

Where: `crates/voxel/src/` (`ChunkData` 32³ blocks + light + torches, `WorldSampleConfig`, `WorldType`, height constants), `crates/worldgen/src/` (`lib.rs` = climate, columns, trees, caves, `sample_block`, `generate`, `generate_padded`; one module per feature: `landscape`, `sites`, `dungeon`, `valley`, `surface`, `terrain_variety`, `zones`, `undercrown`…), app side `crates/app/src/world.rs` (`World`: `get`/`set`, snapshots, day cycle), `world/regions.rs` (append-only 8×8×8-chunk region files), `world/storage.rs` (quicksave, `SAVE_VERSION`), `world/resident.rs`, `world/activity.rs`.

Bricks: `crates/sim/src/` (`engine_sim`) stores the cellular world as 8³ bricks inside 32³ chunks with a `u64` dirty mask per chunk; `crates/voxel/src/bricks.rs` converts `ChunkData` to and from it. Nothing reads it in the game yet (Phase A3+, `docs/PLAN2.md`).

Coordinates: voxel `IVec3`, chunk `IVec3` = `floor_div(voxel, 32)`, metres = voxel × `VoxelScalePreset::meters()`. Generation code is written in metres and converted with `cfg.blocks_for_height` so every scale shares a landscape.

Generator rule: `CURRENT_GENERATOR_VERSION` (35) is what new worlds use; every historical gate (`cfg.generator_version >= N`) must stay literal because old saves regenerate unedited chunks from their own version. A test pins a bit-exact golden for generation (`gen_golden.rs`); `generation_bench` tests in `worldgen/src/lib.rs` compare generator versions.

Read path: `World::get` → resident edit → region file → `sample_cache.get(cfg, p)` (pure generation). Write path: `World::set` → copy-on-write chunk → water map cleared → neighbours woken → dirty marks.

Test: `just test worldgen` (generator tests use `test_world::World`, a cfg-only stand-in), `just test regions`, `just test storage`. Headless scenarios pin whole-world hashes.

Pitfalls: `World` still owns physics, factory and adventure state, which is why those are not separate crates yet (`docs/PLAN.md`). `suggested_spawn` stays in the app because it needs the player collision box.
