# Planet geology, placer gold and processing: design

Status: slice 1 built behind dormant generator gate 122 (2026-11-03).

## Intent

Planets should read as real places with causes. A geoscientist or a gold
panner walking a basin should be able to explain what they see: why a river
is where it is, why a rock band crops out on that slope, why gold sits on the
inside of that bend. The same causes should make the game: prospecting is a
skill of reading land and water, processing raises recovery, and water is the
power source and the hazard.

## Goals

- One coherent drainage basin at every scale: walking, ridge view, orbit.
- Process chain the player can trace: rain, rock, erosion, transport, deposit.
- Placer gold that follows hydraulics, found by panning upstream ("follow the
  colour") rather than by random strip mining.
- A processing ladder (pan, rocker, sluice, wheel and flume, monitor, dredge,
  stamp mill, smelting) where each step raises recovery and has byproducts.
- Planets differ by ore system and history, not by palette.

## Non-goals for slice 1

- Whole-planet drainage (the `sphere` module stays unwired).
- Smooth non-staircase ground meshing.
- New factory machine kinds or resources (each needs a save bump).
- Elevator gameplay and treasure-message work (separate projects).

## Anti-goals

- Landmark scatter every few hundred metres.
- Distant terrain that differs from the walked ground.
- Detail paid for with frame-time spikes or blocking generation.

## Slice 1 (built, gate 122, dormant)

- `geo::active` covers Planet worlds from gate 122 with `PLANET_FACE`
  parameters (one 32 km atlas face, 520 m relief, glacial erosion on).
- `basin.rs`: the atlas face holding the origin uses the hills column
  pipeline (classification, rivers, lakes, strata, woodland). A 1.5 km band
  inside the face edge blends height into the noise shell; the solve already
  rings its face with ocean. Other faces and older planets are unchanged.
- The block path on the solved face skips the hills worlds' authored sites;
  only the meridian terminal (elevator base) is kept, which also fixes the
  monument missing on planets.
- `placer.rs`: lode belts (thin warped noise bands) shed gold where they erode
  (belt strength times channel drop). Gold is routed down the solve's receiver
  network in height order, diluted by discharge and concentrated where stream
  power falls relative to the strongest donor. Grade is normalised on the 98th
  percentile.
- Riverbed cells with grade above `PAYDIRT_GRADE` generate as the appended
  `Paydirt` block (no save bump; appended block ids are append-safe).
- Panning reads the placer grade on the solved face, counts Paydirt as a rich
  bed (3x), and reports `Colour N ‰` on every scoop so players can trace a
  gradient upstream to its belt.
- Far geometry (planet cover at 384 cells on the solved face, globe tiles,
  far previews above 32 m footprint) uses a coarse cover column. The exact
  column builds a detail tile per scattered sample (about 5 ms).

### Slice 1b (built, same gate)

- Lode veins: `placer::vein` makes thin, steeply dipping quartz sheets inside
  belts. They show in rock and on bare slopes as the appended `GoldQuartz`
  block. The field smelter turns 8 of them plus 0.25 coal into one gold block.
- Gate-125 planet caverns (`sites::cavern`) also generate on the solved face.
- The HUD labels planet ground from the planet's own cover off the solved face
  (`basin::planet_label`), so labels no longer come from the hills biome map.
- The planet cover adds a 20 m patch, 8 km wide, around the eye. It recentres
  every 2 km on a worker, and the coarse face grid leaves a hole under it.
  Valleys and ridges now read in the 0.2 to 4 km band.
- The far cover reports ground plus a canopy share. Previews and globe tiles
  show closed stands as canopy.

### Slice 1c: cover matches walked ground (built, same gate)

- Cause of the dark band past loaded chunks: the planet branch of
  `world_shader.wgsl` replaced every backdrop fragment with
  `base * (0.06 + N.L)`, and cover vertices carried material key 0. Cover now
  takes the terrain's full shading below 1.5 km altitude, blends to the plain
  lit colour by 8 km, then to the orbit body as before.
- Cover vertices carry the block's colour, roughness, specular and shader key
  (as horizon tiles do); ocean cells are encoded like horizon water.
- Far cover runs the column material policy (`landscape::far_column`, which
  skips only the outcrop ring, stratigraphic bed and caves). Far and exact
  surfaces agree on 94% of land samples, against 52% for the earlier biome
  table, which painted snow fields over grass. `far_cover_shows_the_walked_surface`
  guards it.
- Solved-face cover colours average 4 samples per cell; the policy switches
  material on 7 to 23 m noise, which a single point per cell aliased into a
  checker from altitude.

Remaining: slope relief reads softer than the old high-contrast cover at the
ridge pose, because the cover now gets the same diffuse light as the resident
rock. Planets still lack the horizon system's vegetation band.

## Next slices

1. Structural rock: dipping, folded strata as 3D planes so outcrops obey the
   rule of V's; resistant beds make knickpoints; lithology feeds erodibility in
   the solve. Gold-bearing quartz veins on belts, exposed on slopes, with
   float and iron staining downslope as surface indicators.
2. Orographic rain: advect moisture across the solve grid with the prevailing
   wind so ranges cast rain shadows; ecology follows.
3. Sediment sorting: grain size from stream power (boulder, gravel, sand,
   silt), terraces where base level fell, fans at range fronts.
4. Processing ladder: rocker and sluice as modes of existing machines first;
   sluice reads `World::water_velocity`; tailings are real sand in the
   cellular sim and bury downstream riffles. New resources (gold dust, black
   sand, garnet) wait for one planned save bump that adds them together.
5. Water power: head and flow drive wheels, sluices and monitors; dams and
   flumes become infrastructure. Needs a flow model in the water sim.
6. Planet identities by ore system (volcanic epithermal, ancient craton with
   banded iron and kimberlite, relic world with reprocessable tailings), with
   the elevator importing what a world lacks.
7. Whole-planet drainage from `sphere` and smoother ground meshing.

## Open questions for the owner

- Promote gate 122 to the new-world default once captures are accepted?
- Names for new blocks and items beyond "Paydirt" need approval.
