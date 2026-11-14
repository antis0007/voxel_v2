# PROJECT AXIOM — MAGITECH PIXEL ART / BLENDER / WORLDGEN PRODUCTION PACK

**Version:** Art direction and implementation brief 1.0 — 2026-10-26  
**Target:** Rust + WGPU, physical multiresolution voxel survival game, custom asset pipeline (inspect existing RON/mesh representation before changing), generator-54 R7 reported baseline, generator-55 design brief.  
**Purpose:** Give an AI coding/modeling agent reliable, constrained, testable prompts for making *real usable* assets rather than attractive but unintegrated renders.  
**Status:** Proposed design and researched production workflow. These prompts have not produced, rendered, or tested Blender assets in this session.

---

## 0 — provenance and known unknowns

The user reports original art references in Windows Downloads: `sylvain-sarrailh-danghostisland.jpg`, `tu-bui-dmz-overall.jpg`, `jung-park-boomtown.jpg`, `Витаутас Лайсонас.jpg`. Only the last reference was directly inspected here from accessible ChatGPT files: an illustrative tall city/canal composition where enormous pale walls create a narrow negative-space gap and small layered golden-roofed buildings cluster near its top and inner edges. Transfer its *composition* (monumental pale masses, roof density, deep vertical incision, human-scale accents), not an expectation that its painterly brushwork becomes a literal pixel texture. The other three may be available to a Windows Computer Use agent in Downloads; that agent MUST open them before citing detailed characteristics. Do not claim to have inspected them here.

Also inspect generator-54 source notes, generator-55 previous brief, actual existing meshes, custom RON asset path, native screenshots, and native shader code before editing. No accessible source for Mycopunk's proprietary energy-shield shader was verified. Official Mycopunk devlogs DO describe projection work, packed UV biome metadata, procedural grass masks, and GPU effects; independent MIT-licensed shield examples describe Fresnel + hit ripple. Design original energy-shield implementations using these *principles*; do not extract, copy, or misattribute game assets or proprietary code.

## 1 — decisive art direction (style ID: `magi-2000`)

**Art target:** the best imaginary 1995–2003 2D pixel-art RPG reinterpreted as a Minecraft-like 3D world, with 2026-era lighting and interaction. Strongly favor A Link to the Past/Chrono Trigger-era color and shape grammar plus readable Minecraft modular architecture, detailed Noita-like reactions, RimWorld-like functional object communication, and distinctive magitech industrial design. It is *not* photorealism, generic low-poly, voxelized stock models, a PS1 texture-warp simulation, nor a global screen-pixelation post-process.

1. **Minecraft construction grammar:** the primary build measure is the game's familiar module grid; walls and machines fit adjacent grids, doors and stairs read as buildable blocks, machines show input/output faces. Use smaller *localized* voxels and geometry for roof edges, carvings, pipes, gables, mechanical interfaces, stairs and supports. The authoritative world/collider must match the rendered shape.
2. **Pixel grammar:** hand-composed color clusters (not salt-and-pepper noise), identifiable material features, an intentional 3–5-band value structure per surface, selective dithering only in broad gradients and transitions, and a coherent texel-world-size across nearby objects. Rough starting study: 32 px/m for world surfaces, 48–64 px/m for hero equipment. Compare against 16 and 50 px/m before locking. Deviations need artistic justification; don't mix 16 px/m stone with photorealistic 2K buttons.
3. **Geometry grammar:** dominant mass 65–80% of silhouette attention, secondary framing 15–25%, small meaningful accents 5–10% (heuristic, not formula). Thick trim, broad roof planes, clear support connections, expressive proportions, functional pivots. No thousands of bevels/bolts as substitute for good design.
4. **Palette grammar:** coordinated shadow/base/light hue ramps; warm timber and clay, cool stone and muted blue-gray steel, bright but sparing mana/copper/amber/cyan accents. Shared regional color temperatures. Avoid uniform muddy gray, neon outlines on every object, and underground torch-wash that turns everything yellow.
5. **Lighting grammar:** pixel-art albedo but antialiased 3D geometry and physically plausible spatial light; broad shade gradients, restrained materials with deliberate specular and subsurface response. Reserve full emission for energized states. Never require bloom to make an object readable.
6. **Animation grammar:** selectively stepped 10–16 fps-looking sprites for small cosmetic motifs if stylistically valuable, but full-rate simulation of mechanisms, collision, camera, projectiles and fluid movement. Avoid inconsistent jitter from globally quantizing motion.
7. **Worldbuilding grammar:** ordinary medieval buildings use wood, stone, cloth, fired clay; industrial evolution visibly introduces iron, brass, ceramic insulators, glass gauges and cables; advanced electronics introduce precise layered assemblies; arcane instruments use crystalline geometries, etched runes and resonance cages; biology grows asymmetrically. Combined devices visibly disclose which subsystems they combine.
8. **Readable consequence:** a glowing shield means active power, a heat gradient means heat, a pressure gauge means pressure, a wet shiny wall means a wet wall, and an overloaded rune field displays a distinct imminent failure state. No arbitrary decorative animation that lies about gameplay.

