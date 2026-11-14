# 03 — ART DIRECTION, CONTENT PRODUCTION AND PHYSICAL WORLD GENERATION
**Rev U1 | October 26, 2026.** Visual quality is a production contract. Asset/model availability and claims of actual Blender execution require receipts.

## 1. The canonical visual statement

**"Chrono Trigger / A Link to the Past / polished GBA-era pixel art as a coherent Minecraft-constructible 3D world, with more responsive materials, high-quality lighting and Noita-caliber physical effects."** An imaginary premium 1995–2003 pixel RPG whose artists had access to modern WGPU illumination, dynamic destructible materials, huge caves and physically functioning factories. Keep human-scale readability and grid affordances. Never substitute generic photorealism, generic low-poly, PS1 jitter or a screen-wide pixelation postprocess.

### Visual principles

1. **Silhouette before material:** locations read as terrain masses → buildings → supports → human use → pixel highlights. Entities and wands must have distinct negative-space and shape identities when rendered solid black.
2. **Pixel clustering rather than noise:** textures use coherent shade clusters, selective texture features and restrained dithering where justified. A photo downsampled with nearest-neighbor is not premium pixel art.
3. **Material palette grammar:** intentional hue-shifted shadow/mid/highlight ramps, roughly 3–6 deliberate values per base material before dynamic lighting. Avoid random 16-bit noise and uniform brown/yellow wash. Colors may be extended for emission and weathering.
4. **Physically aligned details:** beams meet columns, roof ridges connect, chimneys have purpose, machine shafts connect to drives, pipes and electrical conductors have typed ports and coherent routes.
5. **Every scale gets attention:** first-person object, medium-distance building, distant silhouette and giant world composition are different views of one design. No hero-detail that vanishes into disconnected far silhouettes.
6. **State-driven effects:** shaders express wetness, heat, charge, pressure, rust, fungal growth, field strain, leakage and damage from actual simulation inputs. Small common objects can be visually quiet.
7. **Restrained contrast and bloom:** warmth in lantern pools, cool cave bounce, distinct industrial heat, magical emission reserved for interesting conditions. The player should see underground geometry without fullbright fog.
8. **Art supports play:** readable collision/head clearance; exact object grip and attack emitter; pickups visible; machine gauges legible; hidden doors discoverable from clues, not invisible pixels.

### Reference provenance

Named Downloads paintings from previous brief: `sylvain-sarrailh-danghostisland.jpg`, `tu-bui-dmz-overall.jpg`, `jung-park-boomtown.jpg`, `Витаутас Лайсонас.jpg`. **Only the last was inspected during previous reachable review**, showing towering pale walls, a tall deep vertical canyon, layered tiny warm roofs and a narrow watercourse; use *monumental mass vs delicate human-scale density* as composition principle. Future agent must open all originals locally before naming specific visual features. No art theft or AI texture lifted from reference illustrations. Anime/SNES references are influences, not sprite copying.

### Pixel scale research

Start controlled studies of ~16, 32 and 48 px/m for world materials, 48–64 px/m for hero equipment, and authored inventory sprite icons at game UI pixel scales. These are *art experiments*, not hardcoded requirements. Lock apparent size by world-space UVs/atlas policy and test under 1080p/1440p gameplay FOV, sloped roofs, distance LOD and mips. Texture filtering should preserve clusters nearby while preventing far shimmering: mip-aware nearest-in-level or appropriately configured blended mips with padded atlases, evaluated visually. Texture density may vary deliberately by material/hero intent, but never look like unrelated artpacks.

## 2. Materials and shaders: one coherent expression system

Build shared WGSL lighting/BRDF and a small number of material-family responses instead of one bespoke shader pipeline per block ID.

