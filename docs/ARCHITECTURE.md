# Architecture

## Data flow

```
assets/*.ron ──► engine_content (BlockId table), app tables (spells, recipes, machines, bestiary)
WorldSampleConfig {seed, generator_version, world_type, voxel_scale}
        │
        ▼
engine_worldgen::generate / generate_padded ──► ChunkData / PaddedTerrain (engine_voxel)
        │                                              │
        ▼                                              ▼
app World (edits, regions on disk, water volumes, activity) ──► mesh::GeneratedChunk ──► render (wgpu)
        ▲                                                                          ▲
        │ per tick                                                                 │ per frame
   simulation (cellular, rigid, thermal), factory, adventure, player        egui_ui overlay
```

A `World` holds resident edited chunks plus a `RegionStream` of chunks on disk; any coordinate not edited is sampled from the generator, so the generator must stay a pure function of the config. Edits mark chunks dirty, the streamer re-meshes them on worker threads, and the renderer uploads meshes and draws terrain, entities, clouds, sky and post-processing.

## Tick order (fixed 1/120 s, `simulation::STEP_SECONDS`)

1. `expedition.tick(dt)`
2. `factory::step` then `factory.tick(dt, machine_size)`
3. `update_player` once per step with the held `Input`
4. `adventure::tick` (enemies, projectiles, spells)
5. `WorldSimulation::update` (factory extraction, rigid bodies, cellular water and sand, thermal) under a `Budget`
6. `day_night.advance(dt)`

`main.rs` runs this with wall-clock budgets so frames keep pacing; `headless::Session::tick` runs the same order with `Budget::UNBOUNDED` and waits for chunk generation, which makes a scenario reproducible to the hash.

## Scale and LOD

One world has one `VoxelScalePreset` (1 m, 0.5 m, 0.25 m per cell). Generation is written in metres and converted with `WorldSampleConfig::blocks_for_height`, so the same seed gives the same landscape at every scale. Near chunks are meshed per voxel (`mesh/solid.rs`); farther chunks use `mesh::coarsened_mesh`; beyond the resident ring the renderer draws a horizon backdrop (`render/horizon/`) built from the generator's distant surface plus authoritative edited columns.

## How a voxel change propagates

`World::set` → `ResidentEdits` chunk copy-on-write → water volume map cleared for that cell → activity bits wake the cell and its neighbours (next cellular step reads them) → chunk and halo neighbours marked dirty → streamer rebuilds `GeneratedChunk` → `SolidMesher` → GPU upload → horizon edit overlay refreshed for that column.

## Where things live

| Concept | Path |
|---|---|
| Chunk storage, config, constants | `crates/voxel/src/` |
| Terrain, biomes, structures | `crates/worldgen/src/` (`lib.rs` sampling layer, one module per feature) |
| Block properties | `assets/materials.ron`, `crates/content/src/lib.rs` |
| Cellular, rigid, thermal | `crates/app/src/simulation/` |
| Factory, inventory, recipes | `crates/app/src/world/factory*`, `inventory.rs`, `assets/{machines,recipes}.ron` |
| Spells, enemies, bosses | `crates/app/src/world/adventure/`, `assets/{spells,bestiary}.ron` |
| Save format | `crates/app/src/world/storage.rs`, `regions.rs` |
| Meshing | `crates/app/src/mesh/` |
| Renderer, shaders | `crates/app/src/render/`, `crates/app/src/shaders/`, composition in `shader_sources.rs` |
| UI | `crates/app/src/egui_ui/` |
| Headless scenarios | `crates/app/src/headless.rs`, `assets/scenarios/` |