### Semantic palettes (working starting points, test in scene)

- `LIMESTONE`: cool cream shadow `#7C807B`, mid `#B9B49C`, sun face `#E8DCC1`, restrained warm rim `#F2E7CF`.
- `OLD_OAK`: shadow `#44392E`, base `#89613C`, lit `#B78650`, worn edge `#D7A972`.
- `COPPER_BRASS`: shadow `#5B4439`, body `#AE7649`, highlight `#E7BA75`, patina `#568E86`.
- `INDUSTRIAL_STEEL`: blue shadow `#354852`, body `#73828B`, highlight `#BAC1B7`, oxide `#85594B`.
- `MOSS_FUNGUS`: shadow `#254B47`, body `#4C8E70`, accent `#B7BD70`, spore `#D8CDA2`.
- `AETHER_CYAN`: base `#175A76`, active `#43B6BD`, core `#C8F2E3`, overcharge accent `#F0C9A7`.
- `EMBER`: deep `#733B38`, body `#C5683E`, high `#F5C274`, maximum core `#F8EAD1`.

Treat these values as color *targets*, not mandatory hardcoded sRGB shader constants; view-space lighting and tone mapping will change their appearance. Author in an understood color space. Verify palette under neutral, daylight, torch, rain/wet, and night lighting.

### Assets must pass four camera tests

- 3/4-view studio neutral-light render; front and rear silhouette.
- 1st-person gameplay ground view with normal FOV, in actual WGPU renderer.
- Town/factory medium-distance scene with 10–40 adjacent assets, no repeated-noise appearance.
- Far scene/L0D with recognizability of dominant mass, coherent foliage and roof silhouettes.

## 2 — production workflow: AI + Blender 5 + 2D pixel tools + in-engine tests

**A. Preproduction.** Read all available refs, crop a consistent contact sheet, classify by shape/color/lighting/use, draft a one-page asset card specifying role, dimensions, player affordance, palette family, at-rest/active/failure states, expected voxel precision. Build a constrained graybox and compare silhouette to 2–3 references before textures.

**B. Blender structure.** Build parametric mesh using `bpy` scripting where repeated forms make sense, then use Blender interface for careful artistic adjustment. Organize named collections: `COLLISION_OR_AUTHORITATIVE`, `VISUAL_L0`, `VISUAL_L1`, `VISUAL_L2`, `PIVOTS`, `PORTS`, `PARTICLES_ATTACH`, `DECALS`, `REFERENCE_ONLY`. Prefer reusable modules and instancing; every rendered functional component has origin, orientation, and owner. Preserve `.blend` with editable layers/collections and a reproducible generator script.

**C. Pixel texture construction.** Use low-resolution painted albedo tiles and layered masks, not raw text-to-image texture as final. Blender's Image Texture node `Closest` displays pixel textures clearly. Start at 32 px/m, adjust to 48–64 on held tools. Set UV seams at pixel boundaries; preserve texel aspect ratio. Create clustered color regions with deliberate fractures, knots, grain, chips and worn edges. Use Blender Pixel Art Studio (if installed and compatible) or paint in Aseprite/LibreSprite/Krita, then import. Aseprite CLI batch exports sprite sheets, indices and atlas JSON. Texture atlases must include filtering/mip seam padding appropriate for actual renderer.

**D. Material maps.** Albedo is authored pixel art. Roughness/metalness/emissive/normal-height masks can be authored at same texel grid or derived and manually corrected. Don't put baked strong directional light into albedo: ambient occlusion only where it clarifies cavities and is carefully managed. For cloth/wood/stone, build major shape with real geometry before inventing micro normal noise. Blender Cycles can bake AO, normal and roughness to texture for source/reference; reduce and repaint to fit palette grammar.

**E. Filtering.** For near pixel surfaces sample nearest within selected mip, test smooth blending between mip levels and mip generation in linear space, with texture atlas padding and anisotropic filtering when necessary. Verify shimmer, crawl, disocclusion, moiré on slopes, distant roof textures, at 1080p and 1440p. Do not assume pure nearest/zero mips yields good distant render. Prefer pixel-space texel grammar rather than snapping final screen pixels.