| Class | Authored visual clues | Dynamic state | Unique effect bound |
|---|---|---|---|
| Stone/limestone/basalt | Broad strata, designed chips, mortar seams, hue-ramp shadows | Wet, cracked, heated, soot, moss | Thermal fissures, trickle seams, crystalline growth, localized AO |
| Timber/leather/cloth | Directional grain, end-grain, woven clusters, believable stress | Moisture, scorch, char, fracture | Bark/shutter flex, subtle heat scorch; no constant noise |
| Copper/brass/steel | Large deliberate metallic facets, rivets only when mechanically necessary, tarnish | Temperature, voltage, corrosion, oxidation | Broad anisotropic glints, heating hue, conductor pulse near faults |
| Ceramic/glass | Restrained glazed bands, chipped edges, local translucency | Pressure, cracks, heat, contaminants | Gauge fluid meniscus, refraction at meaningful surfaces |
| Water/oil/acid/lava | Volume/depth color, patterned contact foam, directional movement | Level, flow, temperature, sediment | Surface flow, steam, bubbles, precipitation, wet material response |
| Crystals/aether | Internal structured facets, sparse luminous veins, embedded geometry | Flux, resonance, fracture, overcharge | Field distortions and spectral splitting on hero assets |
| Fungus/biology | Grown asymmetric segmentation, gills, soft spore sacs, varied scale | Growth, age, health, spores | Bioluminescent breathing, directed mycelial pulses, decay |
| Rune fields/shields | Geometry-anchored lines, stylized angular glyphs, inactive transparency | Charge, impact, frequency, damage | Local ripples, fracture, interference; no generic always-on hex bubble |
| Atmospheric | Layered fog, dampness, spectral ambient light | Weather, temperature, pressure, toxins | Physically informative fog and wind, restrained volumetric scattering |

**Material parameters** are compact, derived from authoritative world fields where available. Texture IDs, roughness masks, palette lookup, emission mask and dynamic state use shared arrays/bindings; variant count should not multiply GPU pipelines gratuitously. Fix lighting/color-space correctness before creating color-correcting material hacks (particularly the reported underground yellow problem). Ordinary blocks should be cheap to draw; only visible hero effects use higher cost passes.

**Shields inspired by, not copied from, Mycopunk:** public developer notes do not verify accessible proprietary energy-shield source. Implement original material-specific logic using near-invisible rest field, geometry-anchored patterns, fresnel at grazing angles, impact-ring propagation, emitter-directed pulses, localized damage and bounded event buffers. Five aesthetic families: kinetic polygon stress, electromagnetic trace lattice, arcane engraved knot/rune, fungal membrane, faceted crystalline interruption. `shield_state` carries charge, hit positions/ages/energies, topology and failure. All effects tested against world stone, water, glass, player camera and a rapidly changing light rig; surface culling/occlusion and `fwidth` line anti-aliasing are essential.

**Noita-like environmental feedback:** sand granular tumbling, burning wood, acid erosion, condensing steam, droplets, sparks and creature damage have distinctive pixel-cluster particle motifs; nearby particle density adapts without altering physical authority. Effects must make reactions legible, not conceal them. Hit FX should have clear onset/evolution/outcome. Avoid shader "magic" that implies nonexistent conductivity or water pressure.

## 3. Wand art is an ecosystem, not three heavy props

**Reject prior A/B/C prototypes as wands:** Spark-Lantern is a placeable aether lantern; Annular Induction is a ritual machine; Offset Coil is a magical industrial discharge tool. Their nearly identical dark teal/bronze/cyan lantern-like look is not Noita/parity art direction. Reuse shapes in the world only if function fits.

Noita documents a finite pool of **900 random wand graphics and 20 special wands**, with physical hints to stats. Axiom should establish 12 *hand-designed* visual families and ultimately a high-quality constrained generator for many valid combinations, without claiming equivalence from raw permutations.

### 12 authored origins and shapes

