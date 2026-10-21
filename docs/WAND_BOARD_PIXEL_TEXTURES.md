# Wand board, held-hand detail and pixel surface pass

## Spell/modifier board

`src/egui_ui/wands.rs` + `src/egui_ui/wand_xmb.rs`

The old deck sized every slot from its distance to the selected slot (72/44/32
px). Starting a drag changed the selection, which resized and re-wrapped the
whole deck while the pointer was trying to hit a target. That was the moving-UI
bug.

- Deck slots are now a fixed 46 px. Focus distance changes opacity only, never
  geometry.
- The deck sits in a fixed frame with a selected-slot line and a `Clear slot`
  button in a stable location.
- Drag targets get a dedicated highlight for the whole deck and a stronger
  outline over the hovered slot.
- The five stacked palettes were replaced with **fixed tabs** (Bolts, Effects,
  Summons, Shot mods, Program), a search field, and a reserved-height 54×58 px
  card grid. Selecting a tab, dragging, or filtering never moves the board.
- Modifier cards carry an amber left rail and a fixed icon/name layout.

## Held wand and hand

`src/mesh/models/wand.rs` + `src/mesh/models.rs`

- The hand is now layered lit geometry rather than a handful of glow bars:
  sleeve core and four cloth blocks, cuff band and stitch pixels, palm heel,
  back-of-hand plane, two segments per finger, knuckle highlights, and a
  two-part thumb.
- Skin/cloth use a new shader material (`kind == 54`) with a 14-cell/metre
  pixel grain and crease grid instead of inheriting brushed machinery or being
  treated as a glow particle.
- The wrist rolls with the cast, the fingers curl, the thumb presses, and the
  idle wand uses an incommensurate two-sine figure-eight instead of a single
  mechanical bob.
- The shaft has a stepped pommel, two collars and a ferrule. A brief
  eight-pixel flare ring expands from the crown at cast peak.
- `Model::set_surface_kind` makes the skin material explicit and testable.

## Pixel textures, dithering and grass

`src/shaders/world_shader.wgsl`

- Added a world-anchored 4×4 Bayer function and a 20-cell/metre (5 cm) albedo
  lattice. Opaque surfaces are quantised to 18 levels with dithering and fade
  the effect once the lattice becomes subpixel.
- Explicitly skipped water, glass, fire, glow particles and torch materials.
- Lowered the highest material-detail frequencies: 16→8 cells/m grain,
  11→7 mineral grain, 128→48 brush, 64→32 scratches/pits, 22→12 ore bands,
  20→12 mushroom gills.
- Grass is brighter and more saturated, with hard 12-cell/metre (8 cm) tip/root
  pixels, a strong two-tone blade lift and retained filtered LOD so distant
  turf does not boil.

## Validation

- `assets/pixel_palette.txt` is the shared six-role palette; Rust and Python
  tests keep their ramps aligned. See `docs/PIXEL_ART_DIRECTION.md`.
- Naga WGSL validation via `build.rs` passes after touching the shader sources.
- `cargo test --offline --target-dir target -- --test-threads=1`: **164 passed,
  0 failed, 13 ignored**.
- Tests cover fixed deck metrics and the new hand material tag.
- Runtime visual capture was not possible because the running game holds the
  executable open. Close it before rebuilding.
