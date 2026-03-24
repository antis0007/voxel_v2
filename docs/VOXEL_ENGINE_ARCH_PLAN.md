# Voxel Engine Architecture Upgrade Plan: World & Lighting

## 1. Executive Summary
The current engine has made excellent strides in moving towards a AAA-quality rendering pipeline (as evidenced by the transition to clustered rendering passes, cascaded shadows, and runtime atmosphere products). However, the **underlying World and Lighting data architectures** are lagging behind the rendering capabilities. 

Currently, the engine relies on procedural "math soup" for environmental lighting, point-light heuristics for torches, and highly inefficient `HashMap`s for world state. To achieve true AAA voxel engine scalability (e.g., high view distances, multiplayer server authority, and robust voxel lighting), we must refactor how world data is stored and how light propagates through it.

## 2. World Architecture Analysis & Upgrades

### Current Weaknesses
*   **Memory-Inefficient Storage:** `World` stores player modifications in a global `HashMap<IVec3, BlockId>` (`edits`) and a nested map `chunk_edits`. This uses ~32 bytes per modified block and completely ruins CPU cache locality. It prevents efficient serialization, streaming, and large-scale explosions/edits.
*   **Split-Brain Terrain Generation:** Terrain is currently evaluated on the CPU (for collision, raycasting, and tools via `sample_block`/`sample_column`) and generated independently on the GPU via compute shaders (`TerrainGenerator`). This violates the Single Source of Truth (SSOT) principle and risks desyncs between physical and visual terrain.

### Upgrade Path
1.  **Paletted Chunk Storage:** Replace the `HashMap` edit lists with localized `Chunk` structs managed by the `World`. Each `Chunk` should use Paletted Storage (a 1D array of indices pointing to a small palette of `BlockId`s) or Run-Length Encoding (RLE). 
2.  **Authoritative CPU Chunk Generation:** Move all terrain generation to a highly parallelized CPU pipeline (using `rayon`). The GPU compute shader generation is fast, but readbacks to the CPU cause pipeline stalls and latency. By generating on the CPU, the `World` instantly knows about physical collision boundaries, and meshes can be dispatched to the GPU seamlessly.
3.  **Chunk State Machine:** Implement a formal state machine for chunks (`Unloaded` -> `Generating` -> `Lighting` -> `Meshing` -> `Active`).

## 3. Lighting Architecture Analysis & Upgrades

### Current Weaknesses
*   **No Global Voxel Illumination:** There is no concept of propagated voxel light (Sunlight and Blocklight). Torches are currently passed to the GPU as an array of point lights (`MAX_TORCH_LIGHTS = 16`). This does not scale and doesn't allow light to bounce around corners or down caves.
*   **Monolithic Sky Heuristics:** `LightingState::from_time_and_day` is a massive procedural block of magic numbers blending fog, clouds, sun intensity, and sky colors. It is difficult for artists/designers to tweak.

### Upgrade Path
1.  **Flood-Fill Voxel Lighting (Global Illumination):** 
    *   Add `sunlight` (0-15) and `blocklight` (0-15) to the voxel data structure.
    *   Implement a BFS (Breadth-First Search) flood-fill algorithm on the CPU that runs when chunks are generated or blocks are placed/broken.
    *   Pass these light values into the chunk mesh vertices (`Vertex3`). The world shader will use these values to sample a 1D lightmap curve, allowing infinite torches and realistic cave darkness.
2.  **Data-Driven Environment Lighting:** Replace the procedural math in `LightingState` with evaluated splines/LUTs (Look-Up Tables) driven by Sun Elevation and Weather state. 

---

## 4. Implementation Roadmap

To avoid breaking the application entirely, this refactor must be done incrementally:

*   **Phase 1: Voxel Storage Overhaul:** Refactor `World` to own actual `ChunkData` arrays instead of `HashMap` edits. Update CPU getters/setters to read from these arrays.
*   **Phase 2: CPU Light Propagation:** Introduce `LightNode` queues and implement the BFS flood-fill for `blocklight`. Update `ChunkMesh` generation to read local light values and bake them into vertex data.
*   **Phase 3: GPU Lighting Adjustments:** Modify `world_shader.wgsl` to accept voxel light values from vertices, removing the hardcoded 16 point-light loop.
*   **Phase 4: SSOT Terrain Generation:** Deprecate the compute shader terrain generator in favor of a multithreaded CPU generator that writes directly to the new `ChunkData` arrays.

---

## 5. LLM Coding Agent Prompt

*Copy and paste the following prompt to your LLM coding assistant to execute Phase 1 and Phase 2 of the upgrade plan.*

```text
<PERSONA>
You are an expert AAA Engine Developer and Rust systems programmer.
</PERSONA>

<OBJECTIVE>
Execute Phase 1 and Phase 2 of our Voxel Engine Architecture Upgrade Plan without breaking the existing GPU meshing pipeline. We are migrating away from `HashMap`-based block edits to a localized flat-array chunk storage system, and introducing foundational Voxel Light propagation.
</OBJECTIVE>

<INSTRUCTIONS>
Please modify `src/world.rs` and `src/mesh.rs` to implement the following changes:

1. **Create a `ChunkData` Struct (src/world.rs):**
   - Create a `ChunkData` struct that holds a flat array of blocks: `blocks: Box<[BlockId; 32 * 32 * 32]>`.
   - Add a light array: `light: Box<[u8; 32 * 32 * 32]>` (where the upper 4 bits are sunlight, and lower 4 bits are blocklight).
   - Implement indexing helper functions (e.g., `fn local_index(x, y, z) -> usize`).

2. **Refactor `World` Storage (src/world.rs):**
   - Replace `edits: HashMap<IVec3, BlockId>` and `chunk_edits: HashMap<IVec3, HashMap<IVec3, BlockId>>` in the `World` struct with `chunks: HashMap<IVec3, ChunkData>`.
   - Update `World::get` to first check if a `ChunkData` exists for the requested coordinate. If it does, return the block from the array. If not, fallback to the procedural `sample_block` generation.
   - Update `World::set` to instantiate a `ChunkData` if it doesn't exist (populating it via `sample_block`), and then set the specific block in the array.

3. **Establish Light Propagation Foundation (src/world.rs):**
   - Create a basic Breadth-First Search (BFS) flood-fill function `fn propagate_light(world: &mut World, start_pos: IVec3, light_level: u8)`. 
   - When `World::set` places a block that emits light (check `id.is_torch()`), call this propagation function to spread blocklight (decreasing by 1 per block) to adjacent non-solid blocks within the `chunks` map.

4. **Update `WorldSnapshot` and `GeneratedChunk` bridging (src/world.rs & src/mesh.rs):**
   - Update `WorldSnapshot` to safely clone or Arc-reference the new `chunks` map so the background meshing threads can read the explicit chunk data and light data.
   - Update `apply_snapshot_edits_cubic` to read from the new `ChunkData` structures instead of the old `chunk_edits` hash map.

Ensure your code is clean, highly optimized (using bitwise operations for coordinates/lighting where appropriate), and handles edge cases (like crossing chunk boundaries). Output the unified diffs for these changes.
