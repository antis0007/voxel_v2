# Implementation Prompt: Realistic-Shape Terrain, Atmosphere, Clouds & Flora

## Context

You are working in a Rust + WGPU 3D voxel engine for a **Noita-inspired sci-fi magitech game** with a **variable-scale voxel world** (fine voxels near the player and active simulation, coarse voxels at distance). It also draws on Factorio and modded Minecraft, so expect automation and magic systems. Everything in the world is meant to be a **simulated material**.

The current visuals have these problems:

- **Sky:** a uniform sepia/brown distance fog, with no sun-relative gradient and no horizon definition.
- **Clouds:** 2D-looking, over-warped and smoky. They have no flat condensation bases and no vertical development, the horizon clouds look stamped and repeated, and the lighting is flat.
- **Terrain:** isolated noise-threshold cone mountains on flat plains. There is no erosion, no hydrology, a uniform-width beach, and a single plateau height.
- **Vegetation:** one tree type at one scale, laid down as a uniform carpet, with thin dark trunks.

**Goal:** natural, believable shapes and lighting with an alien palette, running very fast. The alien palette must be intentional and physically coherent.

**Frame budget targets** (1080p, mid-range GPU such as an RTX 3060 or RX 6600):

| Subsystem | Budget |
|---|---|
| Sky + aerial perspective | ≤ 0.3 ms |
| Volumetric clouds | ≤ 1.5 ms |
| Cloud sim update (amortized) | ≤ 0.3 ms |
| Terrain gen/erosion | Async, off the frame; never stalls the frame |

## Ground rules

1. **Read before you write.** Map the existing renderer, the world generation, the chunk/LOD system and the material registry before you change anything. Reuse the existing abstractions. Do not create parallel systems.
2. **Keep the code small, readable and agent-traversable.** Use one concern per module, with modules ideally under about 400 lines. Put a doc comment at the top of each module explaining its role and data flow. Avoid macro magic.
3. **Everything must be deterministic.** World generation must be a pure function of `(seed, region coords, LOD)`. Coarse and fine LODs must agree, so silhouettes never pop on refinement.
4. **Drive tunables from data.** Every visual parameter lives in a RON/TOML config, hot-reloadable in debug builds.
5. **Ship each phase behind a feature toggle and a debug overlay**, using whatever egui or debug UI already exists. Each phase must leave the game runnable.
6. **Measure.** Add GPU timestamp queries per pass (`wgpu::Features::TIMESTAMP_QUERY`, with a fallback when unsupported) and show the timings in the overlay. Report before/after numbers in each phase summary.
7. **Do not add heavy dependencies** without justification. `glam`/`bytemuck` and the existing crates are fine. Noise is generated in your own GPU compute shaders where possible.

---

## Phase 1: Physically based alien atmosphere

Implement Sébastien Hillaire's "A Scalable and Production Ready Sky and Atmosphere Rendering Technique" (2020).

**LUTs, all built in compute shaders:**

| LUT | Size | Format | Contents | Rebuild when |
|---|---|---|---|---|
| Transmittance | 256×64 | rgba16f | Indexed by (view zenith, altitude) | Atmosphere params change |
| Multi-scattering | 32×32 | rgba16f | Isotropic multiple-scattering approximation | Atmosphere params change |
| Sky-view | 192×108 | rgba16f | Latitude-longitude around the camera, non-linear latitude mapping concentrating texels at the horizon | Every frame |
| Aerial-perspective froxels | 32×32×32 | rgba16f | In-scattering (rgb) + transmittance (a) out to about 32 km | Every frame |

**Atmosphere parameters** live in a config struct, so the alien look is pure data:

- planet radius and atmosphere height
- Rayleigh scattering coefficient (RGB) and scale height
- Mie scattering and absorption, scale height, and anisotropy g
- ozone-like absorption layer (RGB, layer center, width)
- sun illuminance (RGB), angular radius and limb darkening
- optional second star or moon light

**Presets:** ship an `earthlike` preset for validation, plus at least one `alien_default` preset (for example, an amber-violet sky with dense Mie haze). The alien preset must still show:

- a sun-relative gradient
- horizon brightening
- a distinct sun disk and halo
- correct darkening at dusk

**Integration:**

- Apply aerial perspective in the main lit pass, or as a full-screen pass, to terrain, water, vegetation and clouds alike.
- **Remove the old fixed-color distance fog entirely.**
- Expose sun direction from a time-of-day system. Create one if none exists.
- Feed ambient sky irradiance to terrain lighting from a tiny SH or a low-resolution cubemap derived from the sky-view LUT.

**Acceptance:**

- The earthlike preset visually matches reference images: a blue zenith, a whitish horizon and an orange sunset.
- A sweep across time of day shows no banding and no NaNs.
- Cost is ≤ 0.3 ms.

---

## Phase 2: Volumetric clouds driven by a simulated vapor field

