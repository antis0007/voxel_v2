# Surface visual pass: first-person quality on planet surfaces

Status: plan, 2026-11-12. Owner request: "full art pass ... VISUAL QUALITY ON
THE SURFACE IN FPS", with ocean and biome-dependent particles and ambience.
This consolidates the open items of `2026-11-03-surface-rendering-review-triage.md`,
`2026-11-03-first-person-and-lighting-critique-triage.md` and
`2026-10-27-visual-audit-triage.md` with the owner's 2026-11-12 screenshots.

## Constraints

- Authored direction stays: pixel-palette motes (one native screen pixel, no
  soft halos), smooth continuous shading, no posterization, balanced exposure.
- Every look change ships with a before/after capture from the same pose and
  time (`visual-changes-need-ab-captures`). Compiling is not evidence.
- Worldgen-visible changes (block covers, flora placement) need a generator
  gate; shader and presentation changes do not.
- No new proper nouns or lore without approval (UI_RULES).

## What already exists (start here, do not rebuild)

| Area | Where | Notes |
| --- | --- | --- |
| Biome ambience | `crates/app/src/world/ambience.rs` | Per-biome motes (firefly, pollen, frost, ember), drift (petal, spore, spray, dust), tint, mist; blended over five biome samples; dev-panel tunable |
| Mote renderer | `shaders/mood_shared.wgsl` `mood_ambience` | Eleven kinds, one lattice walk, wind-carried, pixel-exact |
| Surf | `world_shader.wgsl` `water_breaker*` | Breaker crest displacement and plunge on shallow coasts |
| Water ripples | `world_shader.wgsl` (current ripples) | Advected by mesher-encoded current; 2x speed since 2026-11-12 |
| Audio beds | `soundscape/ambience.rs` | Wind, rain, insects, water rings (outer ring = surf), cave drone |
| Clouds and shadows | `world/weather.rs`, `render/globe/shell_sim.rs` | Strobe fixed 2026-11-12 (front crossfade, slower solver) |
| Grass / flora | `mesh/` blade clumps, `worldgen/flora.rs` | Per-m2 thinning below 1 m voxels on gen 290 |
| Review captures | `tools/Review-*.ps1`, `--review-*` flags, `audit-views.ps1` | Biome poses in memory `biome-capture-poses` |
| Live tuning | `tools/axiom-dev` workbench, `dev_panel/engine.rs` | `ambience` tuning, material studio, probes |

## Owner findings 2026-11-12 (first person, 1 m and sub-metre voxels)

1. Contour-like dark lines across grass: one-voxel terrace step edges read as
   drawn lines (edge AO / face shading on single steps) plus rock exposed on
   short risers (fixed on gen 290: slope 0.75).
2. Grey gravel bands and blobs near rivers and on beaches (fixed on gen 290).
3. Too little sand on shores (gen 290 sands gentle shores -2..+4 m).
4. Rivers read as slow (ripple speed doubled; the cell simulation itself is unchanged).
5. Floaty gravity on planets (fixed: movement gravity near the ground).
6. Clouds pulse; cloud shadows strobe (fixed, needs capture).
7. Many tiny single-voxel plants below 1 m (thinned on gen 290; models still
   one voxel).
8. "Looks realistic from above, suffers at first person": the near field lacks
   mid-scale detail between the voxel face and the 20 m landform.

## Inspiration (techniques to borrow, never assets or looks wholesale)

| Reference | Technique worth borrowing | Applies to |
| --- | --- | --- |
| Valheim | Low-detail surfaces carried by strong atmospheric light, fog and colour; dense near-camera grass that thins with distance; shoreline foam and drifting spray | Near-field grass density, coast |
| Zelda: BotW/TotK | Visible wind gusts as travelling waves across grass, shared by grass, particles and sound; restrained pollen/firefly motes | Gust field (P3) |
| Ghost of Tsushima | Wind-carried leaf and petal streams as a readable motion layer; grass bending as one field | P3, P4 |
| Vintage Story | 1 m voxel terrain made readable by grass/soil lip overhangs, climate tinting and falling leaves; stepped terrain without outlines | P2 step edges |
| Minecraft shader packs (Complementary, BSL) | Waving plants, wet-surface darkening after rain, puddles, soft contact shading on block edges | P2, P6 |
| Sea of Thieves | Ocean read by depth colour, foam lines on shore and crest, spray in wind | P1 coast |
| A Short Hike / Firewatch | Strict palette per biome and time of day; grade as art direction, not realism | Biome grade (tint already exists) |
| Teardown | Contact shadows and AO at voxel scale give weight to small geometry | P7 medium AO |

## Work packages (priority order)

### P1 Shore and ocean (this pass)