**F. WGSL material implementation.** Reuse an existing compact set of shader families: stone, wood, metal, ceramic/glass, water/wet, crystal, organic/mycelial, arcane field, energy shield. Components share BRDF/lighting/fog/LOD but override material response: rim light, wet film, heat glow, controlled animated masks, crystals, electrical arc highlight. Expose state from real simulation (temperature, moisture, voltage, flux, pressure, mana, damage, flow).

**G. Engine integration.** Read the real importer: the existing held-item pipeline has reportedly used custom RON meshes and vertex/material data. A `.blend` or `.glb` alone does not count as integration. Implement/export through the supported build-time converter, preserve UVs, origin, pivots, material IDs, part hierarchy, collision shape, rotation rules, scale, animation channels, and state mapping. Register actual placeable/block/item/entity behavior, not just a cosmetic model.

**H. Computer-use quality loop.** Open model and references in Blender, render or viewport capture at multiple distances, check readably posed silhouette, capture texture closeups, adjust model or pixel clusters, export, run native WGPU scene, compare with R7/55 reference images, file reports with exact build hash, camera, material version, texture checksum, and owner. Distinguish use of Blender UI, Blender Python, offline export, and actual in-game rendering in final report.

**I. Asset evidence:** `asset.blend`, `generate_asset.py`, editable paint files, `albedo.png`, `roughness.png`, `emission_mask.png` if needed, LOD geometry data, generated importer manifest (only if actual pipeline supports it), `metadata.json` (size/palette/texel/ports), `preview_neutral.png`, `preview_ingame.png`, and test results. Refuse to call a shader-laden Blender beauty render a tested game asset.

## 3 — format/interaction schema for every placeable

An asset should specify:

- `id`, `family`, `placement_grid`, `bounding_box_m`, `worldgen_allowed_sizes`, `origin`, `rotation_rules`;
- `physical_bricks` or collision mesh tied to actual world occupancy and destruction; `visual_lods` derived from same piece geometry;
- named `ports` (`item`, `fluid`, `gas`, `electrical`, `thermal`, `mechanical`, `signal`, `aether`, `biology`) with direction, capacity, and attachment orientation;
- `states`: `idle`, `working`, `starved`, `damaged`, `overheated`, `overcharged`, `broken`, or family-specific states;
- `interaction`: inspect, open, rotate, connect, maintain, configure, salvage, destroy, automate; don't claim unimplemented affordances;
- `visual response`: parameter→shader/animation link; power→lamps, heat→incandescence, liquid→level, pressure→gauge, mana→glyph displacement;
- `worldgen_sockets` and protected-clearance areas: entrances, stair/headroom, servicing, player collider, vision and puzzle paths;
- `variants`: orientation, temperature/weather patina, repair level, biome material swap, procedural gables/trim, scale variants.

## 4 — multivoxel integration: generation with real geometry

**Essential:** An asset authored for a 1-meter block world must be compatible with localized 0.5 m, 0.25 m and optionally 0.125 m fine cells *where implemented*. These values are examples. The generator must first query supported scales and not pretend unsupported scales exist. Use an integer rational coordinate lattice or compatible world-space representation; never globally allocate at finest scale.

A generation stamp is a semantic assembly, not a coarse voxel stamp with an invisible high-detail model overlay. Example: a gabled roof occupies real roof voxels of locally refined volume, giving appropriate silhouette, colliders, editability, consistent light and LOD. Architecture objects without editable fine-voxel representation need an explicit, documented physical proxy and an interaction policy, not a false promise of destructibility.

Generator can express `PlacePlan`: site/landscape, primary masses, protected voids and circulation, support anchors, port networks and connection requirements, landmark composition, and a library of fine-detail sockets. Stamp from large to small; test collisions and navigation; never let dressing block doors, stairs, mine workings, library nave, pit landing, rails, or hoist.

Preserve generator 53/54 output for existing worlds. Introduce or use generator 55 only if generation rules change. Test all six place families at seed `2738958700` (five) and `424242` (library), plus at least three other deterministic seeds per family and supported voxel scales. Include before/after captures matched to R7.

**Hierarchical LOD:** parent proxy retains dominant rooftop/eave profile, open arch, tower cap and tree trunk/canopy connectivity. Fine decorations may vanish, not landmark silhouette; revisions invalidate parents and children; no appearance of floating roof pieces, disconnected trees, or missing canopies during streaming.

## 5 — model and asset catalog: many more things worth placing

Don't simply add 200 block IDs with one cube material. Make 25–40 high-quality parametric families, each yielding functional, aesthetically controlled variants, creating 150+ distinct usable placements without 150 unrelated Blender modeling jobs. Suggested families:

