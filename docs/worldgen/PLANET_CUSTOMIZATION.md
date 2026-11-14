# Planet customization

`New world` → `Planet` opens the recipe editor. Eight presets initialize twelve
integer dials; presets and rerolls retain locked dials. Seed lock affects rerolls.
Individual dials reset to 50; Undo restores the previous recipe and seed.
Recipe import explicitly replaces all dials, including locked ones.

| Group | Controls | Generator consumers |
| --- | --- | --- |
| Land & Water | Ocean extent, continent size, mountain height, erosion, lake depth | Plate land mask, noise shell, drainage erosion, inland lake beds |
| Climate | Temperature, rainfall, seasonality | Annual temperature, precipitation, discharge, glacial climate, seasonal temperature range |
| Life & Relics | Forest density, monuments | Climate-limited vegetation, Surveyor placement |
| Underground | Natural caves, ore deposits | Natural cave carving, geology-dependent ore bodies |

Mountain height is a terrain scale, not an exact height cap after local detail
and volcanoes. Forest density remains constrained by soil and climate. Ore
controls affect geological deposits; authored rewards and alluvial placers
remain independent. Natural caves exclude authored dungeon rooms. Monuments
control Surveyor works, not settlement populations or city size. Seasonality
changes generated climate; it does not reconfigure the orbital sky system.
Ocean extent and continent size are relative dials, not promised area percentages.

The continent preview uses the actual full-resolution drainage solve, with
terrain, height, temperature and rainfall layers. It depicts the solved spawn
face, without detailed buildings, voxel caves or volcano overlays. Work is
debounced 400 ms and runs on one background worker. Old results are discarded;
the previous recipe's map is hidden while a new recipe is pending. Generation
reuses the solve cache. No preview can mutate a save.

`AX136` recipe codes include the seed, all twelve dial values and a checksum.
Block size, difficulty and game mode remain separate creation options.

Custom planets use generator 136. The ordinary generator and historical planet
version constants retain their existing values. World sampling, drainage,
placement and renderer caches include the recipe or its fingerprint. Integer
settings travel in the initial multiplayer snapshot before terrain verification
and chunk installation; the lobby wire protocol is unchanged.

Save format 51 appends an optional recipe to the session footer. Older formats
load without overrides. The fixed manifest header and region references retain
their offsets. A malformed recipe rejects the save or snapshot before use.

Civilization history, population controls, generated cities and faction simulation
are separate systems; the editor does not expose inactive controls for them.