- [implemented 2026-11-12, NOT yet captured] Spray on open coasts. `ambience::place` replaced the
  shore biome's spray weight with `water x water_motion`; sea water is static
  in the cell simulation, so open coasts had no spray. Now spray is the larger
  of the moving-water term and an open-water term scaled by surface wind.
- Spray motes now draw in two frames, sea-grey and palette paper, so they read
  against both water and sky (`mood_shared.wgsl` kind 6).
- Capture status: shore poses on the owner seed so far landed underwater or
  inside the terminal island rim (`cap.ps1` clearance is above the seabed;
  spray needs a grounded camera within about 16 m of water, and motes draw
  only within 22 m). Next: find a sand beach column with a planet column
  probe, then stand on it at clearance 1.7.
- Captures 2026-11-12 (owner seed, review-time 0.8 = day there):
  - The island near (1100,-900) is not land. Worldgen gives -58 m seabed there,
    so poses kept landing in water.
  - Real sand shore: `-5720,-23370,1.7,0,-0.12`, sea to the south and west.
    It is a sheltered mangrove flat. The local biome has no spray weight, so A
    (pre-fix) and B (fix) are identical. Correct, but still not a test of the
    fix: it needs an Ocean/Coast-biome exposed beach.
  - Far poses need about 6 minutes (2.5 min solve plus streaming), so use a
    copy of `cap.ps1` with `WaitForExit(600000)`.
- Swash band checked: planet `SEA` is `WATER_LEVEL_METERS` (12 m), the same
  absolute metres the band uses, so it lines up. The owner's beach without a
  wet band needs a different explanation (possibly calm water, since the band
  follows breaker energy).
- Wet swash band: darken and gloss sand within ~1.5 m above sea level near
  breakers (shader, `world_shader.wgsl` sand material by height above sea and
  breaker energy). Capture at a beach pose, day and dusk.
- Foam line on sand at the breaker reach (reuse `water_breaker_band`).
- Later, needs owner approval: seabird silhouettes as fauna (`soundscape/fauna.rs`
  already has a bird-call path).

### P2 Step-edge readability (largest first-person win)

- Source found 2026-11-12 (pose `6519,6135,1.7,0.8,-0.15`, stepped stone
  hills): the lines are the one-voxel riser faces. They face away from the sun
  and sky, so each step is a dark band.
- Tried and reverted: half-weight vertex AO for single-step occluders in
  `mesh.rs` `face_corner_ao`. A/B at that pose and at the beach showed no
  visible change, because the AO crease is not what draws the line.
- Next: light single-step risers (the riser block has air above it) partly as
  if they faced up, blending lighting toward the top face. The mesher knows
  the step height; it needs a vertex flag (the normal also picks the texture
  face, so do not bend the normal itself). Keep full contrast on walls of two
  or more voxels.
- Grass lip: the blade-clump mesher already places tufts; bias them onto step
  edges so turf overhangs the riser (Vintage Story style).

### P3 Shared gust field

- One travelling gust function of world position and simulation time, read by
  grass blade sway, leaf sprays, the mote wind term (`local_surface_wind`) and
  the wind audio bed. Today each uses a constant wind vector.

### P4 Biome particle content

- Forest: falling leaves (drift kind with a slow tumble and autumn palette from
  `foliage_tint`).
- Desert and beach: blowing sand near the ground in wind (dust kind, low band).
- Wetland: midge swarms at dusk (pollen kind clustered in a few cells).
- Snow: diamond dust already exists; add wind-blown snow at ridges.
- Keep the presence ceiling (`min(total * 0.055, 0.14)`); richness must not add
  ray marches.

### P5 Flora at sub-metre scale

- Multi-voxel plant models sized in metres for voxel scales below 1 m
  (cabbage, flowers, grass tufts), so a quarter-metre world does not show
  single 25 cm cubes. Gate with the flora placement change.

### P6 Weather on surfaces

- Wet darkening and gloss on exposed surfaces after rain (rain accumulator
  already in lighting), puddles in shallow hollows.

### P7 Lighting depth (from the 11-03 critique)

- Measure exposure first (`audit-views.ps1`), then medium-scale AO and near
  macro normals inside 64 m.

## Verification recipe

1. Build A and B exes into `C:/AI/pyrenet-work/.cargo-target-biomeart`
   (release-fast); never trust a piped build.
2. Poses: beach (gen 290 planet, owner seed), river mouth, aqueduct meadow
   (owner screenshot 11-12, `6519,160,6135`), forest, desert.
3. Same review time (0.45 day and 0.8 dusk), clouds on and off.
4. Side-by-side PNGs in the session scratchpad; record luma of foreground band.
5. Ambience tuning live via the workbench `ambience` tuning before baking new
   defaults.
