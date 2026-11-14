# Planet lab

Render and measure generated Planet terrain without a window or the game. One
process solves a planet once and then answers any number of views, so a
generator change is checked in minutes and compared against the previous
generator in the same picture.

The lab draws generated ground: relief, surface cover, water. It does not draw
trees, props, 3D carves (ravines, caves) or the game's shading. Use a native
capture for those, and for anything the player-visible renderer decides.

## Commands

```text
node tools/axiom-dev/planet-lab-cli.mjs build
node tools/axiom-dev/planet-lab-cli.mjs review 290 300 --at 4400,4100
node tools/axiom-dev/planet-lab-cli.mjs patch --gen 300 --side 512 --px 768 --layer detail
node tools/axiom-dev/planet-lab-cli.mjs view --gen 290,300 --alt 2 --yaw 40 --pitch -5
node tools/axiom-dev/planet-lab-cli.mjs globe --layer tectonics --lon 40 --lat 10
node tools/axiom-dev/planet-lab-cli.mjs stats --gen 290,300 --side 512 --spacing 1
node tools/axiom-dev/planet-lab-cli.mjs --fast where
```

| Command | Shows | Main options |
| --- | --- | --- |
| `patch` | Top-down window, north up, one sample per pixel, sun shadows | `--side` metres, `--px`, `--layer shaded, cover, slope, height, detail` |
| `view` | Perspective from the ground or the air, marched against nested height grids | `--alt` metres above ground, `--yaw`, `--pitch`, `--fov`, `--size WxH` |
| `globe` | Whole planet, orthographic | `--lon`, `--lat`, `--size`, `--phase` sun angle, `--layer shaded, height, tectonics, flow, sediment, climate` |
| `stats` | One JSON object per generator | `--side`, `--spacing` |
| `where` | Chart metres of spawn, the coastal bake and the nearest confluence | |
| `find` | Nearest dry places matching cover and form, as chart metres | `--block Sand`, `--min-h`, `--max-h` (above sea), `--min-slope`, `--max-slope`, `--side`, `--spacing`, `--limit` |
| `probe` | Height, slope, gradient measured from heights and gradient the erosion tiles report | `--at`, `--step` |
| `batch <file>` | One command per line, one solve for all | |
| `review <A> [B]` | The standard evidence set, A/B when two generators are given | `--at`, `--name` |

Common options: `--seed` (default 2738958700, the review planet), `--gen A[,B]`,
`--scale 1, 0.5, 0.25`, `--at x,z` or `spawn`, `coast`, `confluence`,
`--out path-prefix` (default `target/planet-lab/<command>-<generators>`),
`--sun azimuth,elevation` in degrees, `--exposure`.

`--gen A,B` writes both generators side by side, A on the left. `patch` also
writes `-diff.png` (red where B is higher, blue where it is lower) and prints
the mean and largest height change.

Yaw 0 looks along +z (up in a patch); positive yaw turns toward +x.

## Layers

- `detail`: height minus its 48 m box blur. Orange stands above the local
  mean, blue below. This is the layer that shows gullies, knolls and ripples;
  a flat grey picture means the ground has no relief below about 100 m.
- `height`: hypsometric tint with 10 m contours.
- Globe `tectonics`: red collision, green plate boundary proximity, blue rift.
  `flow`: routed discharge. `sediment`: mobile cover (yellow), former ice
  (blue), carbonate (grey). `climate`: red temperature, blue rain.

## Stats

`relief_rms_m_by_wavelength_m` is the height left after removing the mean of
four neighbours, per octave. It is the number to compare when a change claims
more or less fine detail. `slope` percentiles and `max` catch a discontinuity
(a wall where two samples disagree). `riser_fraction` is the share of land
steeper than 1 and 2; `break_of_slope_per_ha` counts neighbouring cells whose
slope differs by more than 1, so a ledge lip or cliff foot counts and a
smooth swell does not. Compare both when a change claims rock benches or
cliffs (generator 304). `us_per_query_by_footprint_m` is the
single-thread cost of one ground query; chunk generation and the horizon pay
it per column. `hash` pins determinism: the same generator, seed and window
must print the same hash twice.

## Debugging a pattern

A stripe or band that should follow the slope and does not: run `probe` at the
spot. `measured_gradient` (finite difference of sampled heights) and
`tile_gradient` (what `slope_tiles.apply` hands the carve stage) must agree in
direction; if they do, the pattern is wrong, not its input. Then render the
same window with `--layer height`: anything parallel to the 10 m contour
bands is periodic in height, not in position. That is how the generator 300
terracettes were told apart from its gullies.

## Speed

A full solve is about 3.5 minutes (720 Myr of plate history). The lab keeps
its own geology cache in `target/planet-lab/geology` (override with
`PLANET_LAB_CACHE`). `--fast` sets `VOXEL_HISTORY_MYR=20`: a quick solve of a
different, younger planet, good for checking a mechanism and not for
comparing against a saved world.

## Orbital shaders without a rebuild

`VOXEL_SHADER_DEV_DIR=<repo>/crates/app/src` makes the game load the globe,
cloud shell, planet air and celestial sky WGSL from disk at startup. Change a
constant, relaunch the capture, look. An invalid shader stops at pipeline
creation with the naga message. The built-in copies are what ships, so finish
with a normal build, which validates them.

## Adding a view

Sampling lives in `examples/planet_lab/field.rs` (`sample` is one
`planetary::orbital_surface` query), pictures in `render.rs`, numbers in
`stats.rs`. Solve fields the composed ground does not expose are read through
`planetary::review::lab::probe`; add a field there, never a second copy of
generator logic in the lab.
