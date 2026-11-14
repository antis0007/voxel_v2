# 04: Torch flame as pixel art, held and placed

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first. This is
a visual change: before/after captures are required (render rule).

## Goal

The torch flame reads as chunky pixel fire: few flat colours, square texels the
same size as the rest of the held art, motion in discrete frames. It is not a
smooth shaded volume with continuous gradient and sway. Held torch, offhand
torch, hotbar icon and placed torch all agree. Gameplay light behaviour is
unchanged.

## Current implementation (read these before touching anything)

- `crates/app/src/mesh/models/held_item.rs`
  - `torch(out)` builds the shaft with `torch_box` (3 boxes) and the ember box
    (`surface.w = -1`), then `flame_volume(out)`.
  - `flame_volume` builds a six-sided shell from 4 rings
    `(height, radius, bend)` with 4 continuous colours
    (`[1.0,0.88,0.47]` to `[1.0,0.29,0.055]`) plus a 3-ring secondary tongue.
    Each vertex carries `surface.w = -2 - lift` (lift 0..1 up the flame) and
    `KIND = 55.0`. Vertex colours interpolate across faces, which is the
    smooth gradient the owner dislikes.
  - Constants: `TORCH_FLAME = 0.185` m, `SPRITE_TEXEL = 0.016` m,
    `torch_tilt()`, `torch_flame_offset()` and `animated_torch_flame_offset()`
    (shared with the moving light).
- `crates/app/src/shaders/world_shader.wgsl` about lines 329 to 371: flame sway
  uses continuous `sin` of `light.torch_meta.y` with phase and `lift`; tip
  travel about 3 cm; the flame shortens on each flick; embers carry
  `surface.w = -4`, spawn near the tip and drift with `sin`/`cos`. `flame_lift`
  is defined near line 1066 (the mesher packs lift into `surface.w`).
- Fire field and flame pixel pattern helper:
  `crates/app/src/shaders/fluid_surface_shared.wgsl` lines about 64 to 110
  ("hard one-texel patterns (flame pixels...) keep full contrast",
  "sub-pixel texels fall back to the bare flame shape"). Reuse this, do not
  write a second fire function.
- Placed torch: `mesh/solid.rs` (`is_torch` branches at about lines 57 and 77)
  builds the same flame volume for world blocks; the held mesh was made to
  match it ("the placed torch's tapered volume").
- Hotbar and inventory icon: `egui_ui/icons.rs` `Icon::Torch` (about line 73).
- Menu UI flame (a different effect, already pixelated):
  `shaders/ui_prism.wgsl` `flame()` quantises to 2 px cells and 12 fps with
  banded heat. It is the visual target for stepping and banding.
- Palette: `assets/pixel_palette.txt`, mirrored as `MOTE_*` in
  `shaders/mood_shared.wgsl`. Use palette entries; do not invent colours.
- Memory to read: `fire-torch-capture-and-commit-gotchas` (the torch fixture
  ignores time of day; sandbox-review has fire voxels; one-hunk commit recipe),
  `factory-machine-review-captures` (VOXEL_REVIEW_FOCUS), `held-item-viewmodel`
  (`VOXEL_REVIEW_HELD=torch` capture), `visual-changes-need-ab-captures`,
  `feedback-no-posterization` (world shading stays smooth; this change applies
  to the flame and its embers only, not terrain or sky).

## Design

1. **Texel grid.** Pick one texel size for the flame. For the held view use
   `SPRITE_TEXEL` (0.016 m) so the flame matches the sword and arm (prompt 05
   keeps the same constant). For the placed torch use the world pixel
   (`PIXEL_RES` = 20 texels/m, 0.05 m) so it matches the block textures at that
   scale. Express this as two constants, not one magic number.
2. **Geometry becomes a small voxel stack.** Replace `flame_volume`'s smooth
   shell with axis-aligned texel cubes arranged as a column with a taper:
   about 3 texels wide at the base, 2 in the middle, 1 at the tip, 5 to 6
   rows tall, plus a 1-texel offset lick. Build it with the existing `Out::face`
   helpers or a `flame_box` like `torch_box` so triangles stay few (a cube is 12
   triangles; budget well under 400 vertices for the whole torch). Each cube
   carries a single flat colour; no per-vertex gradients.
