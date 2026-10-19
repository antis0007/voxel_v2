# Gameplay HUD design audit

Scope: the attached combat HUD and the nearby gameplay overlays that compete with it. This is a source-backed UX audit, not a claim of in-game visual verification.

**Implementation status, 2026-10-18:** the numbered findings below are the historical baseline. Those line numbers describe the pre-fix working copy. See `VISUAL_STYLE_UPDATE.md` for the implemented resolutions, current code references, and rendered evidence.

## Traceability note

The attached image does not match the adventure HUD source inspected before the recent redesign: that source drew ten heart icons and a mana strip, but no `Health 100 / 100` pill or `LMB Strike guardians · E Wand recipe` footer. The current implementation is now in `src/egui_ui/adventure.rs`; the pill and footer themselves cannot be traced to this checkout from the available source. The image supports visual findings about those elements, while code findings below refer to the current HUD and nearby overlays.

## Findings

### 1. Screenshot-only: the pill looks like a web progress widget

The long capsule, smooth red fill, and rounded outer card have no voxel/material language; they read as a generic web meter pasted over the game. The bright red is equally strong at full health, so its color signals danger even at `100 / 100`. The card spends nearly the full width of the 360 px image for one value. These are visual findings from the attachment; the exact pill renderer is not present in the source path described above.

### 2. Screenshot-only: text and controls lack hierarchy

`Health 100 / 100` gives the category and value the same line and weight, while the footer compresses two actions into one small sentence with a middle dot. There are no distinct keycaps or visual grouping between combat and inventory. The footer also fails to distinguish opening a recipe from crafting it: the actual craft action is disabled unless a bench is available (`src/egui_ui/factory.rs:232-239`).

### 3. The health display competes with other bottom-center overlays

The vitals panel is anchored at bottom-center with a fixed `-125 px` offset (`src/egui_ui/adventure.rs:45-58`). The target-block card is also bottom-center at `-116 px` (`src/egui_ui.rs:2924-2935`). Those rectangles can overlap when a block is targeted. Journey notifications also use bottom-center at `-205 px` (`src/egui_ui/front_end.rs:641-650`); with a wand equipped, the vitals panel grows vertically to include mana, spell slots, and a spell label (`src/egui_ui/adventure.rs:98-144`), putting the two in the same area. The positions are independent constants rather than a layout that reserves space or stacks panels.

### 4. The screenshot’s action hint is not a reliable control instruction

The image says `E Wand recipe`. In the current input handler, E and Tab toggle the inventory palette (`src/main.rs:1120-1128`); the handler does not select a wand recipe. The visible HUD’s own spell help says `R ... T ...` only as trailing plain text (`src/egui_ui/adventure.rs:139-143`), with no keycap treatment or per-slot binding. This makes the screenshot’s hint look like a placeholder and leaves the actual action path unclear.

### 5. Health is drawn as a generic bar, with no damage-state treatment

The current meter is a 252×13 rectangle with one static red fill and ten dark tick marks (`src/egui_ui/adventure.rs:10-42`). Health is normalized and drawn directly (`src/egui_ui/adventure.rs:60-62, 96`): there is no recent-damage trail, loss animation, low-health threshold, or distinct critical state. The player does get a red screen-edge flash (`src/egui_ui/adventure.rs:161-171`), but it does not communicate damage direction or severity. The result is readable as a value, but weak at communicating combat urgency and change.

### 6. The health meter’s “segments” are only decoration

The nine internal divisions are always the same visual ticks (`src/egui_ui/adventure.rs:28-36`); the fill is a continuous value behind them. No rule connects a segment to a health unit or upgrade, so the marks add texture without teaching the player anything. The 13 px track is also too shallow to give the meter a strong visual hierarchy (`src/egui_ui/adventure.rs:11`).

### 7. The health panel uses arbitrary fixed sizing and spacing

The inner width and meter width are hard-coded to 252 px, and the meter height to 13 px (`src/egui_ui/adventure.rs:11, 57`). The panel does not derive its size from screen safe areas or available width. The spell row separately hard-codes 42 px slots and the mana strip’s 20 cells (`src/egui_ui/adventure.rs:98-118`). This is brittle on narrow windows and makes the HUD feel assembled from constants rather than designed as one responsive unit.

### 8. Mana has no label or numeric value

When a wand is present, the HUD draws a thin blue strip but gives it no `MANA` label or current/max number (`src/egui_ui/adventure.rs:98-113`). The health value is explicit while mana is color-only, so the two resources have inconsistent feedback and mana is less accessible.

### 9. The health header has weak information hierarchy

The row combines a small pixel heart, the generic word `HEALTH`, an uppercase rank, and the numeric value at similar visual scale (`src/egui_ui/adventure.rs:64-94`). Rank does affect maximum health (`src/egui_ui/adventure.rs:60-62`), but its name is not needed to interpret the meter and competes for attention. The value is the most actionable information and should lead; rank belongs in progression or character context unless it changes in the moment.

### 10. The screen edge damage flash is crude and non-directional

Damage feedback paints four uniform 5 px red edges around the entire content rectangle (`src/egui_ui/adventure.rs:161-171`). It does not identify where the hit came from, distinguish a light hit from a dangerous one, or relate the effect to the health panel. It reads as a generic overlay effect rather than a deliberate combat feedback language.

### 11. HUD panel styling is inconsistent across adjacent elements

The vitals panel uses an opaque dark fill and olive border (`src/egui_ui/adventure.rs:5-8, 51-55`). Shared gameplay cards use a translucent dark fill and a faint pale-green border (`src/egui_ui/presentation.rs:66-75`). The hotbar, target card, and status cards use that shared frame (`src/egui_ui.rs:2911-2935, 2957-2963`). This makes the vitals panel look like a separate widget rather than part of one HUD system.

