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
|  site modifier (Alt)    compass (tracked or Alt)          minimap 112    |
|  hell biome             look-at panel (Jade)              coordinates    |
|  guardian bar*          enemy focus                       goal tracker   |
|                                                           tool data (Alt)|
|                     [ NOTICE BAND, 16% height ]                          |
|                      journey / relic reward / receiver / world border    |
|                     [ TITLE CARD, 24% height ]                           |
|                                                                          |
|                                  +  crosshair                            |
|                              break progress                              |
|                                                                          |
|                         wand instrument (cooldown/Alt)                   |
|                         status effects                                   |
| chat                     vitals plate                    subtitles       |
|                          [off] hotbar 1-9   pickup        quality notice |
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

Slot names in use: `compass`, `look_at`, `enemy_focus`, `goal_tracker`
(survival goals and the settlement tracker), `tool_hud`, `journey`, `reward`,
`receiver_notice`, `world_border`, `title_card`. Claimed but not placed:
`minimap` (its top still follows the F3 panel).

## Findings
