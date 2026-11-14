# 05: First-person swords, tools and arm: authored sprites and real animation

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first. Visual
change: before/after captures required. Start after prompt 04 has merged, since
both edit `mesh/models/held_item.rs`.

## Goal

Held swords, pickaxes, bores and assembled tools look like hand-authored pixel
art with a readable silhouette and material shading, and move with a real
attack, mining and idle animation. The arm and hand read as the same pixel
style. Hotbar icon and held item still come from one source of art.

## Current implementation (verified by reading)

- `crates/app/src/mesh/models/held_item.rs` (437 lines): "other items as their
  hotbar sprite extruded one texel deep" with `SPRITE_TEXEL = 0.016`, hold point
  `REST = (0.34, -0.22, 0.62)` m, `CUBE_EDGE = 0.20`. `View` carries `sway`,
  `gait`, `speed`, `interact` (0..1 pulse), `mine` (-1..1), `lowered`.
  `frame(v)` turns those into a position offset and a rotation about X
  (`-0.4*poke - 0.42*mine`) and Z (`0.16*mine`). `placement()` gives weapons
  `Quat::from_rotation_y(-0.2) * from_rotation_z(0.25)` with the note
  "Reference mesh is already authored around the centre of its binding."
- `crates/app/src/egui_ui/held_art.rs`: `Art::{Block, Icon, Wand}`; `Item::Weapon(tier)`
  maps to `Icon::Sword(tier)`, `Item::Tool(6..=8)` to `Icon::Pickaxe`,
  `Item::Tool(1|2)` to `Icon::Bore`, `Item::Assembled` to `Icon::BuiltTool(schema,
  materials, state)` via `engine_tools`.
- `egui_ui/icons.rs`: `Icon::Sword(tier)` at about line 542 and 980,
  `Icon::Pickaxe(tier)` at about line 178; `egui_ui/icons/sprites16.rs` (167
  lines) multi-palette 16x16 sprites from text sheets (memory
  `sprites16-icon-sheets`: bucket, shears and pickaxe arms were waiting on peer
  Icon variants). Enchanted swords get `magic_fx::Effect::Enchanted`.
- Other viewmodel pieces: `mesh/models/reference_sprites.rs`, `voxel_asset.rs`
  (`.vox` loader), `wand_asset.rs`, `wand_pose.rs`, `wand_kit.rs`, `wayfarer.rs`,
  `hart.rs`. `assets/runtime/viewmodels/presentation.ron` is the viewmodel
  presentation table (check its schema first). The wand appearance pipeline is
  the model to copy: `assets/wands.ron`, `docs/design/WAND_APPEARANCE_SCHEMA.md`,
  one texel size `pixel_m`, authored family sprites.
- Memory to read: `wand-arm-viewmodel-audit` (2026-10-25 defects and the capture
  recipe without a rebuild, `VOXEL_REVIEW_WAND` and `VOXEL_REVIEW_STYLE`),
  `machines-mekanism-look-rejected` (owner direction: authored voxel assets on
  the fine sub-grid, not float greebles), `held-item-viewmodel`
  (`VOXEL_REVIEW_HELD=stone|grass|planks|crystal|torch|food|sword|gem`),
  `wand-appearance-foundation`, `sprites16-icon-sheets`,
  `visual-changes-need-ab-captures`.
- The screenshot that triggered this prompt: a sword with a brown jagged blade,
  a red pommel, a flat beige hand and a blue sleeve ending in a hard angular
  forearm. Problems visible: blade outline is noisy with no coherent edge
  highlight, the hand is a skin-coloured slab, the sleeve is a smooth flat
  polygon unlike the pixel items, and the sword sits along the arm with no
  guard or grip structure.

## Design

### A. One pixel size for everything held
Use a single constant for held-art texels (`SPRITE_TEXEL`, 0.016 m, or the
value the owner prefers after capture; decide once, here). The arm, hand,
sleeve, sword, tools and torch (prompt 04) all use it. Anything glued to the
camera is textured in view space (`is_viewmodel`, `vm_position` in
`world_shader.wgsl`); never texture with world-anchored patterns.

### B. Authored sprites, not extruded icons
- Hotbar icons stay 16x16. Held art gets its own higher resolution art, 24x24
  or 32x32 depending on the item, authored per item class and material, stored
  as data. Preferred storage: text sheets like `sprites16` (rows of palette
  letters, reviewed in diffs) or `.vox` through `voxel_asset.rs` if a 3D shape
  matters (guard, pommel, pickaxe head with thickness). Pick one pipeline and
  document it in `docs/design/` next to the wand schema; do not add a third.
- Art rules per sprite: a 1 texel dark outline on the shadow side only, a 1
  texel light rim on the upper-left edge, 3 to 4 flat shade steps per material
  from the palette (`assets/pixel_palette.txt`), no per-texel noise except
  deliberate wear, a visible guard, grip, pommel and a fuller on blades,
  head/shaft/binding on tools. Silhouette must read at 64 px on screen.
