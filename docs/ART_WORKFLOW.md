# Art workflow: texture refiner

`art_refiner` renders the game's own surface shaders onto a labelled PNG sheet
without a window, a world, a game rebuild or the app crate's build script (a shader error elsewhere in the tree does not block it). It composes the same WGSL files
the renderer uses, appends a small compute harness (`crates/art_refiner/harness.wgsl`),
and calls `block_surface` and the tilt functions exactly as `fs_main` does.

## Loop

```
just art --tiles stones --faces side,up --watch
```

Open `target/art/refine.png` in an editor that reloads images. Edit any world
shader file under `crates/app/src/shaders/`; each save re-renders in about one
second (shader compile dominates). Compile errors print to the terminal and the
last good PNG stays.

Each tile has four bands: albedo, diffuse lighting with relief normals, combined
diffuse and specular lighting, and highlights/reflections alone at double strength.
Captions sit above each tile's albedo band and give the block and shader kind.
The refiner preserves the material ID as well as its family, so industrial
concrete, steel, corrosion, hazard paint and glazing keep their actual surfaces.

## Options

| Flag | Meaning |
|---|---|
| `--tiles` | preset (`stones ground ores wood metals industry kinds`) or comma separated `BlockId` names |
| `--faces` | `up`, `side`, `x`; one tile per face |
| `--zoom` | metres across one tile; 1.2 shows 24 texels per metre of tile |
| `--dist` | footprint multiplier; 8 shows how the material filters at range |
| `--origin`, `--light`, `--seed` | pattern position, sun direction, `cam.scene_seed.x` |
| `--out`, `--tile`, `--cols`, `--no-labels` | output path and layout |

`--tiles kinds` shows one block per distinct shader kind, a quick audit of every
procedural material.

## Rules for art changes

- Judge readability at `--dist 1` and `--dist 6`; a pattern that vanishes or
  shimmers at range needs a `surface_detail_weight` fade.
- Keep the 5 cm texel lattice (`PIXEL_RES`): sample from `physical_uv` or a
  texel-snapped position, never raw metres, for anything that should look like pixel art.
- Save a `before` sheet with `--out target/art/<name>_before.png` first, then the
  after sheet. The sheet is a fast filter, not final evidence: a visual change
  still needs an in-game before/after capture (`VOXEL_TORCH_REVIEW_MODE=minerals
  VOXEL_REVIEW_STONES=1 <exe> --benchmark-torch-review`, frame `OneMeter-004.png`).

## Limits

- Lighting is one directional light plus ambient. Torch BRDF, shadows, AO and
  wetness are not in the sheet.
- Base colour is the material's authored colour decoded to linear. Palette tints
  from the material studio are not applied.
- Anything computed in `fs_main` before `block_surface` is not reproduced:
  the kind 12 edge tilt and terrain macro relief. Kind 13 uses the shared `cobble_tilt`.
- The harness binds zeroed camera and lighting uniforms with metre voxels. A
  material that reads other `cam` or `light` fields shows them as zero.

## Keeping the file list honest

`crates/app/src/world_shader_files.rs` lists the world shader files in
composition order. The test `refiner_file_list_matches_the_world_program` fails
if that list differs from `world_shader_source()`.

## Industrial comparison

`just art --tiles industry --faces side --dist 1 --out target/art/industry-near.png`
compares industrial finishes with masonry, chest wood and a workbench. Repeat at
`--dist 6` and `--faces up` before changing the material. The `industry` preset
uses existing content IDs; it does not create a new material or change saves.

Focused regression checks: `cargo test -p art_refiner`; the ignored
`gpu_refiner_compiles_current_world_shader_and_renders_industry` test validates
and renders the current composed shader on the native adapter. These checks
validate the tool, not native gameplay lighting or art acceptance.
