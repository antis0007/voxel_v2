# HUD layout

Where every in-play HUD element lives, why, and how to tune it without a
rebuild. Applies to the survival/adventure HUD during play, not to menus,
inventory pages or machine screens. Text rules stay in
`crates/app/src/egui_ui/UI_RULES.md`; motion and material rules in
`UI_MOTION_20261028.md` and `UI_PIXEL_MAGIC_20261028.md`.

Reference register (from UI_RULES and the arena HUD pass): vanilla Minecraft
minimalism, Jade's look-at panel, Xaero's minimap, FTB Quests' pinned quest,
plus the arena vitals and wand instrument with Noita-style pixel effects.

## Zones

Measured in UI points. The owner's display (2560x1440 at 150%) resolves to
1536x864 points; the layout must also hold at 1280x720 (1:1 points).

```
+--------------------------------------------------------------------------+
| [context, left]        [ FOCUS BAND, centre ]        [ NAV COLUMN, right]|
|  site modifier (Alt)    boss bar (when a boss is up)      minimap 112    |
|  hell biome             compass (tracked or Alt)          X · Y · Z      |
|                         look-at panel (Jade)              goal tracker   |
|                         enemy focus                       tool data (Alt)|
|                     [ NOTICE BAND, 16% height ]                          |
|                      journey / relic reward / receiver / world border    |
|                     [ TITLE CARD, 24% height ]                           |
|                                                                          |
|                                  +  crosshair                            |
|                              break progress                              |
|                                                                          |
|                         wand instrument (cooldown/Alt)                   |
|                         status effects                                   |
|                                                           pickup feed    |
| chat                     vitals plate + mana strip       subtitles       |
|                          [off] hotbar 1-9                quality notice |
+--------------------------------------------------------------------------+
```

1. **Focus band, top centre.** What the player is facing and aiming at:
   compass strip, then the look-at panel, then enemy focus. These are read
   while aiming, so they sit on the vertical axis of the crosshair. Each one
   stacks below the previous one through the region registry, so the look-at
   panel can never cover the compass again.
2. **Navigation column, top right.** Minimap, its coordinate line, then the
   pinned goal, then Alt tool telemetry. A goal is read at a glance and acted
   on through the map and compass, so it belongs with them. Right-aligned
   pins under the map are the convention players bring from FTB Quests, WoW
   and most RPG trackers.
3. **Notice band, upper third.** Event feedback that appears for a few
   seconds. Its home is a fixed line at 16% of the window height, clear of the
   crosshair and below the focus band. Notices only move down when the focus
   band actually grows into them (a tall machine readout), so they do not jump
   every time the crosshair finds or loses a block.
4. **Title card, 24% height.** Area entry (sky notice). Larger type, fades.
5. **Bottom centre.** Hotbar, vitals, status effects, wand instrument; owned
   by the arena HUD pass (`adventure.rs`, `hud_style.rs`, `progression_hud.rs`).
6. **Bottom corners.** Chat bottom left, subtitles and short system notices
   bottom right, as in vanilla.

## Placement rules

- **One owner per zone.** The top centre is the look-at panel's zone (UI_RULES
  §Look-at). Nothing persistent goes there; persistent elements go to a side
  column or the bottom.
- **Persistent weighs less than on-demand.** The pinned goal is visible at
  rest, so it has no frame line, a translucent plate (alpha 168), a 2 pt gold
  spine, and recedes to 82% opacity eight seconds after it last changed. The
  look-at panel, which appears only when asked for, keeps its full frame.
- **No instructional text at rest.** Toggle hints such as `[F6] Hide` show only
  while Alt is held, like every other Alt-detail readout.
- **Stacks are explicit.** A slot names the regions it must clear in `below`
  (top anchors) or `above` (bottom anchors). There are no implicit pushes and
  no cycles; a cycle would oscillate one frame at a time.
- **Never stack a transient element under a constantly changing one by
  default.** The look-at panel appears and disappears with every glance;
  anchoring notices directly below it would make them twitch. Give the
  transient element its own fixed line and list the changing one in `below`
  only as a collision guard.
- **Edge margins are 12 pt** on the right column and 8 pt at the top centre;
  gaps between stacked elements are 6 pt, 10-12 pt between unrelated groups.
- **Widths:** look-at about 260 pt, goal tracker 224 pt (narrower than the
  minimap column plus margin would allow on a 720 pt-wide window), notices up
  to 320 pt.

## Live tuning

Placement lives in `assets/hud_layout.ron`; the module is
`crates/app/src/egui_ui/hud_layout.rs`. Each element calls
`hud_layout::place(ctx, "<slot>")` for its anchor and offset and
`hud_layout::claim(ctx, "<slot>", rect)` with what it drew. Claimed rects stay
valid for one extra pass, so draw order does not matter (the compass draws
after the look-at panel but still pushes it down).

