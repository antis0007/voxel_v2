# Rune glyphs, colours and Rune Goggles

Every rune in `assets/runes.ron` has a carved glyph and a colour group. Players read
phrases as glyphs. Rune Goggles colour each glyph by its group; without them every glyph
is drawn in one plain ink. The grammar itself (forms, effects, qualifiers, events, costs)
is described in the header of `assets/runes.ron` and in `crates/content/src/runes.rs`.

## Where glyphs appear

| Place | Code | Notes |
| --- | --- | --- |
| Wand page, compiled phrase | `egui_ui/wands/rune_glyphs.rs` `strip` | Written order, event children included; a thin bar marks where a child phrase starts. Hover shows the rune tooltip. |
| Wand page, Rune Key | `rune_glyphs.rs` `key`, opened from `rune_desk.rs` | Every rune grouped by role, qualifiers split by what they attach to. Runes above the held wand's tier are dimmed, runes in the current phrase are underlined, and clicking a glyph appends its word to the phrase field. |
| Ritual circles in the world | `world/adventure/circles.rs` `circle_model` | Glyphs lie flat inside the rim, tops pointing outward, and light up as the ritual progresses. |

Not yet shown: the grimoire, bolts in flight, the "Bound:" line on the wand page (still text).

## Tooltip

Built from table data only, following `egui_ui/UI_RULES.md` (name plus at most three stat
lines, no prose):

```
Pierce                       name, in goggle colour when worn
Qualifier · Force            role, then colour group (omitted for Form and Event)
On: Form                     qualifiers only: the part it may sit on
Cost: ×1.5                   qualifiers scale cost; others show Mana and Upkeep
Tier: I                      or Requires: Ritual Circle for circle-only runes
```

A rune counts as circle-only when it is a circle form, or an effect or material with no
`card`.

## Drawing a glyph

`glyph` in a rune row is one of:

- `Strokes([(x0, y0, x1, y1), ...])`: straight segments on a 4 × 6 grid, y down
  (`GLYPH_GRID` in `runes.rs`). Every point must lie inside the grid.
- `Cipher(left, right)`: a qualifier's cipher stave. One vertical line `(2,0)-(2,6)` with
  `left` and `right` branches (0 to 3 each, at least one). Branch k sits at
  `y = 1.2 + k * 1.1` and spans 1.6 units. Branches rise on form qualifiers, drop on
  effect qualifiers and stay level on qualifiers that fit either. Only qualifiers may use
  `Cipher`.

`Table::strokes(id)` expands either kind to segments; renderers draw only segments.

`Table::parse` rejects:

- a rune without a glyph;
- a stroke off the grid;
- `Cipher` on a non-qualifier, or with more than 3 branches a side or none at all;
- two runes whose strokes come out identical (cipher staves are compared after expansion,
  so a form qualifier and an effect qualifier with the same branch counts do not clash);
- a `hue` with no `palette` entry.

## Letter choices

Forms, effects, materials and three events use Elder Futhark letters, chosen so the
letter's traditional meaning loosely fits the rune. The three remaining events use
Younger Futhark or late letters so no two events share a shape.

| Rune | Letter | Rune | Letter |
| --- | --- | --- | --- |
| TOUCH | mannaz ᛗ | SHRINK | hagalaz ᚺ |
| BOLT | tiwaz ᛏ | PLACE | othala ᛟ |
| ORB | jera ᛃ | PURGE | algiz ᛉ |
| SCATTER | raido ᚱ | STONE | perthro ᛈ |
| BEAM | eihwaz ᛇ | GLASS | wunjo ᚹ |
| RING | ingwaz ᛜ | COPPER | fehu ᚠ |
| DISK | dagaz ᛞ | ON_FIRST_HIT | uruz ᚢ |
| FIRE | kenaz ᚲ | AFTER_DELAY | nauthiz ᚾ |
| CHILL | isa ᛁ | ON_EXPIRE | ansuz ᚨ |
| SHOCK | sowilo ᛊ | ON_BOUNCE | yr ᛦ |
| VENOM | thurisaz ᚦ | ON_NEAR | ior ᛡ |
| DRENCH | laguz ᛚ | ON_RELEASE | os ᚬ |
| PUSH | ehwaz ᛖ | GROW | berkanan ᛒ |
| GRIP | gebo ᚷ | | |