## Overall assessment

The key design miss was treating this as a standalone bar instead of a combat HUD with a placement budget, explicit resource semantics, control affordances, damage-state feedback, and collision handling. The screenshot’s pill and footer look temporary; the current source also has concrete placement, hierarchy, responsiveness, and feedback gaps that would remain even after changing the bar’s shape.

---

## Survival Inventory, Minimalist HUD & HWYLA Update (2026-10-18)

### 1. Ownership Boundaries & Save Compatibility
- **Authoritative Survival Inventory (`src/world/inventory.rs`)**:
  - `World::inventory: SurvivalInventory` owns the 9-slot survival hotbar (`hotbar`), 27-slot backpack (`pack`), 4 armor slots (`equipment`: Head, Chest, Legs, Feet), 2×2 hand-crafting grid (`craft_grid`), held cursor stack (`cursor`), dropped world stacks (`dropped`), and sub-unit resource remainders (`resource_remainders`).
  - `CreativeState` (`src/creative.rs`) retains its separate 9-slot creative hotbar and searchable block catalog (`page == 4` on the `E` screen), so creative building tools and `torch_review` drag-and-drop tests remain cleanly separated from survival item stacks.
  - `World::sync_from_inventory()` and `World::reconcile_legacy_to_inventory()` keep `Expedition::resources`, `Factory::kits`, `Adventure::rations`, and `Adventure::weapons` synchronized with `SurvivalInventory` so existing workbench, machine, and combat systems interoperate losslessly.
- **Save Format & Legacy Migration (`src/world/storage.rs`)**:
  - Save header magic `VXWSAV16` (with `version_thirteen = version >= 13`) serializes `SurvivalInventory` and dropped world stacks in `session_footer` while keeping `session_header` at its exact 176-byte layout for `background.rs` and `library.rs`.
  - Saves from `V1`–`V12` load cleanly via `World::migrate_legacy_save_to_inventory()`, which populates starter survival stacks and reconciles legacy resources, machine kits, rations, and weapons into slots without item loss.

### 2. Minimalist Minecraft / Noita HUD & HWYLA Block Inspection
- **Split Vitals Row (`src/egui_ui/adventure.rs`)**:
  - Positioned directly above the centered 9-slot hotbar (`Align2::CENTER_BOTTOM, [0.0, -76.0]`), matching the width of the hotbar (`9 * 48 + 8 * 6 = 480 px`, scaling down on compact viewports).
  - **Left side**: 10 pixel-art health hearts (full, half, empty) with low-health jitter and damage flash, plus an armor shield row directly above health when armor is equipped.
  - **Right side**: 10 pixel-art hunger haunches draining right-to-left (full, half, empty) with golden eating bounce (`eat_timer`), low-hunger wobble (`<= 25%`), and crimson starvation pulse (`hunger <= 0.0`).
  - Verbose text status banners were removed so the icons and world state teach the player directly.
- **HWYLA / Waila Top-Center Block Inspector (`src/egui_ui.rs:target_name`)**:
  - Anchored at `Align2::CENTER_TOP, [0.0, 10.0]` so it never collides with the bottom-center hotbar or vitals row.
  - Displays a 3D isometric voxel icon of the aimed block, its name, and a concise category or `[F]` interaction tag (`Workbench`, `Smelter`, `Supply cache`, `Building block`, etc.).
- **Pickup & Overflow Feedback (`src/egui_ui.rs:pickup_toast`)**:
  - Anchored at `Align2::RIGHT_BOTTOM, [-18.0, -78.0]` with the item icon and `+N Item` or amber `PACK FULL` when mining with a full inventory.

### 3. E-Key Survival Inventory & Modded QOL (`src/egui_ui/inventory.rs`)
- **Layout**:
  - 4 equipment slots with armor silhouettes (`Head`, `Chest`, `Legs`, `Feet`) + dynamic pixel-art player preview reflecting equipped armor tiers and `DEF %`.
  - 2×2 hand-crafting grid + output slot + clickable compact `Recipes` grid with hover ingredient counts (`have/need`).
  - 27-slot main backpack (3×9) above the 9-slot hotbar (`1–9`).
- **Interactions**:
  - Left-click pickup/place/merge/swap, right-click half-stack split or place-single, drag-and-drop across slots, `Shift+Click` quick transfers (hotbar ↔ pack ↔ armor ↔ craft grid), and `1–9` number-key hotbar swaps while hovering any slot.
  - Responsive slot sizing ensures all 4 armor slots, 2×2 crafting grid, 27-slot pack, and 9-slot hotbar fit cleanly inside both `1600×900` and compact `800×450` viewports.

### 4. Verification Evidence
- **Automated Unit Tests**:
  - `cargo test --offline` passes all 10 unit tests (`world::inventory::tests::*`, `world::storage::tests::save_and_load_preserves_survival_inventory_and_migrates_v12`, `creative::tests::*`).
- **Rendered Visual Captures**:
  - Normal (`1600×900`): `target/survival-review-normal/OneMeter-000.png` (full vitals + HWYLA pill), `OneMeter-001.png` (partial hunger + eating bounce + pickup toast), `OneMeter-002.png` (starvation state + `PACK FULL` overflow guard), `OneMeter-003.png` (`E`-key survival inventory with 2×2 crafting and armor preview).
  - Compact (`800×450`): `target/survival-review-compact/OneMeter-000.png` through `OneMeter-003.png`.

