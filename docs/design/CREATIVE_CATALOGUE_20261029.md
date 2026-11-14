# Creative catalogue and equipment

Implemented in the existing inventory screen, without changing survival keys,
item IDs or saves. Native visual quality and multiplayer interaction still need
fresh acceptance; source changes alone do not establish modded-Minecraft parity.

- Twelve icon tabs: Blocks, Terrain, Building, Nature, Lighting, Machines,
  Items, Materials, Weapons, Armour, Tools, Food. Tabs wrap with screen width.
- Compact 40/48-point slots replace labelled cards; names/stats remain on hover.
- Search matches every word against names or categories. Search and category
  filters apply consistently to blocks, equipment and bulk hotbar filling.
- Equipment: LMB selects the chosen quantity, RMB requests one, middle-click
  requests the stack limit, Shift requests a full stack. Hover + 1–9 assigns
  another hotbar slot without changing which slot is selected.
- Blocks: LMB/middle-click select a full stack, RMB requests one. Existing
  palette dragging is retained and routed to actual inventory assignment.
- The catalogue hotbar displays real inventory contents. Hand-mode hotbar
  dragging uses the existing inventory swap operation.
- Selection preserves displaced stacks. A full inventory rejects the staged
  transaction. Bulk filling commits the whole row or nothing. Selecting the
  same item never shrinks its existing stack.
- Local play and host commands call `creative/catalogue.rs`. The host checks
  creative authorization; the helper validates eligibility, quantity and slots.
  `/give` keeps its existing backpack-grant behavior.
- Sword icons and held blades share alloy tinting, preserving painted bindings
  and warm inlays. Pickaxe and bore glyphs retain the shared pixel palette and
  are used by the existing icon/held-art paths.

## Source ownership

- `crates/app/src/creative/catalogue.rs`: catalogue entries and staged selection.
- `crates/app/src/egui_ui/inventory/catalogue.rs`: navigation, slots and search.
- `egui_ui/inventory.rs`: catalogue integration; `egui_ui.rs`: palette drop.
- `main.rs` and `multiplayer/actions.rs`: action routing.
- `egui_ui/icons.rs` and `mesh/models/reference_sprites.rs`: shared equipment art.

## Native review remaining

Use a disposable creative world. Open Creative, visit all tabs, search multiple
words, select a tool and use it, swap hotbar slots, select and place a block,
drag a palette block, then fill a row. Repeat with a full backpack and confirm
no displaced stack disappears. Inspect swords/pickaxes/bores at actual slot size
and held scale. Repeat selection on a joiner with the same executable build;
verify host rejection in survival. No tests added or run under current defaults.

The existing `--benchmark-torch-review` harness supports `VOXEL_REVIEW_PANEL=4`
for native inventory captures. Record executable hash, resolution and UI scale;
an old executable cannot verify this change. See `docs/TRACKING.md` for actual
compile/build results and blockers.