**Architecture (35+ useful pieces):** pale-limestone courses; brick/cobble/ashlar overlays; half/quarter/thin wall detail; beam and timber braces; thick timber frames; gables, hips, steep roof slope, eave/ridge/dormer pieces; doors, shutters, mullion windows and stained glass; half arches, pointed arches, vaults, stairs, steps, ramps, balustrades; balconies, buttresses, colonnades; bridges, battlements, crenellations; roof gutters, water spouts, chimneys, grates; cellar hatches, lifts and railings.

**Settlement/interiors (30+):** workbenches, bookshelf modules, modular shelves with actual aisles, desks, benches, beds, chests, cabinets, crates, barrels, casks, sacks, market stalls and awnings, signage, cooking hearth, forge, simple water wheel, millstone, fountains, wells, notice board, map table, lantern sets, polished archives, library indexing shelves, secret rotating bookcase, small pulley and hoist.

**Science/industrial (35+):** pulverizer, crusher, washing drum, ore sieve, centrifuge, furnace, coke oven, smelter, alloy mixer, heat exchanger, turbine, generator, battery, capacitor, transformer, rectifier, induction heater, conveyor, hopper, sorting arm, multiblock tank, intake valve, glass sight-pipe, gauge, fluid pump, pressure vessel, distiller, electrolyzer, centrifuge, fabrication cabinet, sealed chamber, lithography machine (nested engineering interface), wire spools, cable/rail conduits, locomotive/hoist fixture.

**Arcane machinery (25+):** aether condenser, glyph engraver, crystal resonator, wand press, mana turbine, rune projector, kinetic anchor, arcane shield node, field focus, spell logic block, transmutation crucible, levitation plinth, alchemy still, star chart, observatory optical mount, stabilized portal rim, alchemical fluid reservoir, crystal growth rack, elemental containment cage, summoning circle plates, spellcasting automaton sockets.

**Ecology/biology (20+):** dense foliage clusters (coherent canopy), moss drapes, luminescent caps, ordinary mushrooms, spore lungs, fungal root conduits, sprouting planters, bio-reactor trough, larval cocoon, glowing reeds, flowering plants, woven vine cabling, skeletal growth structures, mutated vines, biological sensor stalks, fungal containment beds, harvestable plant growth stages.

**Interactive secrets (20+):** puzzle levers, movable statues, encoded rune panels, hidden latch walls, rotating rings, repairable machine interfaces, sealed compartments, pressure plates, weight-sensitive floors, telescopes, mineral resonators, fuses, fluid floodgates, timed valve assemblies, turnable mirrors, hanging counterweights, trap dispensers, breakable support beams.

**Enemies/entities:** deliberately few functional families with hand-authored silhouettes and modular anatomy: cave miner automaton, fungal guardian, power-draining parasite, small walking ore sorter, levitating rune sentry, mechanical spider, amphibious cave creature; salvageable modules have visual and physical identity. Start with 2–3 exceptional models before scaling counts.

Every functional family must include at least one meaningful activity beyond placement/decoration (process, convey, open, heat, cool, sense, shield, grow, actuate, connect, illuminate, inspect, repair, salvage, or puzzle). Decorative variants remain valid but must be clearly categorized.

## 6 — material/shader look-development plan

### Base material layer
- `Stone`: stable albedo clusters, rounded chipped edges, subtle strata/occlusion at cracks, wet-darkening when moist, physically coherent dust accumulation, low glitter. No random speckle.
- `Wood`: coarse long directional grains and knots; physically exposed end-grain, restrained varnish glint, scorch-to-char transition, cracks only under damage.
- `Copper/iron/steel`: broad metallic highlights, hand-painted panel boundaries, oxide/patina masks, temperature response and physically present bolts at silhouette-relevant scale.
- `Ceramic/glass`: stable glazes, chipped dielectric edge, tinted transmission in select windows, useful reflections without generic glowing outlines.
- `Water`: surface height/normal flow, ambient/reflection, tinted absorption with depth, pixel-style foam borders as sheets/sprites at contact, wet material response.
- `Crystal/aether`: internally structured color ramps, anisotropic/split highlights for select hero assets, emissive cores tied to flux with restrained halo and low-frequency pulse, rune flow if active.
- `Fungus/bio`: soft shape transitions, segmented growth bands, cellular pigmentation, faint subsurface response, spore sacs/particles, movement linked to life activity.

