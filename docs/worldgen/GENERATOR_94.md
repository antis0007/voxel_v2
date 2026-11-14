# Generator 94: relict landscapes

Seven new biome variants use the compact geology and regional climate from
generators 92–93. Placement uses a seeded, rotated 280 m field with a smaller
boundary warp, then temperature, moisture, height and slope constraints. The
first 130 m retain the established starting landscape. Ocean, desert, badlands,
tundra and alpine retain priority. Older generator versions keep their rules.

| Biome | Placement | Ground and vegetation |
| --- | --- | --- |
| Lost plateau | Warm, wet uplands at 60–190 m | Limestone patches, turf, short cycads and tall umbrella trees |
| Cycad marsh | Warm, very wet, gentle ground below 75 m | Mud and moss over clay, radial cycad fronds |
| Cloud forest | Cool, wet uplands above 85 m | Moss and soil, hanging broadleaf crowns |
| Redwood grove | Cool, moderately wet land | Soil and moss, 22–34 m tapered conifers with thick trunks |
| Overgrown gardens | Mild, gentle lowlands | Turf and moss, scattered weeping trees |
| Fungal grove | Rare wet regional interiors | Fungal soil and moss, amber and umber mushroom canopies |
| Last shore | Cool coastal regions | Clay and sandstone with sparse moss, no ordinary trees |

Cycad, fungal and redwood forms have their own voxel samplers and explicit
horizontal/vertical bounds. The shared tree placement admits moss and fungal
soil, while unrelated giant broadleaf trees stand aside in these three biomes.
The same biome selector, material policy and tree profiles serve resident and
horizon sampling. Diagnostic map colours and player-facing labels include all
seven additions. Far tree proxies retain the existing generic crown rendering;
the new canopy geometry is exact voxel geometry inside resident terrain.

Surface accents run after generic geology. Valley settlements, riverbanks,
lakes and authored district overlays retain their later placement priority.
The variants do not change height at categorical boundaries, avoiding cliff
seams. Lost plateau occupies suitable existing uplands; it does not create a
new flat-topped mesa. Cloud forest does not add a local cloud renderer.
Overgrown gardens do not add ruins or flowers in this increment.

Creative references:

- [Biomes O' Plenty](https://github.com/Glitchfiend/BiomesOPlenty): distinct
  ecological regions and vegetation silhouettes. No mod assets or code copied.
- [The Lost World](https://www.gutenberg.org/cache/epub/139/pg139-images.html):
  isolated uplands, prehistoric forests, cycads and bogs.
- [The Time Machine](https://www.gutenberg.org/cache/epub/35/pg35-images.html):
  neglected gardens and the far-future red shore with surviving green growth.
  Last shore reinterprets the ground palette, not the novel's astronomical state.

Validation: offline `cargo check -p engine_worldgen` passed. No tests were added
or run. Native appearance, per-seed coverage, proxy transitions and exploration
pacing remain unverified. The executable has not been rebuilt. Use a fresh
generator-94 world after rebuilding; existing saves are not migrated.