3. **Discrete heat bands in the shader, not the mesh.** Keep `surface.w` lift
   encoding but quantise in the fragment stage: `band = floor(lift * 3.0)` and
   pick one of 3 or 4 palette colours (white-yellow core, yellow-orange,
   orange, red tip). No `mix` between bands. Reuse the hard-pixel pattern from
   `fluid_surface_shared.wgsl`.
4. **Stepped time.** Flame time becomes `floor(light.torch_meta.y * 8.0) / 8.0`
   (8 to 12 steps per second, same idea as `ui_prism.wgsl`). Displacement of
   the vertex stage is snapped to whole texels: `disp = round(disp / texel) *
   texel`. Tip sway then moves in 1-texel jumps. Per-frame `sin` jitter is
   removed. Keep sway amplitude small (0 to 1 texel) and phase derived from
   world position or the torch's seed so adjacent torches do not sync.
5. **Embers.** One screen pixel each (render rule: motes are one native pixel),
   palette colour, positions from a pure function of the stepped time and a
   per-torch seed. Count: 3 to 5 per torch. They already carry `surface.w =
   -4`; keep that contract and the exemption from flame sway.
6. **No smooth halo or glow orb.** The light is the dynamic torch light
   (`held_torch_pos_radius`, `torch_positions`). Do not add a bloom sprite.
   Keep the dynamic light's flicker, but note it is a separate system; if it
   should also step, that is a separate change, ask first.
7. **Ember box and shaft.** Replace the orange `torch_box(...-1.0)` blob (the
   ember box) with a 2x2 texel charred cap. Shaft keeps its wrap and collar but
   uses flat palette colours and a 1-texel dark outline on the lit edge for
   readability. Remove nothing the shader needs to find the flame: the vertex
   `surface.w` values (`-1` ember, `-2 - lift` flame, `-4` mote) remain the
   interface.
8. **Icon.** Update `Icon::Torch` to the same colour ramp and silhouette so the
   hotbar and the held item agree (the held art comes from `held_art::art`).
9. **Placed torch parity.** `mesh/solid.rs` must call the same flame builder
   (move the builder to a shared function if needed, for example
   `mesh/models/torch.rs`, keeping files under 300 lines). Chunks that contain
   torches rebuild only when edited; this change should not alter meshing cost.
   Compare vertex counts before and after for a 100-torch fixture.

## Contract checks (render rule)

- If you add or move fields in the vertex format (`Vertex3`), the CPU struct,
  the vertex layout and WGSL `VertexInput` change together. Prefer no format
  change: reuse `surface.w`.
- Any new uniform for stepped time must be added to both
  `camera_lighting_shared.wgsl` and `lighting_shared.wgsl` (they mirror each
  other: `torch_meta`). Prefer deriving the step in the shader from the
  existing `torch_meta.y`.
- `just check` validates shaders through `build.rs`; the shaders list test is
  `every_wgsl_file_is_included_by_shader_sources`.

## Capture and acceptance

- Baseline first (before editing): run the torch review fixture and
  `VOXEL_REVIEW_HELD=torch` from the same pose, with label
  `torch-pixel-before-<date>`; also a night pose with a placed torch row and one
  daylight pose. Then the same poses after, label `torch-pixel-after-<date>`.
  Recipes: memories `fire-torch-capture-and-commit-gotchas`,
  `held-item-viewmodel`, `review-capture-hooks`. Evidence under `target/`.
- Inspect PNGs at 1x and 3x crop. Pass criteria: texel edges visible and square;
  at most 4 flat flame colours; no gradient across a face; flame positions step
  between frames (capture three frames 125 ms apart and compare); embers are 1
  screen pixel.
- Performance: `just bench` torch scene or `--benchmark-*` equivalent; vertex
  count and frame time must not regress. Interleave A/B runs (memory
  `timing-tests-need-interleaving`).
- Tests: `just test torch`, mesh tests, `just test held`. Add a test that the
  held torch mesh has only the three `surface.w` classes and no vertex with a
  non-palette colour.

## Do not

- No change to the dynamic light radius or colour as part of this prompt.
- No posterisation of terrain, sky or water; the owner's rule is smooth world
  shading.
- Do not leave the old `flame_volume` as dead code.