### Shader architecture
Stable base-lighting pipeline, compact `MaterialClass` and mask data, consistent world-space UV mapping and texel density. Add world-dependent overlays (water film, soot, frost, moss, corrosion, arcane residue) composed by material properties, not one shader per every possible pair. Weather and temperature modify masks/parameters, not entire albedo textures per state. Group variant instances for draw batching. Small differences should be material inputs, not separate shader pipelines.

**Effects budget:** most placed blocks use inexpensive static shader with optional detail. Hero machines add 1–2 dynamics when active. Magic and boss effects may use additional passes and particles, with distance/readability/performance tiers. Do not make every material literally expensive; make almost every material *distinctly responsive*.

### Energy shields: original visual families, not a proprietary shader copy

The public GodotShaders example uses rim/Fresnel + impact ripple (MIT for code). Take the general technique and expand:

- Physical runtime shield state: `energy_capacity`, `charge`, `heat`, `damage`, `impact_events[]`, `field_topology`, `polarity`, `source_ports`.
- Surface base: very faint field or nearly invisible. Grazing silhouette visible by Fresnel, but *avoid* uniform glowing ball; show woven glyph facets, segmented geometric boundaries and localized flowing energy near conduits.
- Event: hit at world/local position transfers energy, spawns two temporally separated concentric response ripples with decay, geometry-anchored brightness, slight surface displacement, and traveling pulse toward nearest emitter/anchor.
- Depth: intersection soft-fade; shield should visually occupy geometry, not appear always through walls. Respect occlusion.
- Alias protection: fwidth-smoothed lines, temporal stability, budgeted ripple list and no per-hit mesh creation; stable local-coordinate pattern to avoid screen-space crawl.
- `kinetic shield`: broad tiled stress facets, deflect impulse, visible stress fractures and physical support connections.
- `electromagnetic shield`: sparse conductive trace lattice, arcs following paths and polarity, reactive to metal/water/electric impacts.
- `arcane ward`: discontinuous runes and knot patterns, color bands tied to resonance, reaction to spell frequency and magical overload.
- `biological shield`: shifting mycelial membranes and fluid surfaces, repair/growth and vulnerability to heat/toxic chemistry.
- `crystal barrier`: faceted light fracture and expanding angular interference pattern. Avoid generic always-on hex grid.

State combinatorics: kinetic heat impacts make local hot facets; wet+electric cause brighter branching arcs near conductive intersections; aether charge changes rune displacement frequency; physical damage creates persistent fracture mask; shield coverage can be partial or shaped by emitters. The visual effect must correspond to mechanics. Don't require dynamic mesh rebuilding for routine hits.

Pseudo-formula (design only, not copied code):

```
view_facing = pow(saturate(1 - dot(normal_world, view_direction)), rim_exponent);
for each bounded recent hit:
    distance = length(local_surface_pos - hit.local_pos);
    shell = 1 - smoothstep(0, width, abs(distance - propagation_speed * hit.age));
    amplitude += shell * exp(-decay * hit.age) * hit.energy_norm;
pattern = sample_crystal_lattice(local_surface_pos, class, charge, polarity);
emission = class_color * (rim_gain * view_facing + hit_gain * amplitude + sparse_pattern_gain * pattern);
opacity = clamp(base_transmission + reaction_opacity, 0, opacity_cap);
```

Test multiple impacts in one frame, close camera intersections, overlap with water/stone/glass, different projected sizes, and a shield in dark and sunlight. Must look like part of the same SNES-inspired universe, not an imported sci-fi shooter VFX pack.

## 7 — ready-to-paste prompting framework

For every individual prompt below, prepend the following MASTER INSTRUCTIONS unchanged. Output actual files and a truthful provenance/verification report, not conceptual prose only.

### MASTER PROMPT — art production agent / Blender / WGPU integrator

> You are the principal technical artist, pixel artist, Blender 5 procedural modeler, and Rust/WGPU asset-integration engineer for a persistent multiscale voxel magitech survival game. Visual influences are Chrono Trigger SNES, A Link to the Past SNES, GBA-era high-end spritework, Minecraft's friendly modular block grammar, RimWorld's readable industrial forms, and Noita's reaction-rich magic. Our four external architectural paintings in Downloads are composition references; inspect them before use and report which are actually accessible. The accessible Витаутас Лайсонас artwork depicts monumental pale walls and a narrow vertical city/canal chasm with warm dense roof clusters. Aim for an original cohesive visual design, not copies. Read existing generator-54/55 docs and native screenshots; inspect real Blender + Rust/WGPU import path before writing. Our art uses hand-composed pixel clusters, an initial test of 32 px/m terrain or 48–64 px/m hero assets, strong silhouettes, controlled palette ramps, readable construction, and physically grounded magical energy. Do not use generic procedural speckle, high-frequency photographic textures, gratuitous bloom, fake high-detail noninteractive overlays, or global low-res postprocessing. Use real named Blender objects and manageable parametric geometry; preserve part pivots, state channels, physical placement footprint, interactions, and material IDs. Make a graybox first, inspect from 1st-person and 3/4 view, then produce painted/baked editable textures. If direct Blender UI/Computer Use is unavailable, provide reproducible `bpy` scripts and explicitly state that native Blender and game renders are unverified. Every deliverable includes `.blend` + generator script + painted source + atlas/masks + geometry/LOD + engine import if available + screenshots of real output + QA report. Avoid editing concurrently owned shader files. Do not claim success based on a build or Blender beauty render alone. Iterate harshly if silhouette, color, texel density, material hierarchy, or game visibility fails.

