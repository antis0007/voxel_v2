# Wand appearance schema, version 1

Data: `assets/wands.ron`. Loader and validation: `crates/app/src/wand_appearance.rs`.
Mesher, icons and dropped wands: `crates/app/src/mesh/models/wand_kit.rs`.
Families come from `docs/design/WAND_ECOSYSTEM_20261026.md`.

## Identity and saves

Nothing here is saved. A wand item keeps its `look` bits (tier, shaft, head,
glow, crafted flag; bits 0-11) and a held wand keeps its `WandStyle` indices.
`styles[head]` names the family drawn for head index 0-5, so the six seeded
heads draw six families. Shaft index picks the shaft material ramp, grip index
the binding colour, glow index the emitter colour. Reordering `styles` changes
the look of every saved wand; append families instead.

The save rejects wand looks with bits above 11 (`world/inventory.rs`), so the
grip is not carried on items: icons and dropped wands draw the default
binding. The held wand uses the full style.

## Table

| Field | Meaning |
|---|---|
| `version` | `SCHEMA_VERSION`; a mismatch fails validation |
| `pixel_m` | authored texel edge in model metres; the held rig scales it by `presentation.ron` `scale` (0.84) |
| `width` | texels per sprite row (16) |
| `styles` | family id per `WandStyle::head` |
| `families` | twelve rows, `Modeled` or `Planned` |

Family fields: `id`, `name` (must equal `wand_style::HEADS` for mapped
heads), `lineage` (`Vernacular`, `Scholarly`, `Industrial`, `Biological`,
`Anomalous`), `status`, `silhouette` (one line), `length_m` (0.45-0.75),
`max_depth` (1-6 texels), `materials`, `segments`.

### Sprite keys

| Key | Colour |
|---|---|
| `.` | empty |
| `1`-`4` | shaft material ramp from `SHAFTS[style.shaft]`, darkest first |
| `5`-`7` | grip ramp from `GRIPS[style.grip]`, darkest first |
| letters | family material: `kind` (`Wood`, `Bone`, `Fiber`, `Metal`, `Ceramic`, `Mineral`, `Glass`, `Ember`), sRGB `#rrggbb`, optional `emitter: true` |

`Fixed(rows)` places rows as written. `Fill(share, rows)` tiles its rows over
`share` of the rows left after the fixed ones, so the wand keeps `length_m` at
any texel size while heads, collars and pommels stay as drawn.

### Derived points

- Grip socket: centre of the rows that contain grip keys. The hand's
  `GripSocket` attaches here.
- Casting origin (muzzle): centre of the topmost row that contains emitter
  texels. Light, release glow and `tip_light` use it.
- Depth: each texel extrudes to the width of its row run, capped at
  `max_depth`, centred on the shaft plane. A 2-texel twig is square in
  section; a broad head stays slim. Faces of different depth are closed with
  bands, so every sprite is a closed surface.

## Rendering

Built once per family at start-up, recoloured per draw. Idle emitters are
painted (25 percent toward the glow colour, no emission). While casting
(`cast > 0.04`) emitter texels switch to the glow material with emission
`1.6 * cast`. The generic muzzle glow cube remains only for the review GLBs
(`VOXEL_REVIEW_ITEM`), which have no emitter material. No shader file changed.

Review override: `VOXEL_REVIEW_WAND_PIXEL_MM=<model mm>` rebuilds every family
at another texel size.

## Validation (`just test wand_`)

`wand_table_validates` checks version, texel range, unique ids, row widths,
undefined keys, colours, length range, depth range, a grip binding, an
emitter above the grip, one 4-connected body (no detached texels) and that
planned families carry no art. Other tests: one family per head with matching
names, twelve families, silhouette overlap below 0.72 between every pair of
styles, length held at 4.0-6.55 mm texels, closed and correctly wound
geometry, idle wands never glow, emitters glow when cast, deterministic mesh,
shaft material recolours, icons match sprites.

## Adding a family

Draw rows in the RON, set `status: Modeled`, run the ignored
`dump_wand_icons` test (icons at 4x and vertex counts in
`target/wand-icons/`), then capture natively with
`tools/Review-Artificer.ps1`. Mapping it to a seeded style needs a new head
index, which changes `WandStyle` clamping and the look bits; that is a save
review, not a data edit.
