# Biome and timber art direction, 25 October 2026

Scope: wood materials (bark, planks), per-biome ground and crown palettes in
`world_shader.wgsl`, and the rules that keep later passes on the same line.
Companion to `REFERENCE_BRIEF_20261024.md` (lighting and water rules still
hold) and `WOODLAND_ART_DIRECTION_20261024.md` (tree forms).

## References read for this pass

Downloads batch of 2026-10-25 01:26 to 01:29, 27 images. The ones that set
rules here, by basename:

- `sylvain-sarrailh-danghostisland`: saturated jade shallows, warm sand rims,
  crowns as clumped lobes with sky between, one red accent per 50 m of green.
- `pengzhen-zhang-0012`: meadow is olive and straw with small flower accents;
  mist in the valley; ruins read as warm ochre against cool fog.
- `tu-bui-rock-pillar-02`, `tu-bui-bnw-arch-sky-tb`: rock is warm ochre with
  moss only on up faces; hanging vines on the shaded side; red-leaf crowns as
  accents, never a field.
- `tu-bui-aftermath`, `komrade-matt-crashsite-fg`: desert is pale straw sand
  with rust-red rock and one dark value anchor; shadows cool lavender.
- `tu-bui-exotic-plants-01`, `tu-bui-dmz-*`, `jung-park-*`, `luke-aegis-*`,
  `james-paick-titanfall-1`, `bruno-werneck-titanfall`: overgrowth follows faces
  (moss on tops, vines from edges); big quiet planes with detail at edges.
- `camille-peyrebere-5`, `-6`: timber architecture is dark umber frames around
  pale infill; boards and beams read by their joints, not by grain noise.
- `mathilde-marion-ww-keyframe1-final`, `jean-francois-liesenborghs`: distance
  takes the sky colour; the horizon is a palette, not detail.

Filename attributions are not verified; the images, not the names, set the rules.

## Timber

Problem in the 2026-10-24 captures: trunks were flat dark brown blocks and the
workshop wall a single brown plane. Bark and planks were both "brown plus
noise", so nothing said wood.

Rules:

- Bark is plates on the pixel grid: columns 3 to 4 texels wide, plates 10 to
  17 texels tall, staggered per column. One-texel dark furrow between columns
  and plates; one-texel lit edge on the left of each plate (light from the
  upper left, like every sprite). Every fourth column seam is a deep furrow.
- Cut ends are wood: concentric rings, dark heartwood to pale sapwood, a dark
  bark rim two texels wide. Moss sits on some upper ends, never on all.
- Planks are boards 5 texels tall and 1.5 to 2.5 m long. Joints are
  staggered per row so a wall never repeats per block. One-texel seams, a
  one-texel lit top edge, long grain streaks (one texel high) and a knot on
  about one board in five. Each board has its own value and a warm or cool
  lean of a few percent.
- Lichen and moss are accents on older runs and in furrows, at most a third
  of the face.
- Any speck material (lichen on rock, pebbles in soil) is a step paler than
  the surface it sits on, never a fixed pale colour: a fixed grey reads as
  white confetti on basalt and on dark soil.

## Crowns

Crowns in the references are clumped lobes with sky gaps and three tones.
The leaf material shades each face by two lobe scales (4.8 m masses and
2.2 m clumps) so a crown reads as clustered lobes with shaded hollows;
sunlit tops carry the same clump field at lower contrast. Lobe silhouettes
stay with worldgen tree forms.

## Biome floors

Problem: every biome with a Grass surface (meadow, forest, taiga, tundra,
heath, fen, savanna, rainforest) rendered the same olive turf, so biomes were
only readable by tree species.

The shader now assembles a floor per biome from the same heat and moisture
fields worldgen classifies with (`classify_landscape` thresholds, softened
into ecotones). Linear reflectance, matted floor before tufts:

| Biome | Floor | Patch materials | Crown drift |
|---|---|---|---|
| Meadow | olive and sage (unchanged) | straw tips on dry patches | none |
| Temperate forest | cooler shaded green `(0.060, 0.100, 0.048)` at half strength | worn soil | none |
| Savanna | straw gold `(0.260, 0.205, 0.075)` | green near groves, bare dust | khaki olive |
| Rainforest | deep green `(0.028, 0.095, 0.040)` | emerald lights, wet dark soil | deep emerald |
| Taiga | blue-green moss `(0.040, 0.072, 0.050)` | rust needle litter, pale lichen specks | grey-green |
| Tundra | grey-ochre mat `(0.165, 0.160, 0.100)` | rust moss cushions, grey lichen specks, snow | grey-green |
| Highland heath | straw `(0.190, 0.165, 0.075)` | heather runs `(0.190, 0.090, 0.150)`, dark hollows | none |
| Lowland fen | wet green `(0.035, 0.090, 0.045)` | peat hollows, rush straw specks | none |
| Desert, badlands, coast | authored sand, clay and sandstone blocks | unchanged | none |

Biome weights are smoothsteps straddling the worldgen thresholds (warm 0.56,
polar 0.24, the moisture splits at 0.42, 0.49, 0.55, 0.58, 0.68 and 0.78), so
each floor reaches full strength inside its biome and blends over the
ecotone band. Forest blends at half strength. Snow overrides the floor. The
first revision used wider ramps and 0.8 blends and was not readable in
captures; the second revision is the one kept.

Rules for later passes:

- A biome is read from 500 m by floor colour and crown colour first, tree
  silhouette second, props last. Change the floor before adding props.
- Hue carries the biome; value stays in the same band as the meadow so
  exposure and fog keep working. No biome floor brighter than the straw
  savanna or darker than the rainforest.
- Accents are small and saturated on greyed masses (heather, rush, flowers).
  No field-wide saturated colour.
- Lighting rules from the reference brief stay: warm sun, cool shadow, far
  forms take the sky colour.

## Evidence

Seed 2738958700, generator 53, noon, partly cloudy, shadows off, clouds
skipped, 1280x720, static poses, `--benchmark-view-chunks=6`. Captures under
`target/reviews/forest-repair-20261021/` with `camera.png` as the last frame
and a `manifest.json` (executable hash, arguments, camera) per run:

- `biomeart-b2-<pose>` (before) and `biomeart-a1b-<pose>` (first revision) at
  the `--terrain-survey` points: meadow, forest, rainforest, savanna,
  badlands, desert, coast, wetlands, fen, taiga, heath, alpine, tundra,
  workshop, workshop-wall.
- `biomeart-b3-<pose>` (before) and `biomeart-a2-<pose>` (kept revision) at
  open-turf points picked from `target/maps/map-2738958700-v53-8k-b.png`:
  meadow2, forest2, rainforest2, savanna2, taiga2, tundra2, heath2, fen,
  taiga, badlands2, workshop-wall, workshop.
- `alb-b2-taiga` and `alb-a2-taiga`: `--benchmark-debug=albedo` at the taiga
  pose; the ground band mean drops from (90, 100, 59) to (74, 81, 54) sRGB
  and the litter patches appear.

Executables: `target/release-fast/biomeart_before2.exe` (dirty tree before
this pass, plus the benchmark-while-occluded fix), `biomeart_after_r1b.exe`,
`biomeart_after_r2.exe` (kept). Poses: session scratchpad
`Capture-Biomes2.ps1` and `Capture-Biomes3.ps1`; camera is
`x,z,clearance,yaw,pitch`.

Inspected and accepted: workshop-wall and workshop (bark plates, staggered
boards on the gable), rainforest2 (trunk plates, darker floor), savanna2
(straw-gold floor), taiga (dark moss floor with rust litter), tundra2 (ochre
and grey mat on the slopes). Unchanged as expected: meadow2, forest2, desert,
coast, badlands.

Round 2 (`biomeart-b3-*` before, `biomeart-a3-*` after, same seed and
settings): `--terrain-survey` now lists the nearest Grass-surfaced columns
per biome (`... turf:` lines), which gave heath3 `-1248,-356,5,0,-0.12`,
fen3 `96,-836,5,0,-0.12` and forest3 `288,-68,5,0,-0.12`. Inspected and
accepted: heath3 (heather runs and ochre on the slope), fen3 (darker wet
floor), forest3 and rainforest2 (crowns shade as clumps, two lobe scales),
heath2 (basalt lichen is a step paler than the rock instead of white
confetti), fen and fen3 banks (soil pebbles a step paler than their soil,
not white specks).

Limits: one seed, no shadows, no motion, no performance claim. The benchmark driver now renders while the
window is occluded (`main.rs`), because a fullscreen game in front of the
hidden review window stalled every capture; that change is part of this pass.