### PROMPT A — medieval/fantasy architectural kit and roofs

> Build a modular 1 m-compatible architecture kit for castles, towns, towers and libraries. Make 12 deliberately designed interchangeable roof/structure archetypes: steep gable, asymmetric gable, hip, half-hip, conical turret cap, stepped timber cap, overhanging eave, ridge/valley, dormer, buttressed arch, two-storey timber bay, balcony/support. Emphasize long uninterrupted roof planes (not coarse one-meter stairs), plausible support beams and inhabited spaces, restrained clay-red/slate/ochre tiles with pixel-art roofing patterns, pale limestone and dark timber contrast. Use locally refined voxel/collision geometry for roof silhouettes if engine supports it; otherwise implement smallest truthful semantic proxy. Each piece must be rotatable/snappable, join seamlessly, use consistent texels and contain material/weather variants. Make three facade compositions: village workshop, two-storey townhouse, tall castle tower; include entrance, windows, usable stairs, human scale. Mandatory: before/after Generator 54, near/far roof LOD and regular gameplay view, no pale-wall/yellow-roof wash, no floating beams.

### PROMPT B — practical industrial machine family

> Produce a family of three industrial machines sharing genuine mechanical design DNA: ore crusher (2×2×2 modules), thermal foundry (3×2×3), fluid condenser/pump (2×2×3). Use late-1990s illustrated pixel-art metalwork: heavy cast housings, exposed cog/shaft features at meaningful size, ceramic insulators, glass sight gauges, readable metal plates, warm copper/steel/oxide colors. Model actual ports for inputs/outputs/power/fluid/signal, accessible maintenance panels, stateful gauges, moving shafts/belts, heat/steam/leak/overload effects. No arbitrary gears attached to surfaces. Capture idle, operating, power-starved, overheated and broken states. Implement Blender parametric components and texture atlas shared across machines, plus WGPU render state parameters and a physical placement manifest. Ensure machines form an attractive adjacent factory room and can be connected to conveyors/cables/pipes without overlaps.

### PROMPT C — arcane aether condenser (hero reference asset)

> Build one exquisite, actually usable `Aether Condenser`: footprint 2×2×3 m or grid-compatible equivalent, with a 3D readable ancient brass/stone base, four ceramic/glyph-bearing supports, crystal containment core, restrained engraved conductor paths, mechanically plausible inlet and outlet couplings, movable shutters/rings, and a 32-px/m stone/brass palette with selective higher-density glyph atlas. Integrate 5 states: unpowered (mostly matte), low flux (small inner flow), normal (crystal circulation and tiny glyph points), overload (uneven high-energy arcing, stress visible on ceramic), failure (broken supports/debris). The magical flow uses energy and heat from actual simulation. Deliver 8-angle turnaround, 1st-person closeup, 10 m and 50 m silhouettes, night/dark/torch scenes, full `.blend`, painted tile sources, importable assets and code for state binding. Reject any unmotivated rainbow ring, thick neon contour, over-detailed PBR surfaces, or inaccessible core.

### PROMPT D — medieval science laboratory and electronics family

> Build a cohesive kit that advances from alchemy through laboratory electricity to primitive semiconductors: desk alchemy still, glass-tube apparatus, handwound induction coil, ceramic/wood bench multimeter, adjustable transformer, insulated battery cabinet, wafer furnace, optical exposure instrument, precision etching station. Models must exhibit understandable flow of glass tubing, gas, heat, electricity and signal. All equipment adheres to a common pixel-art texel scale, limited paletted materials, consistent port colors/shapes and visible operational state. Each item provides useful inspection/control affordances and can function with shared industrial pipe/cable systems. Player's microchip layout design itself belongs to a dedicated nested detail interface; DO NOT model billion-transistor polygons in the world. Produce an evolvable asset family, not nine disconnected concept arts.