To iterate without rebuilding, point the game at a copy of the table:

```powershell
Copy-Item assets/hud_layout.ron $env:TEMP/hud_layout.ron
$env:VOXEL_HUD_LAYOUT = "$env:TEMP/hud_layout.ron"
$env:VOXEL_TORCH_REVIEW_MODE = "survival"
& target/release-fast/minecraft_style_voxel_prototype.exe --benchmark-torch-review --benchmark-width=2560 --benchmark-height=1440
```

The file is re-read within 250 ms of each save (`[hud-layout] loaded ...` in
the log; parse errors keep the previous table and print the error). The same
variable works for the engine workbench, because the server passes its
environment to the game: set it before `node tools/axiom-dev/server.mjs`, then
edit the file and `engine-cli capture <name>` after each change. Copy the
values you settle on back into `assets/hud_layout.ron`.

Slot names in use: `boss_bar`, `compass`, `look_at`, `enemy_focus`,
`goal_tracker` (survival goals and the settlement tracker), `tool_hud`,
`journey`, `reward`, `receiver_notice`, `world_border`, `title_card`,
`pickup_feed`. Claimed but not placed: `minimap` (its top still follows the F3
panel) and `subtitles` (the pickup feed stacks above it).

### Player editor (Alt+F6)

Players move elements without touching the table. Alt+F6 in play frees the
mouse and outlines every placed element on screen with its slot name. Drag to
move, right-click to return an element to the table position, Alt+F6 again
(or pausing, or anything that captures the mouse) to finish. Each drag is
stored as a per-slot nudge in `hud_offsets.ron` in the save directory and
added to the table offset before stacking, so a nudged element still clears
the regions in its `below`/`above` lists. Only elements on screen can be
picked up (no boss, no boss bar). The hotbar, vitals and wand instrument are
not slots yet, so they do not move. Code: `hud_layout::editor`.

## Findings (2026-11-14)

Evidence: survival torch review at 2560x1440 (`--benchmark-torch-review`,
`VOXEL_TORCH_REVIEW_MODE=survival`) and the engine workbench on seed
2738958700 (hills, view 6, survival, stone wall placed at z = -4 so the look-at
panel shows while the workshop lead is tracked).

**Before.** Every top-centre element was hard-coded, with no knowledge of the
others:

| Element | Old top (pt) | Problem |
|---|---|---|
| look-at panel | 8 | Covered the compass (8-42) whenever both showed, which is most of the opening because a fresh journal auto-tracks the workshop. |
| compass + destination strip | 8-63 | Strip at bar bottom + 3..21 overlapped by look-at and goal tracker. |
| goal tracker | 48 | Butted against the look-at bottom (about 48) with no gap, framed in gold, 248 pt wide, directly above the crosshair; showed `[F6] Hide` at all times. |
| world border | 34 | Inside the compass bar. |
| receiver notice | 40 | Inside the compass bar. |
| sky title | 56 | Over the look-at panel. |
| enemy focus | 74 | Fixed; collided with a tall compass + strip. |
| journey / relic reward | 76 / 78 | Fixed under the strip, over the look-at panel. |

**After.** Focus band stacks compass, strip, look-at with 6 pt gaps in the
workbench capture; the tracker sits 12 pt below the minimap coordinates,
right edge aligned with the map.

**Tracker placement A/B** (same exe, layout file swapped, no rebuild):

- *Under the minimap* (kept): grouped with the map and compass it is used
  with, right edges align, sits over sky or ceiling in most views.
- *Top left* (rejected): reads as a debug readout, disconnected from
  navigation, and collides with the hell-biome label (16, 52), site modifier
  (12, 82), lift panel and storage notice that already use that corner.
- *Right edge, mid height* (rejected): floats at eye level inside the
  viewmodel swing area and the subtitle column.
- Width 224 vs 196 tried live in the workbench: both wrap a typical action
  line identically, 224 keeps short goals on one line. Kept 224.
- Plate alpha 168 over bright overcast sky read as a grey slab with weak
  contrast on the muted tier line; raised to 196.

**Bottom stack** (arena HUD owner's area, observed; the affinity wrap, pickup
toast and hotbar size were addressed in the modpack-player pass below):

- At 1440p the affinity line `Mana regen 8.0x · Mana poisoning` wraps to two
  lines inside the vitals plate; the state words need to be shorter or the
  plate wider (`adventure.rs` `mana_affinity`).
- With a wand on cooldown, wand instrument + status chips + vitals + hotbar
  is about 29% of the window height at 720p, all in one 330 pt column.
