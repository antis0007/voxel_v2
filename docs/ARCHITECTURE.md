# Architecture

## Data flow

Revision-4 integration: [Axiom implementation track](parked/2026-10-24-axiom-implementation.md) (parked for M1–M3).
`world/effects.rs` now provides synchronous conditional terrain replacement and
water-buffer exchange for drill/pump/drain consumers. It admits prepared legacy
owners and wake capacity, returns explicit failures or a consumed commit receipt,
and publishes the existing mesh dependency halo only after success. This is not
a global transaction journal, a renderer replacement or a CA authority cutover.

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

Both `headless::Session::tick` and the window now call executable `simulation::fixed::gameplay` and `fixed::finish` for every 120 Hz step, with tool ingress between adventure and simulation. Zero-step frames retain input edges; several steps execute the full sequence individually. Headless settles legacy source jobs; the window shares one bounded simulation budget across its steps. Legacy residency and edit scheduling still depend on wall time, so equivalent execution order does not establish world-hash equivalence. See ADR 0018 for clocks, edge consumption and remaining scheduling debt.

`--headless=<path> --ca-cpu-region=x,y,z` activates the bounded CPU development path: an exact 8³ interior, immutable sealed halo, ordered commands, a 60 Hz divider and tick-labelled two-tick mirror. Player queries use that mirror; unmigrated writers are suspended for the isolated session. Ordinary window play and persistence retain legacy authority. No CA-to-legacy rounding or save migration is performed; ordinary saves reject an active experimental mirror.

## Scale and LOD

One world has one `VoxelScalePreset` (1 m, 0.5 m, 0.25 m per cell). Generation is written in metres and converted with `WorldSampleConfig::blocks_for_height`, so the same seed gives the same landscape at every scale. Near chunks are meshed per voxel (`mesh/solid.rs`); farther chunks use `mesh::coarsened_mesh`; beyond the resident ring the renderer draws a horizon backdrop (`render/horizon/`) built from the generator's distant surface plus authoritative edited columns.

Above the surface tier (`altitude.rs`: transition at 1,500 m, resident load stops at 6,000 m, orbit at 30,000 m, 200 m hysteresis) the orbital globe takes over: `render/globe.rs` drives `render/globe/`, which bakes cube-face quadtree height tiles from worldgen on a worker thread, caches them on disk (`VXGT0002`), and draws them as a sphere with a cloud shell and night lights, discarded under the resident chunk disc. It is on with `VOXEL_GLOBE=1` or `--benchmark-globe`; see `docs/adr/0039-orbital-globe.md`.

## How a voxel change propagates

`World::set` → `ResidentEdits` chunk copy-on-write → water volume map cleared for that cell → activity bits wake the cell and its neighbours (next cellular step reads them) → cell appended to `World::changed_cells` → `World::publish_changes` moves it into the dirty block set and marks its chunk halo → streamer rebuilds `GeneratedChunk` → `SolidMesher` → GPU upload → horizon edit overlay refreshed for that column.

Every chunk writer (`set_many_with`, `commit_resident_water_cell`, `set_replica_cell`) appends to `changed_cells`, so a feature that writes through `World` gets meshing without touching the dirty sets. `fixed::finish` publishes after each step and the window publishes again before it rebuilds dirty chunks, which covers edits made by input handlers on frames without a step. Callers that still mark the sets themselves are redundant and are being removed per module; player edits keep their separate chunk set because it carries fast-lane priority, not correctness.

## Where things live

| Concept | Path |
|---|---|
| Chunk storage, config, constants | `crates/voxel/src/` |
| Terrain, biomes, structures | `crates/worldgen/src/` (`lib.rs` sampling layer, one module per feature) |
| Block properties | `assets/materials.ron`, `crates/content/src/lib.rs` |
| Cellular, rigid, thermal | `crates/app/src/simulation/` |
| Star systems on rails | `crates/space/src/`, `assets/star_systems.ron` |
| Factory, inventory, recipes | `crates/app/src/world/factory*`, `inventory.rs`, `assets/{machines,recipes}.ron` |
| Spells, enemies, bosses | `crates/app/src/world/adventure/`, `assets/{spells,bestiary}.ron` |
| Save format | `crates/app/src/world/storage.rs`, `regions.rs` |
| Meshing | `crates/app/src/mesh/` |
| Renderer, shaders | `crates/app/src/render/`, `crates/app/src/shaders/`, composition in `shader_sources.rs` |
| UI | `crates/app/src/egui_ui/` |
| Headless scenarios | `crates/app/src/headless.rs`, `assets/scenarios/` |