### PROMPT E — forest, fungal ecology and tree LOD kit

> Create a hand-directed botanical/biological kit: 3 trunk types, 4 canopy shapes, moss clusters, 5 mushroom morphologies, rooted mineral fungus, luminous spore stalks, hanging vines, crystal-root symbiont, and juvenile/mature/decayed variants. Use chunky organic silhouettes, grouped leaves with 2–4 distinct value bands, no confetti leaf noise, no generic emissive green. Palette coordinated with SNES fantasy environments. Generate semantically coherent trunk/canopy attachments, correct near/medium/far LOD including 1-m voxel review worlds, no disconnected canopies. Leaves/fronds can use intersecting handmade cards or grouped geo where performance benefits; keep recognizable physical form. Mycelial growth has states tied to hydration/nutrition/mana and actual harvestable items. Test under forest sunlight, cool cave, neutral and torch light and in groups to reveal tiling.

### PROMPT F — dungeon place kit: library + cistern + mine + pit

> Make a shared but role-differentiated dungeon architecture set: vaulted pale-stone library nave, narrow services and grand gallery; cistern with visible water paths and sluice gates; mine with raised working gallery, supported ore face, hoist and rail; monumental vertical pit with **actual large chambers and usable landings**, not a ring corridor. Model wall/roof/arch/column/support families, reachable doors, bridges, shelves, vault spans, ladder/stairs, wall lamps and one focal mechanical apparatus per place. Reserve negative space before detail. Choose cool stone shadows and small warm pools of firelight; refuse uniform yellow tint. Provide place-layout semantic anchors, player swept-collider clearance tests, at least 3 seed variants each, multi-scale stamping where supported, and in-engine ground-camera evidence. No clutter in primary view corridor, no floating columns or low mine beams intersecting navigation.

### PROMPT G — original magitech energy-shield shader and object family

> Implement a new WGPU/WGSL material family for kinetic, electromagnetic, crystal and arcane field shields. Do not pretend to have Mycopunk's proprietary shield shader; official Mycopunk technical devlogs and open MIT-licensed reference examples inform general techniques only. Create actual shield emitter models with readable projectors, finite energy storage, connectable ports and inspectable condition. Implement state-driven Fresnel rim, anchored sparse pixel-art glyph/facet masks, impact-local concentric energy propagation, physical energy depletion and asymmetric overload fractures. Avoid always-on opaque generic hex bubbles. Patterns must remain anchored to shield geometry, antialiased with derivatives when needed, and must not crawl at distance. Expose host parameters: field strength, temperature, charge, polarity, impact ring buffer, damage mask. Include wet/electric/thermal/arcane combinations grounded in gameplay physics. Add impact stress tests (64 impacts, scrolling camera, overlapping shields, camera inside shield, waterfalls, cave torches) and p99 GPU time. Provide WGSL code, a minimal example scene, and screenshots at multiple lights/distance; preserve fallback when high-end shader is disabled.

### PROMPT H — pixel-material atlas authored as *pixel art*, not AI noise

> Produce a reusable 16–24 tile material palette sheet for limestone, sandstone, shale, mossy stone, oak plank/end grain, roof clay, roof slate, copper/oxidized copper, iron, steel, ceramic, marble, glass, crystal, fungal flesh, spore cap, alchemical residue, and engraved rune plates. Choose working 32 px/m and integer-aligned UV grids; deliver 32×32 or 64×64 tile sources only as justified by intended world scale. Each tile must be seamless where repeating is expected, contain intentional connected color clusters, have 3–5 value bands, and remain distinctive from other tiles when displayed in 8×8 tiled wall. Generate scratch/base motifs with scripts if helpful, then hand-correct/AI-assisted edit, *never raw noise as final art*. Write roughness/normal/emissive/damage masks sharing texels and material properties. Build a visual atlas sheet, tile-repeat test and neutral-light/in-game scene. Ensure no atlas bleeding or mip sparkles. Outputs include layered source, atlas PNG, JSON metadata, sample shader mapping, and test screenshots.

### PROMPT I — an animated small-world object pack

> Create 12 small placeable interactive objects that actually enrich exploration: wheeled cart, hand pump, waterwheel governor, mine hoist pulley, retractable grate, functional wall gauge, rune key switch, hidden door actuator, alchemical reagent rack, archive indexing device, mechanical telescope, gas-vent damper. Prioritize distinctive silhouette, tactile interaction, believable attach/rotation pivots, state changes, feedback and repair/salvage consequences. Each must fit grid semantics, share materials and textures, and have deterministic functional data. At least 4 interact with existing world mechanics right now; if systems are unimplemented, export honest *future socket* metadata, not fake functionality. Native scene: place objects in castle, town, tower, mine, pit and library; do they make the environment meaningfully more inhabitable and readable without creating clutter?