- The pickup toast sits 10 pt right of the hotbar and is taller than it, so it
  reads as a tenth slot. A better home is a short feed above the subtitle
  column, bottom right.
- Hotbar slots render about 28 pt (47 px at 1440p), smaller than vanilla at GUI
  scale 3 (60 px); stack counts are hard to read. Any change must respect the
  16.3 px sprite step in `icons::paint_rows`.

**Workbench traps hit:** the server strips `VOXEL_*` variables from the game
process; `VOXEL_HUD_LAYOUT` is now passed through (`tools/axiom-dev/game.mjs`).
Benchmark worlds exit on `horizon_terminal_budget` when system commit is low
(peer builds); view 6 and waiting for builds to finish helped. A peer's
half-written content file can panic the game at startup; run the server from a
D: snapshot of the tree so code and assets match.

## Title menu

Same principles, applied to `egui_ui/main_menu.rs` (2026-11-14):

- Removed: the marketing tagline (UI_RULES text rule 6 bans exactly this
  "mine, craft, automate" line), the headframe emblem the owner rejected on
  2026-11-13, the decorative accent rule under the title, line icons on every
  button, and the mixed grid of tall card, button pairs and small pairs drawn
  as tinted glass boxes.
- Now: `AXIOM` in spaced capitals with a hard shadow, then one list of text
  rows in the vista's quiet band: Continue (world name inline, muted), Worlds,
  New World, Multiplayer; a gap; Settings and Quit in a smaller muted size.
  The default row carries a pixel caret (Enter's target); hover or focus adds
  the caret, a strip of panel glass and a 2 pt accent bar. Notes under the list
  (`Loading`, errors) align with the labels.
- Verified with `--capture-menu` at 1600x900, 640x480, 360x640, fresh (no
  recent world), long world name (elided with a tooltip) and busy.
- Open: the sub-pages (Settings, Multiplayer, Worlds, New World) still use a
  top navigation strip and a centred 720 pt surface pinned near the top, so
  leaving the title jumps the eye from the left column to the top centre. They
  should open in the same left column over the same vista. The multiplayer
  exporter fixture renders on a flat recess fill without the vista. Both live
  in `front_end.rs` / `online_browser.rs`, which another session was editing.

## Modpack-player pass (2026-11-14)

A cold read of the survival HUD and title menu as a new player who knows
kitchen-sink packs (Jade, JEI/EMI, Xaero's, FTB Quests). Each finding, the
fix, and where it lives. Items marked *open* were not changed.

### HUD

| Finding | Fix | Code |
|---|---|---|
| Goal tracker showed `3 / 2` on "Planks": it listed the recipe *inputs* (3 logs held, 2 needed) and never the goal's own count (0 / 8 planks), so a met input read as an overflowed, stuck counter. | The tracker leads with the target item and its count for "hold N" goals, then `From` and the inputs capped at what is needed (`2 / 2`, never `3 / 2`). Place and build goals show only their inputs: a block in a pocket is not a placed block. Alt shows `[Alt+R] Recipe`. | `world/goals.rs` (`View::target`), `egui_ui/goals_hud.rs` |
| A bare `100` beside the hearts read as an unlabelled stat. | The health numeral shows on Alt, at 25% health or below, or while mana-poisoned. The hearts carry health at rest. | `egui_ui/adventure.rs` `vitals_plate` |
| Minimap coordinates `0 · 100 · 2` had no axis names. | `X 0 · Y 100 · Z 2` (planet layer: `E · S` map axes), right-aligned to the map so long values stay on screen. | `egui_ui/map/minimap.rs` |
| Mana was a text line (`Mana regen 3.4x · Affinity`) unless the wand instrument was up (cooldown or Alt). It wrapped at 1440p. | A thin mana bar across the vitals width with the rate at its end (`3.4×`, plus `Affinity` / `Blight` / `Poisoning` off neutral ground). It never wraps. Hover gives mana, density and both regen rates. | `adventure.rs` `mana_strip` |
| The look-at panel put `20°C` on every brick and said nothing about mining. | Temperature shows when the cell is 10 °C or more off its climate ambient, or on Shift. A Jade harvest line: `Requires: Stone pickaxe` (red with `No Drop` when the held tool is under tier) and the hold time with the held tool. Hand-harvestable blocks show only the time, and only on Shift. Survival only. | `egui_ui/look_at.rs` (`Harvest`), `main.rs` (target probe) |
| Block outline nearly invisible at the crosshair: the authored sparkle lattice kept about 35% of edge pixels in the screen middle. | Coverage floor raised: `0.45 + 0.75·up + 0.40·edge` (was `1.10·up + 0.70·edge − 0.20`). Palette, dither, motes and the upward gradient are unchanged, and cells low in the view still break into dashes. | `shaders/outline_shader.wgsl` |
| Thin crosshair over pale stone and sky. | 2 pt stroke on a 4 pt, alpha-170 dark rim (was 1.5 on 3 at 120). | `egui_ui.rs` crosshair |
| Hotbar slots 26 pt; counts hard to read at 1440p. | Default hotbar size 1.25 (32 pt, a whole sprite step). Existing preferences keep their saved size; the "Hotbar size" slider is unchanged. | `preferences.rs` `HOTBAR_SCALE_DEFAULT` |
| A flat sword icon in the bottom-right corner repeated the held weapon. | Removed. The 3D viewmodel draws the held weapon in every mode. | `egui_ui.rs` |
| Pickup badge beside the hotbar read as a tenth slot. | A pickup feed bottom right, above the subtitles: one row per item, repeats merged, five rows at most, each fading after 3 s. | `egui_ui/pickup_feed.rs`, slot `pickup_feed` |
| The boss bar sat top left under the hell-biome label. | `boss_bar` slot at top centre (320 pt). Compass, look-at and notices stack below it. | `egui_ui/relics.rs` |
| Recipe lookup (R/U) only worked inside the inventory. | Alt+R / Alt+U in play open the recipe browser on the block under the crosshair, else the current goal's item, else the held item. Rebindable as `Show Recipe` / `Show Uses`. The chord keeps R (Cast) and U (Wand Atelier) unchanged. | `main.rs`, `egui_ui::show_recipe`, `assets/keybinds.ron` |
| No way for a player to move HUD elements. | In-game editor, Alt+F6 (above). | `egui_ui/hud_layout.rs` |

Corrections to the first read: the minimap does render terrain, waypoints and
a death marker (`✕`). The flat green square came from the review fixture's
flat world. The world map (`M`) and waypoint key (`N`) already exist.

### Title menu

| Finding | Fix | Code |
|---|---|---|
| `Continue  Workshop` gave no way to tell worlds apart. | A quiet second line under Continue with world type and age (`Wildlands · 2 h ago`), from the world browser's own summary. | `main_menu.rs`, `world_browser::summary` |
| `AXIOM` in the heading face at row colour read as a placeholder. | Three-step pixel extrusion in a dark accent tone under the letters, the drop shadow beyond it. No mark or emblem (owner rules: no rings, towers or flat marks). | `main_menu.rs` `wordmark` |
| Systems (machines, magic, space…) seemed hidden. | Already present: the New World form lists every system from `assets/systems.ron` as toggles. No change. | `front_end.rs` |
| The tagline was removed in the morning pass. | A peer restored a tagline (`One world, from valley to orbit`) noted as an owner request on 2026-11-14. Kept. | `main_menu.rs` |

*Open (need owner direction or content):*

- **Rotating title vistas** (factory line, hedron grove, orbit) to show the
  pack's range. The vista is an authored procedural diorama
  (`menu_scene.wgsl`, 4 moods × 3 poses of one province). New scenes are new
  art.
- **Version → changelog.** No changelog exists to link to; player-facing
  release notes are content the owner has to write.
- **Sub-pages in the left column** (from the morning pass): still a centred
  surface in `front_end.rs` / `online_browser.rs`.
- **Hotbar, vitals and wand instrument as layout slots,** so the editor can
  move them too.
- **Biome and day readout** on the minimap (Alt), the other common
  Xaero's line.

### Verification (2026-11-14)

Built from a D: copy of the tree (the shared tree did not compile: a peer's
`undercrown.rs` uses the test-only `flags` re-export; patched in the copy
only). Full `cargo check` of the app crate was clean for these files.

- Survival torch review, 2560x1440, same pose, before (morning build) and
  after: no `20°C` on the brick, tracker `0 / 8` then `From 2 / 2`, no bare
  `100`, no corner sword, `X 0 · Y 100 · Z 2`, larger hotbar with legible
  counts, heavier crosshair. The outline at the crosshair is now a continuous
  line where it was broken. Its cream tint still has low contrast on pale
  stone; a darker or inverted edge would change the authored palette (*open*,
  owner call).
- Workbench session (seed 2738958700, hills, survival): the minimap shows
  terrain with `X 0 · Y 37 · Z 0`, and the tracker sits under it.
- `--capture-menu`: title at 1600x900 and 360x640 with the extruded wordmark
  and `Wildlands · 2 h ago` under Continue.
- Not seen on screen: the mana strip (the review fixtures have not reached
  the magic unlock, and `/give` has no wand), the harvest line (stone bricks
  are hand-harvestable, so it shows only on Shift), the pickup feed, the boss
  bar, Alt+R/U and the Alt+F6 editor (the workbench sends no keys). These are
  compile-checked only.