| Family | Visual silhouette | Casting/prop identity | Construction language |
|---|---|---|---|
| Witchwood switch | Crooked few twigs, unbalanced thin outline | Rapid crude casting / early wand | Wood, fiber, seed, soot |
| Bifurcated focus | Long forks/open negative space | Multicast, split streams | Hand-carved stave and narrow tips |
| Bone needle | Long needle or barb | Precise damage/curses | Bone, dark ferrule, tiny rune |
| Runic key | Irregular key-tooth/bit rather than rod | Wards, locks, structural manipulations | Flat etched metal or stone |
| Rootbound scepter | Root weave and living asymmetry | Growth, creatures, healing | Organic/chitin/symbionts |
| Cinder rod | Fine dark char with ember cracks | Heat and burning | Blackened grain, glassy fissures |
| Alchemical quill | Long taper with asymmetric nib | Drawing sigils and transforming matter | Thin ceramic and ink channels |
| Forked conductor | Precisely manufactured narrow double prong | Electrical/kinetic | Modest ceramic, copper wire, metal tip |
| Shard cluster | Angular uneven thin splinters | Chaotic elemental behavior | Multiple minerals, restrained bindings |
| Grave reliquary | Hanging charms/voids on slender base | Curse/summon | Iron, bone, translucent ancient wax |
| Astral filament | Sparse outline with suspended tiny focus | Spatial/gravity | Dark frame, seemingly weightless geometry |
| Ancient prism | Strong monolithic tiny shape | Ancient/secret intrinsic property | Founder blackglass/rule-bearing matter |

**Visual lineage constraints:** vernacular witchcraft; scholarly thaumaturgy; industrial magitech; biological fungal; ancient/anomalous. Each has different silhouette and craft grammar. Avoid simply changing material IDs. Smaller normal wands can use 2D sprite/2.5D or intentionally minimal 3D meshes, while hero objects use actual Blender models. Inventory pixel icons are authored/retouched, not automatically accepted from beauty renders.

**Mechanics affect appearance in subtle ways:** deck capacity may affect length; focus/fork may hint multicast; conductive prongs hint electric interactions; overcharged base emits temporary glints; inactive objects should usually be quiet. Spell-installed particle cues and activation signatures distinguish otherwise similar items. Never make every wand a big cyan lantern.

**First-person standard:** model occupies restrained screen area at actual FOV; grip placed on visible hand; named tip transform aligns visible effects with correct camera-targeting projectile semantics. Animation states: idle/walk/sprint, anticipation, release, recovery, sustained channel, switching, instability/damage. Do not add arbitrary camera recoil that ruins precision or accessibility. Compare silhouette at inventory ~48 px display, ground pickup, close inspection and first person. 

**Acceptance of first 6:** at least six visually **unmistakably** distinct wands built/exported/loaded in native WGPU; grayscale silhouettes; 3/4, first-person, neutral/day/torch scene; matching icon and in-world pickup; three distinct spell effects; real emission and damage state; no Blender-only claims. Then scale to 12 authored designs and constrained proc catalog.

## 4. Blender + image + pixel tool workflow (verifiable)

**Source art contract** for each approved interactive asset:

```
<asset_id>/
  source/<asset_id>.blend                # truly editable Blender model
  source/build_<asset_id>.py             # reproducible bpy if script-built
  source/pixel/<asset_id>.aseprite       # or editable Krita/LibreSprite layered source
  textures/albedo.png                   # controlled texel grid
  textures/material_masks.png           # rough/metal/wet etc as needed
  textures/emission.png                 # only if emitted
  export/<asset_id>.<actual_game_format> # RON mesh, glTF via verified converter, etc.
  manifest.ron                          # verified schema: dimensions/ports/states/LODs
  evidence/{neutral,firstperson,distance,active}.png
  report.md                              # tools actually run + native scene hash + defects
```