### 2a. Simulation: coarse vapor grid (CPU or compute, amortized)

**Grid layout:**

- A camera-centered, toroidally scrolling 3D grid covering about 16×16 km horizontally and 0–8 km altitude.
- Cells are 32 m horizontally and 64 m vertically. The resolution is a config value; start around 512×512×128.
- Store **vapor** (u8 or f16) and **condensed water** (u8 or f16) per cell.

**Update:** advance 1/N of the grid per frame, as round-robin slabs.

- **Advection:** semi-Lagrangian, using a wind field from low-frequency curl noise plus a global wind vector that varies with altitude (wind shear).
- **Sources:**
  - evaporation over water surfaces (sampled from terrain/water heightmaps)
  - forest transpiration (small)
  - **gameplay events** through an API such as `vapor_inject(world_pos, amount, temperature)`, used by fire spells, steam vents, explosions and magitech exhausts
- **Condensation:** vapor converts to condensed water above a lift condensation level (LCL), which varies smoothly with the weather map's humidity and temperature. This produces **shared flat cloud bases**.
- **Dissipation:** slow evaporation of condensed water at edges and in dry air.
- **Precipitation hook:** where condensed water density stays above a threshold, emit a rain event so the existing material sim can spawn water voxels below. Keep the rate low and the effect localized.

**Weather map:** a 2D map at about 128 m per texel. The R channel is coverage bias, G is cloud type (0 = stratus, 0.5 = cumulus, 1 = cumulonimbus) and B is precipitation tendency. It is generated from low-frequency noise and modulated by the simulation. It seeds the grid and biases condensation.

### 2b. Rendering: raymarching (after the Nubis / Horizon Zero Dawn approach)

**Density function:**

```
base      = sample(vapor_grid.condensed, p)             // trilinear, simulated macro shape
height_g  = type_height_gradient(cloud_type, h_norm)    // flat bottoms, billowy tops, anvils for Cb
shape     = remap(perlin_worley_128³(p * s0), ...)      // R = Perlin-Worley, GBA = Worley fBm
density   = remap(base * height_g, 1 - shape, 1, 0, 1)
detail    = worley_fbm_32³(p * s1 + curl_offset)        // erode edges only
density   = remap(density, detail * edge_mask, 1, 0, 1)
```

- **Noise textures:** generate the 128³ and 32³ textures once at startup in compute, store them as `rgba8unorm`, and make them tileable. Add a 128² 2D curl-noise texture for detail distortion. **Keep the domain warp subtle**, since the current smoke look comes from overwarping.
- **Erode only near boundaries** so that cores stay dense and tops keep crisp, cauliflower edges.

**Lighting:**

- **Beer–Lambert transmittance with a "powder" term**, meaning darkening at thin edges facing the light.
- **Dual-lobe Henyey–Greenstein phase function**, blended. One lobe has g ≈ 0.8 for the silver lining; the other has g ≈ −0.3.
- **Approximate multiple scattering** with 3 octaves of attenuated extinction and phase, after Wrenninge's approach.
- **Light sampling:** 6 cone samples toward the sun, plus 1 long-distance sample.
- **Ambient:** sky irradiance from Phase 1, with a height-based gradient (darker bases, brighter tops).
- **Sun color:** take it from the transmittance LUT at the sample position.
- **Shadows on terrain:** bake a top-down cloud shadow map (512², updated each frame from the coarse grid only) and sample it in terrain lighting.

**Performance:**

- Render at **quarter resolution** with **temporal reprojection**, updating one pixel per 4×4 block per frame in a Bayer order. Reproject the history using cloud depth (the transmittance-weighted average distance), with a neighborhood clamp to fight ghosting.
- Start each ray with a blue-noise jitter that varies per frame.
- **Adaptive stepping:** take large steps in empty space using the coarse grid as an occupancy hint, then switch to fine steps (about 4–8 m) once density exceeds zero. Step back once on entry.
- Exit early when transmittance falls below 0.01.
- **Use distance LOD:** skip detail noise beyond about 6 km, and lower the step count with distance.
- Use the depth buffer to clip rays against terrain.
- Use a bilateral upsample to full resolution.
- **Composite before aerial perspective is applied**, so the clouds are hazed correctly.

**Acceptance:**

- Cumulus fields show flat, aligned bases and developed tops.
- Mixed cloud types are visible across the sky, and the scale reduces correctly with distance with no stamping.
- Fire spells create visible steam that rises into the cloud deck.
- Cost is ≤ 1.5 ms render plus ≤ 0.3 ms amortized simulation.
- Fast camera rotation shows no visible ghosting.

---

## Phase 3: Terrain macro shape, erosion and hydrology

**Macro fields** (2D, deterministic, sampled at region scale):

