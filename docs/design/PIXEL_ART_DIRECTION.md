# Pixel-art direction and asset pipeline

## One palette

`assets/pixel_palette.txt` is the canonical palette. It is mirrored by:

- `src/egui_ui/pixel_art.rs` for runtime UI sprites,
- `tools/pixel_palette.py` for Python generators,
- `src/egui_ui/icons_spells.rs` via the generator.

| Role | Value | Use |
|---|---|---|
| outline | `#1d1f22` | one-pixel silhouette and internal separations |
| paper | `#ebe1c5` | warm text/paper accents, never a large fill |
| gold | `#daad60` | fittings, trimmed machinery, treasure |
| ember | `#c16631` | wood, heat, rust and warn states |
| teal | `#65b7af` | mineral/arcane body accent and fluid/power |
| highlight | `#f7e49f` | specular pixels and spell cores |
| shade | `0.62` | body role multiplied for the shadow side |
| bottom shade | `0.76` | one-pixel exposed lower edge |
| top light | `0.20` paper mix | one-pixel exposed upper edge |

The six glyph roles are stable across every sprite:

`1` shade, `2` body/accent, `3` outline, `4` highlight, `5` gold, `6` ember.

## Drawing rules

- Author on an integer pixel grid. UI sprites are at most 14×12 glyphs and are
  centred before integer physical-pixel scaling.
- Light comes from the upper left. The shared painter adds the top-light and
  bottom-shadow strips on body pixels; generators bake the same strips into
  source PNGs.
- One outline colour only. Do not use black for shading; use the shade role.
- Reserve highlight (`4`) for the brightest one or two pixels. It should read
  as a spark, crystal core or polished metal, not a general fill.
- Spell accents still carry element identity, but their shade/highlight ramp is
  derived from the same roles. No sprite invents its own outline or cream.
- World textures retain their metric albedo lattice. The held kit has its own
  art direction below; solid wand parts must not take the glow-particle path.

## Held kit: authored atlas and Blender forms

The current owner direction is **Minecraft modded style**, with models authored
in Blender. This supersedes the earlier low-poly choice and the overly detailed
Blender candidate on 2026-10-25. The hand and sleeve were restarted on 2026-10-26.
Match the world's coarse detail through broad planes, pixel colour clusters,
a limited material palette, readable silhouettes and quiet surfaces.

The grip has closed finger profiles that curl in depth around the handle,
a palm heel and an opposed thumb. The sleeve has a short asymmetric outline
and a leather cuff. It retains uniform scale through every pose. Split timber,
ceramic insulation, a copper strap and a recessed mineral tie the default wand
to the lore's workshops. Detail stays concentrated around functional parts.

`tools/pixel_kit/build.py` now authors the editable source at
`assets/viewmodel/pixel_source/workshop_kit.blend`. This replaces the rejected
remeshing pipeline in `tools/blender_artificer`; its source remains historical.
The runtime caches `assets/viewmodel/*.ron` and samples `kit.png`'s identical
64×64 RGBA upload using nearest texels in linear-light shading. Materials 54/55
retain cloth, skin, wood, leather, metal and mineral finishes. The mineral
follows the selected spell and cast-release energy. The expanded art rationale,
software research, evidence and limitations are in
`PIXEL_WORKSHOP_RESET_20261026.md`.

## Generators

```powershell
# Spell/modifier illustrations plus optional preview sheet
python tools\gen_spell_icons.py target\spell-icon-sheet.png

# Four 16×16 source sprites in assets/
python tools\gen_tool_icons.py
```

Both generators load `tools/pixel_palette.py` and validate glyphs, silhouette
size, outline/body presence and native-grid alignment before writing.

## Audio asset direction

- Each cue has three decorrelated variants; do not add one-off duplicate cues
  for variation.
- Cues are peak-normalized into a common range after reflections, then receive
  a 12 ms cosine release so the final sample is zero.
- Reverb sends are cue-family based: impacts/rifts receive space, rapid-fire
  sparks stay dry and articulate.
- New cue families must define a reverb send and a spell mapping rather than
  falling back to an unrelated projectile sound.

## Validation

- `cargo check --offline --target-dir target`
- `cargo test --offline --target-dir target pixel_art`
- `cargo test --offline --target-dir target every_spell_and_resource`
- `cargo test --offline --target-dir target audio::`
- Full suite with `--test-threads=1`.