### PROMPT J — multiscale generator/render/integration verification

> Audit and extend the actual procedural placement path so each place family can select coarse world voxels plus localized 1/2, 1/4 and 1/8 detail where the backend supports it. Do not silently render phantom physical geometry. Build an explicit `PlacePlan` structure with semantic solids, protected voids, circulation, support roots, interaction sockets, style materials and LOD-critical silhouettes. Implement region stamps that preserve deterministic versioning, world-space integer alignment, edit/save/load fidelity and collision. Use real generator selection: 53/54 unchanged, new 55 if altered. Test castle/town/tower/mine/pit at seed 2738958700 and library 424242, plus secondary seeds and world scales. Capture native 1920×1080 & 1440p day/night close/mid/far matched cameras. Report per-family silhouette/navigation/material/affordance quality, mesh/atlas memory and p95/p99 time. No shader files owned by parallel agents should be overwritten. Stop if unsupported resolution causes geometry mismatch; report exact missing primitive and implement smallest safe enabler.

## 8 — initial vertical slice: 17 coherent assets, 8 shaders, 1 scene

Produce one *complete* playable “riverside magitech workshop beside ruined library” scene before expanding to hundreds of objects.

- World: limestone wall+arched door, dark timber beam, steep clay roof section, window/shutter, stone stair/railing (5).
- Interior: workbench, shelf, wall lantern (3).
- Industry: waterwheel, pump, ore crusher, small electrical generator (4).
- Arcane: aether condenser, rune switch, crystal shield node (3).
- Ecology: one tree with viable near/far crown; one fungus with growth state (2).

Materials: `stone`, `wood`, `roof`, `metal`, `ceramic/glass`, `water/wet`, `fungus`, `aether/shield`. Modules should share atlases and lighting. The scene must demonstrate 3 physical interactions (e.g. water drives generator; generator powers condenser; condenser powers shield), 2 repair/failure states and one hidden physical clue. If an actual gameplay system isn't implemented, report it as visual-only and do not claim full interaction.

**Quality gates (binary):**
1. Images in studio and engine genuinely depict same materials/asset, not only concept images.
2. Roof and tree correct in near/mid/far LOD; roofs editable/collidable per engine's honest representation.
3. At least one model unmistakably recognizable at 15–25 m and distinguishable at 75 m.
4. Materials remain separated under neutral, day, torch and evening light; underground stone not uniformly yellow.
5. Pixel densities within documented range, no stretched UVs, no random texture salt, no shimmering in motion.
6. Collision and ports align, object interaction and worldgen sockets valid.
7. No regression to old generator versions, existing saves preserved.
8. Deterministic native captures and end-to-end latency/VRAM checks accompany visuals.
9. Failed or unverified gates are explicitly reported, not hidden by beauty shots.

## 9 — source notes, for research and verification

- Blender 5.0 shader Image Texture `Closest`: https://docs.blender.org/manual/en/5.0/render/shader_nodes/textures/image.html (other localized copies also available).
- Blender 5.0 Cycles baking AO, normals, roughness: https://docs.blender.org/manual/en/5.0/render/cycles/baking.html
- Blender 5 Python: https://docs.blender.org/api/5.0/
- Blender Pixel Art Studio (updated Oct 2026): https://alfredbaudisch.com/projects/software/pixel-art-studio/ and https://pardalltools.github.io/pixel-art-studio-docs/pixel-density.html
- Aseprite official CLI, sprite sheets and atlas export: https://www.aseprite.org/docs/cli/
- Minecraft Texture Update (Jappa reference): https://minecraft.wiki/w/Texture_Update
- Mycopunk developer announcements: https://steamcommunity.com/app/3247750/announcements/ (July 10 vertex shader projection, May 15 biome-shader/UV packing, GPU grass work). These do not provide public shield implementation code.
- Open reusable shield technique with MIT licensing: https://godotshaders.com/shader/energy-shield-with-impact-effect/ (the code license excludes associated images/assets; work above is a distinct design).
- Prior user-generated generator 55 brief: `/mnt/data/Generator_55_Places_Quality_Overhaul.md` if present in an accessible environment.

**Final reminder:** beautiful screenshots are necessary but insufficient. Our objective is an expressive and technically truthful world in which 1990s-style pixel-art textures, advanced lighting, procedural buildings, physical machine function, player modification, and multiscale voxel simulation form a single authored aesthetic. The production process is evidence-driven: create, inspect, criticize, revise, integrate, then benchmark.