1. Read reference locally, design card + use cases, actual world dimensions, interaction ports, historical lineage, palette and collision.
2. Draw silhouette/contact sheet (3+ distinct concepts for hero assets); reject weak/overly symmetrical shapes before details.
3. Model in Blender 5 using named components, appropriate transforms and pivots; geometry has purpose. Script repeated parametric families via `bpy`; use visual UI adjustment for artistic quality.
4. Unwrap, align texels, generate controlled limited ramps; paint in Blender/Krita/Aseprite/LibreSprite. Aseprite CLI can batch palette-convert and export spritesheet + JSON; asset-import build must preserve mask channels and mip padding.
5. Bake/capture AO/normal where helpful, then **retouch** into suitable clustered hand-directed pixel art. Do not blindly copy Blender smooth highlights into albedo as high-res shading.
6. Author state→effect mapping. Export with actual importer or validated build-time converter, preserve physical origin/ports/LOD/material IDs.
7. Capture fixed-camera neutral Blender render and **native WGPU** screenshots/video at several distances and illuminations. Rework until shape, lighting and world interaction match.
8. Test collisions, destruction/salvage, edits, save/load, LOD and performance. Only after actual proof can agent say "integrated".

**Computer Use note:** This specification cannot itself operate the user's Windows Blender 5 session. In a computer-use-capable environment, use Blender UI and `bpy` and produce receipts. No environment/tool access → generate reproducible scripts and clearly mark modeling/game screenshots NOT RUN. Keep previous RON mesh/vertex/material pipeline working until a real importer migration is verified.

## 5. Placeable block and entity taxonomy (content as functional affordance)

Use a **family grammar** rather than thousands of disconnected one-off meshes. Examples of primary visual/functional families:

### Architecture (minimum starting kit)

Stone/ashlar, cobble, brick, timber frame, beams/struts, stair/cap/half/quarter, arches, vaults, buttresses, steep gables, dormer, hip/steeple/cap, ridge/eave/valley, shutter/window/mullion, balcony/rail, bridge/gate/portcullis, service grates, pipelines and gutters. Aesthetic and collision fidelity at locally refined scale is necessary especially for roofs. Each piece must work at supported placement rotations and snapped resolutions.

### Settlements and interiors

Bed, cabinet, chest, shelves and books, crate/sack/cask, butcher/kitchen, basin/well, market stall, inn furniture, archive index, cart, street lamp, workbench, hand pump, waterwheel, forge, millstone, chimney, noticeboard, measurement board, movable hoist and lift. Town planning should create parcels and streets, not scatter identical house shells.

### Industry and science

Ore crusher, mill, sieve, sorter, washing/beneficiation machine, charcoal/coke oven, foundry, alloy furnace, heat exchanger, boiler, pump, turbine, generator, motor, battery/capacitor, converter, transformer, rectifier, cable/pipe/shaft networks, tank/valve/filter/meter, inserter/conveyor/splitter, vacuum chamber, distiller, electrolyzer, crystal growth, wafer prep/lithography, microcontroller board and packaging. At least one real input/output and operational state per functional family.

### Magical industry

Mana condenser, resonance resonator, charged crystal, runic projector, glyph writer, wand workbench, shield node, spell logic port, ritual plates, material transmuter, levitation anchor, containment cell, star telescope, observatory, gate lock, spell turret, growth device and arcane reactor. Consistent historical lineages and physical interfaces.

### Ecology and cultivation

Trees with stable canopy LOD, mushrooms with growth stage, fungal root/drip conduits, spore organ, reed/pond life, lichen/moss drapes, vines, magical flowers, cocoon/nest, living sensors, guardian fruiting bodies, biological reactor trough, luminous fungi. Plants are renewable physical resources and habitat interactions, not decorative decals alone.

### Interactive secrets, dungeon devices and creatures

Pressure plates, tuneable resonant chamber, repairable generator, mechanical locks, hidden actuator, movable statue, counterweight, pressure valve, rotatable mirror, siphon gate, trap launcher, rune reader, coded archive, mining hoist, defensive golem, fungal scout, ore-borer, mechanical spider, crystal parasite, dragonfly-like flyer, sentry, repair drone. Every family gets use, failure and repair/scavenging behavior where relevant.