- Materials come from `engine_tools::MATERIALS` for assembled tools: compose the
  held sprite from the part sprites (head, handle, binding) with the same
  `ToolAssembly` data `tool_icon` already uses, so a built tool looks like its
  parts. Broken tools (`is_broken()`) show a chipped head.
- Thickness: 2 texels deep for blades, 3 to 4 for heads and handles, with side
  faces one shade darker. Extrusion depth is data per item, not a constant.
- Tiers 0..4 of `Weapon(tier)`: distinct silhouettes, not recolours. Tier 3 and
  4 keep the enchanted effect through `magic_fx`, driven from the same data.

### C. Presentation data
Per-item held profile in `assets/runtime/viewmodels/presentation.ron` (or a new
`held_items.ron` if the schema does not fit): sprite id, texel size, grip point
in texels, rest pose offset and rotation, swing profile id, trail flag. Add a
`*_ron_round_trips` test. No content tables in Rust.

### D. Arm and hand
- Rebuild the forearm, hand and sleeve as texel geometry at the same size:
  sleeve with 2 to 3 shade steps and a cuff line, hand with 4 distinct finger
  texels wrapped around the grip (the hand sprite changes grip shape per item
  class: sword grip, tool grip, empty, torch).
- The arm origin must stay below and right of the view; check clipping against
  the near plane and the world grid. Small moving geometry is never clipped by
  the world voxel grid (render skill invariant 2).
- Skin and sleeve colours come from the player's appearance if the engine has
  one; do not hard-code a second palette.

### E. Animation (replace the single `poke`/`mine` rotation)
Add a small state machine in a new file `mesh/models/held_motion.rs` (keep
`held_item.rs` under 300 lines of new logic). Inputs already exist in `View`;
add `swing: f32` (0..1 phase) and `swing_kind: u8` set by the caller from the
combat/mining systems. Phases:
1. **Idle**: breathing offset of 1 texel at 0.25 Hz plus gait bob
   (`gait.cos() * 0.006`, scaled by `comfort::camera_motion()`); sword tip
   lags the hand by a damped spring.
2. **Sword attack**: windup (0.00 to 0.25, pull back and up, roll -20 deg),
   strike (0.25 to 0.45, fast arc across the screen with ease-in), hit hold
   (2 frames), recover (0.45 to 1.0, ease-out). Two alternating combo
   variants (diagonal down-left, diagonal down-right). Total about 0.35 to
   0.45 s, matching the weapon cooldown in `world/adventure`; read that value,
   do not duplicate it.
3. **Mining/tool swing**: overhead arc for pickaxes, a short stab for bores,
   chop for axes; hit frame aligned with the actual block damage tick.
4. **Block place / use**: existing `poke` retained but eased.
5. **Draw/holster**: `lowered` (0..1) already exists for wands; reuse for item
   switching so swapping slots lowers and raises.
Motion stays smooth and continuous; only the art is pixel art. If texel edges
shimmer during motion, quantise the final screen position to whole screen
pixels in the vertex stage; add that only after a capture shows the shimmer.
- Presentation only: animation phase may use frame time, but gameplay hit timing
  comes from the fixed tick (`simulation::STEP_SECONDS`, 1/120 s). The viewmodel
  derives its phase from the fixed-tick attack state so it cannot desync;
  `Instant::now()` stays out of simulation code.
- Comfort: scale all translation and roll by `comfort::camera_motion()`; with
  reduced motion, replace the arc with a 2-frame nudge.

### F. Optional trail
A 3 to 5 texel-wide trail during strike frames: stacked texel quads with
decreasing alpha, palette colour per material. Behind a flag in the profile;
off for tools. Must be bounded geometry (fixed quad count).

## Performance and budgets

Held mesh is rebuilt per frame (`models::build`), memory
`orbital-scene-cpu-cost` shows model rebuilds can cost milliseconds. Cache the
per-item mesh keyed by (item, texel size, tool state) and animate with a
transform uniform or by transforming a cached vertex list, not by rebuilding
texel geometry each frame. Target: held mesh CPU under 0.1 ms. Measure with the
frame bench and interleaved A/B (memory `timing-tests-need-interleaving`).

## Capture and acceptance

- Before (baseline) and after, same poses, unique labels, under `target/`:
  idle, mid-swing (frames at 0.15, 0.35, 0.6 of the phase), mining, each of
  `VOXEL_REVIEW_HELD=sword|torch|stone`, a built pickaxe, an enchanted sword,
  at 1 m, 0.5 m and 0.25 m voxel scale.
- Pass: silhouettes readable, one texel size throughout, outline and rim
  consistent, hand wraps the grip, no clipping through terrain at close range,
  swing reads as an arc in a 6-frame strip, hotbar icon and held item
  match, no regression in frame time.
- Tests: `just test held`, `just test icons`, RON round-trip,
  `just lint-ui` if any UI text changes, held-motion unit tests (phase
  boundaries, easing monotonic, reduced-motion path).

## Do not

- Do not edit `egui_ui/wand_xmb.rs` or `adventure/growth.rs` (in-progress work).
- Do not add smooth float greebles. Do not invent new item names or lore.
- Do not change combat damage or cooldown values; read them.