Qualifier staves (left, right):

| Form qualifiers (branches up) | Effect qualifiers (branches down) | Either (level) |
| --- | --- | --- |
| PIERCE 0,1 · HOMING 0,2 · BOUNCE 0,3 · SWIFT 1,0 · HEAVY 1,1 · LASTING 1,2 · CURVE 1,3 · FALLING 2,0 · PHASE 2,1 · FORK 2,2 | CHAIN 0,1 · BLAST 0,2 · PULL 0,3 · KEEN 1,0 · STRONG 1,1 · SUSTAIN 1,2 | WIDER 1,1 |

Free staves: forms 2,3 and 3,0 to 3,3; effects 1,3, 2,0 to 2,3 and 3,0 to 3,3; either
type any pair except 1,1 (WIDER). 0,0 is invalid for any qualifier, since it needs a branch.

## Colour groups

`hue` names a group; `palette` in `runes.ron` sets its RGB. The UI shows the group name
in tooltips (`Arcane` is shown as `Mana`).

| Group | RGB | Used by |
| --- | --- | --- |
| Form | 222, 214, 192 | all forms |
| Fire | 238, 104, 52 | FIRE |
| Frost | 138, 206, 246 | CHILL |
| Shock | 246, 226, 92 | SHOCK |
| Venom | 148, 212, 72 | VENOM |
| Water | 64, 136, 236 | DRENCH |
| Force | 236, 150, 206 | PUSH, GRIP, motion qualifiers |
| Life | 92, 198, 112 | GROW, SHRINK |
| Earth | 192, 146, 92 | PLACE, materials |
| Arcane (Mana) | 172, 122, 238 | PURGE, reach/duration/binding qualifiers |
| Event | 82, 206, 192 | all events |

Without goggles: UI ink is `presentation::PAPER`; circle glyphs use `[0.78, 0.74, 0.66]`.

## Rune Goggles

- Item: `ArmorTier::Runesight = 11`, head slot, name "Rune Goggles"
  (`world/inventory.rs`). The armour tier byte is open-ended, so no save bump (same as
  Survey Goggles, tier 10).
- Recipe (`assets/recipes.ron`, hand): one row `X F X`, X = Crystal block, F = lumen fibre
  (resource 4).
- Effect: `Inventory::rune_sight()` is true while worn. `Adventure.rune_sight` copies it
  each tick, and `View.rune_sight` carries it to the UI.
- Art: 16 × 16 sprite 78 `rune_goggles` in `assets/icons/items16.txt` (brass frame,
  violet crystal lenses, a ᚲ chevron carved in each lens). Mapped in
  `egui_ui/icons/sprites16.rs`; the older pixel-grid icon in `egui_ui/icons.rs` remains
  as a fallback for other slots. World model colour in `mesh/models.rs`.
- Text: `rune_goggles` in `assets/descriptions.ron`.

## Adding a rune

1. Add the row to `assets/runes.ron` with `hue` and `glyph`. Prefer an unused Futhark
   letter; for a qualifier pick a free stave from the table above.
2. If the colour group is new, add it to `Hue` in `crates/content/src/runes.rs`, to
   `palette`, and to `hue_name` in `rune_glyphs.rs`.
3. Check: `cargo run -p engine_content --example check_runes -- assets/runes.ron`.
4. Look at it: the Rune Key on the wand page shows every glyph. For a capture, set
   `VOXEL_REVIEW_RUNES="<phrase>"`. Add `VOXEL_REVIEW_RUNEKEY=1` to open the key and `VOXEL_REVIEW_RUNESIGHT=1` to wear Rune Goggles; capture frame `OneMeter-002` with `VOXEL_TORCH_REVIEW_MODE=survival VOXEL_GRIMOIRE_REVIEW=wands ... --benchmark-torch-review`.