**Priority:** build 20–40 **working visual families** that establish a coherent world, then extend to 150+ useful placements with variants. Do not call 200 differently painted cubes meaningful content parity. All item/part variants have semantic assets/collision and LOD plan.

## 6. Generator 54 → 55: physical and artistic repairs

Previous reported state (NOT independently revalidated here): generator-54 R7 improved six families (castle, town, tower, mine, pit, dungeon library) with R5/R7 screenshots; the later R7 changed mine beams and pit dimensions; build/shader validation passed but traversal/save/perf did not. Remaining problems: coarse 1-meter stepped roofs, scattered town parcels, broken far tree canopy LOD, narrow ring pit, mine fungal clutter and intersecting gallery, uniformly yellow underground stone. Old generators 53 and 54 and saved worlds must be preserved.

### Required semantic `PlacePlan` recipe

```
site context and approach
→ terrain/landscape and primary composition
→ protected major spaces + room/circulation graph
→ load-bearing anchors/foundations
→ walls, roof planes, stair/lift/ventilation/utility geometry
→ physically meaningful materials and source lighting
→ production/quest/secret machine sockets
→ dressing within clearance exclusions
→ navigation/visibility/structural/LOD validator
→ deterministic multiscale voxel stamps + generation metadata
```

**Six families:**

- Castle: defensible gate and approach, courtyard/keep, differentiated rooflines, wall walks and usable stairs, built into terrain.
- Town: main/secondary streets, shared walls, varied heights/roof orientation, distinct workshops and civic services, plaza, credible density and traversal.
- Tower: a memorable silhouette, inhabitable vertical progression, functioning interior mechanisms, unusual architectural lineage.
- Mine: separate large working galleries, supports anchored to real stone, physically plausible hoist/ore routing, clear working headroom; coherent fungi placement, no low beam crossing.
- Pit: wide, dramatic open shaft/chamber and explicit traversal terraces, not just a narrow ring corridor; legible depth and wall lights outside clearances.
- Library: real nave/vault, playable book aisles, galleries/reading pools and important landmark, cooler stone separation with localized warm lighting.

**Version tests:** frozen original R7 capture hash reported in previous doc (verify local file), same seeds/cameras where available; generator 53/54 golden outputs unaffected; new 55 examples under castle/town/tower/mine/pit seed `2738958700`, library seed `424242`, plus at least three other seeds per family and supported voxel levels. Screenshots at neutral/dawn/night and near/medium/far; traverse with *actual player capsule*; save/reload edits; performance budgets. Do not simulate unsupported finest-grid worldgen by adding undestroyable mesh ornaments.

### Performance-sensitive art acceptance

Full-fidelity assets must use LOD, shared material atlases, GPU batching and stable lookup data. Physically animated machine parts can have low-cost skeleton/transform animation; large amounts of live industry need aggregate item rendering. Variation should come from restrained pattern/mask/geometry choice, not costly separate shader pipelines or unique megabyte-sized textures.

## 7. Native scene acceptance: one beautiful playable workshop

Build a **17-asset integrated riverside workshop**: limestone+arched door, dark timber, steep roof, shutter, stair; workbench/shelf/lantern; waterwheel/pump/ore crusher/generator; aether condenser/rune switch/shield node; a proper tree and fungal cluster. At least three real chained interactions: flowing water → wheel/generator (appropriate approximation) → condenser → shield or machine; plus two repair/failure states and one clue to a deeper ruin. If a mechanical system isn't yet available, mark it **visual-only** and do not claim it works.

Pass only when: Blender editable source + game importer; real WGPU captures; 4 lighting contexts; consistent pixel scale; recognizability from 15–25 m and silhouette from 75 m; corrected roof and tree LOD; no yellow underground wash; actual ports/physical interactions; modest GPU cost; saved evidence hashes and reproduction steps. An image generator or Blender-only beauty scene is **not enough**.