- **continentalness:** ocean, coast or inland
- **erosion:** how flat or rugged the area is
- **peaks/valleys:** ridged, folded noise
- **tectonic/ley lines:** sparse, curve-based features (Voronoi edges or spline paths) that magitech spires and ranges cluster along

Map these to base height through **editable splines** in the config, in the style of Minecraft 1.18's terrain shaper. Mountains must emerge as **connected ranges with foothills**, never isolated cones.

**Detail noise:** use derivative-damped fBm (the Inigo Quilez "eroded" fBm, where higher octaves are attenuated by accumulated gradient magnitude) plus ridged multifractal noise for ridges. Apply domain warp sparingly.

**Region erosion pass** (async compute, once per region, cached to disk keyed by seed and region):

- Input is a coarse heightmap at about 16–32 m per cell, with an apron of neighboring cells so seams match.
- Run **GPU hydraulic erosion**, either grid-based (virtual pipes) or particle-based. Particle-based is easier to make seam-safe with a deterministic RNG per region. Follow it with **thermal erosion** using a talus angle that depends on material.
- Compute **flow accumulation** to output river channels, lake basins (by filling depressions), and alluvial deposition.
- **Outputs per cell:** final height, sediment depth, flow amount, wetness and slope.

**Deterministic LOD refinement:**

- Fine voxels equal the coarse eroded surface, interpolated, plus small-amplitude detail noise **bounded so it never changes the coarse silhouette**. Clamp the amplitude per LOD.
- Every LOD reads the same cached erosion result, so distant ranges never pop.
- **Player and spell destruction is a delta on top of the generated world.** Never re-run generation over modified chunks.

**Materials from process, not biome paint:**

| Condition | Material |
|---|---|
| Steep slope | Exposed bedrock and cliff material |
| Gentle slope | Soil |
| High sediment | Sand and gravel in channels, silt and clay in basins |
| Coast, by slope and exposure | Cliffs, beaches of varying width, marsh at low-lying wet edges |
| Near ley lines | Ore and mana-crystal veins along the lines; spires as structured crystal formations |
| Disaster zones (sparse, seeded) | Glassed craters, slag fields, dead riverbeds, magitech ruins |

**Water:** lakes and rivers get their levels from the hydrology output. Water bodies must vary in shape and contain islands. Shallow-to-deep coloring stays, driven by actual depth.

**Acceptance:**

- From altitude, the terrain shows ridgelines, dendritic valleys, rivers flowing to lakes or ocean, and coasts that vary in type.
- There are no seams between regions.
- Flying toward distant mountains causes no silhouette pop.
- Erosion runs async with no frame hitches over 1 ms.

---

## Phase 4: Alien flora as simulated material

**Templates:**

- Make 6–12 voxel structure templates per biome, generated offline or at startup by space colonization or L-systems. Store them as compact voxel blobs with material IDs.
- Templates must include real crowns, trunks of correct thickness and branching. Add alien variants: fungal towers, crystal-growth trees, bioluminescent understory, and so on.
- Store materials per voxel (wood, leaf, resin, crystal) so fire and collapse behave correctly in the existing simulation.

**Placement:**

- Use Poisson-disk sampling per chunk with deterministic seeding.
- Modulate density by fields from Phase 3: slope, wetness, altitude, distance to water, sediment, and biome.
- **Required emergent patterns:** clearings, ragged forest edges, riverside clustering, thinning on ridges, and treeline altitude.
- Apply per-instance random scale (0.6–1.6×), rotation and color jitter.
- Add an understory layer of bushes, rocks, deadfall and glowing flora.

**LOD:**

- Up close, trees are full voxel structures.
- At mid range, merge them into coarse voxels at the chunk's LOD.
- At far range, use **impostor billboards or a canopy heightfield**, color-matched and fed through the same aerial perspective.

**Acceptance:**

- Forests show visible size variation and clearings.
- Fire spreads differently through dense and sparse stands.
- The canopy no longer reads as a uniform carpet from altitude.

---

## Phase 5: Integration and validation

1. Add a **screenshot-diff harness** with fixed seed, camera poses and time of day, for regression checks.
2. Add **perf captures** for the same poses, logging per-pass GPU timings to CSV.
3. Add a **debug overlay** that can show:
   - weather map
   - vapor grid slice
   - cloud occupancy
   - erosion flow and wetness
   - material map
   - per-pass timings
4. Update the docs: write a short `docs/world-visuals.md` describing the data flow from seed to macro fields, erosion, voxels, materials, flora, vapor sim, clouds, and atmosphere.

## Deliverable format per phase

At the end of each phase, report:

- what changed, as files and modules
- before/after timings
- screenshots from the harness poses
- known issues
- the config knobs added

Then stop for review before starting the next phase.

**Order:** Phase 1 → 2 → 3 → 4 → 5. If Phase 3's caching or LOD contract conflicts with the existing chunk system, propose the minimal change to the chunk system first and wait for approval.