## 8. AI/model prompts: build the process, not just a picture

**Master model creation prompt** (prepend to per-asset tasks):

> You are the art director, Blender 5 technical modeler, SNES/GBA-inspired pixel artist and Rust/WGPU integrator for Underkingdom. Read the source art bible, actual renderer materials, supported asset import format and reference images you can truly open. Compose a distinct role/silhouette first. The artwork must be coherent with Minecraft construction grids, readable natural/industrial/arcane lineage and real physical world interaction. Use actual Blender geometry with named pivots and collision, UVs on an intentional texel lattice, hand-directed clustered materials and useful active/failure states. Never fall back on a teal/copper lantern with random bolts, oversized magical heads, sprayed texture noise, excessive bloom or a generic bevelled cube. Produce real editable source + script + atlas + importer manifest + actual native 3D gameplay captures (or mark game integration not run). Compare normal FOV, inventory, world dropped, neutral light and underground light. Iterate when appearance or function fails; do not declare acceptance from a beautiful concept sheet.

**Hero asset prompt (one natural wand / one industrial focus / one ancient artifact):**

> First create a sheet of 18 solid-black shapes spanning at least six material/shape origins, showing true variety. Choose three that remain readable at 48 px and in first-person held pose, with clearly different dominant length-to-width ratios and negative space. Model actual artifacts in Blender: inexpensive irregular living wood, precise slender conductor and rare asymmetrical ancient object. Put meaningful material and casting channels at the tip, not a big glowing battery case. Author inventory icons separately; animate anticipation, release, recovery, and spell-dependent response. Export via existing pipeline and prove in WGPU. Any two assets that look identical when rendered black at inventory scale must be reworked.

**Factory family prompt (early and late industrial):**

> Design 3 related visually legible machines: water-powered crusher, fired-clay refractory furnace, early electrical generator. Their drive, ports, output, stateful gauges, material wear, simple repair behavior and input/output need to be represented physically in game. Match shared machine-body kit without identical box silhouettes. Model in Blender, UV to common atlas, rig useful moving parts, export and prove one functional loop in the existing game. Show the relationship between mechanical input, heat/power and production rate; no progress bar that advances without supplied resources.

**World-site prompt (mine + library + civil factory):**

> Generate a staged, physically navigable PlacePlan. Compose landmark silhouette, protected 3D voids and human circulation before walls and props; root supports in actual terrain; match voxel scales; use temperature-separated lighting; include functional machine anchor and two route alternatives with in-world clues. Fix observed R7 mine low beams/pit ring corridors/yellow wash. Preserve older generator outputs. Capture near/far and player-capsule traversal; reject generic empty or cluttered rooms.

**Shader/material prompt (WGSL):**

> Build one compact shared lighting pipeline with deterministic procedural material masks and state-driven stone, wood, copper/iron, ceramic, water, crystal, fungus and arcane effects. Reference stylized Noita/Chrono Trigger shape grammar rather than copying any game shader. Implement original shield families with emitter-bound ripples and charge/damage; measure GPU cost. Provide neutral benchmark assets, near/far textures, day/night/cave cameras and shader-validation + p95/p99 metrics. Avoid postprocess pixelization, texture shimmer and physically misleading visual states.

## 9. Source tools and trusted implementation references

- Blender 5 Python API: https://docs.blender.org/api/5.0/ and model/UV/texture baking tools corresponding to installed Blender version.
- Aseprite CLI for palette and atlas generation: https://aseprite.com/docs/cli/
- Noita wand visual and stat references: https://noita.wiki.gg/wiki/Wand and https://noita.wiki.gg/wiki/Wand_Appearance
- Mycopunk developer material/shader announcements should be read as technical inspiration only; no proprietary energy shield source is verified available.
- Transvoxel for smooth 2:1 transition where chosen: https://transvoxel.org/
- GDC Noita technical/design talk: https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design/
