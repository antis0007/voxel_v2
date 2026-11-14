# Storage scanner implementation

The first playable stage adds a local storage terminal to every ordinary chest. Open a chest to search the combined contents of eligible storage within 16 metres. The original chest grid is in the **This chest** menu. Storage stays in its physical containers.

## Controls

- **Items:** search name/category words together, filter categories, sort by quantity, select an item, and take one, half a stack, one stack, an exact count, or all that fits. Shift-click or double click takes a stack; right click takes one. Selection and hover distinguish available stock from reserves; withdrawal buttons disable when nothing can move.
- **Sources:** select a source in the menu beside withdrawal controls to restrict withdrawal and highlight its world position for 12 seconds.
- **Containers:** exclude a chest, assign an item/category, set priority, reserve stock, and cap deposits with a target. A zero target means physical capacity.
- **Deposit:** smart routing to assigned destinations, partial stacks, category destinations, then general space. **More → Quick deposit:** backpack items already present in the network. **More → Deposit all:** eligible backpack items, leaving leftovers carried.
- **Restock:** refill existing unlocked hotbar stacks. **Consolidate:** deliberately merge physical storage; the button previews slots freed. View sorting never moves physical items.
- **Craft:** use unlocked backpack and eligible storage ingredients, preserving carried minimums and container reserves. Search/filter recipes, show ingredient availability and shortages, or choose **Craftable**. Choose 1–64 batches or **Max**. Counts are ingredient-limited; the complete output must also fit. Bench recipes still require a workbench within reach of the player. Failed requests publish no partial ingredient debits.
- **L** over an inventory slot locks/unlocks it for automatic sorting and transfers. Item-specific **Keep carried** minimums protect travel supplies.
- **Classic / Balanced / Builder:** world stack presets. Lower caps split player stacks and retain excess in saved recovery storage. **Recover** retries insertion when space becomes available.

## Implementation boundaries

Item identity uses the existing full encoded item identity and unique item references. Stateful equipment keeps a cap of one. Class limits live in `assets/stacks.ron`; a preset may include `overrides: [((kind, payload), count)]` for exact item overrides. These cannot raise unique-equipment caps.

Transactions revalidate loaded source positions and destination capacity on the authoritative world thread. Replayed network sequence numbers are ignored; gaps are rejected. Policy changes, chest contents, cache contents, and world stack rules replicate from the host. Machine processing buffers are excluded; storage-cache resource filters remain authoritative.

The aggregate index caches per-source contributions, updates changed sources, and polls topology/cache changes at most four times per second. Chest mutations trigger refresh immediately. The aggregate index does no inventory work on unchanged frames between polls. Physical consolidation is computed only when the index changes or the player requests it.

Inventory fields start at save version 45; the shared checkout currently writes version 46 with a separate Manyfold extension. Chest sidecar version 3 carries policies; readers retain older formats. Saved stack counts are validated independently of the currently selected cap, so reducing a preset does not truncate saved items. Breaking a chest with the loose-drop budget full routes its contents to recoverable overflow.

## Scope still open

This stage uses chest access rather than a separately crafted scanner block. New drawer tiers, named/private storage ownership, powered range extensions, automated request hysteresis, storage-fed blueprint placement, and equal-distribution machine gestures remain separate work. Reserve/target policies constrain manual transfers; they do not run a replenishment scheduler.

Material carrying capacity uses integer 1/64-cubic-metre units at the existing three world scales. Existing per-cell building quantities and resource remainders remain; this is not a completed cross-resolution material-ledger migration. Existing over-cap container stacks remain recoverable rather than being silently truncated. Preset migration normalizes player inventory; it does not forcibly repartition full physical containers.

Native evidence and focused test results are recorded below after validation. Unit-level authority/replay checks do not establish two-client transport or multiplayer usability.

## Focused validation, 2026-10-29

- Scanner: **21 passed**, including stale/unloaded sources, conservation, reserves/priorities, retry admission, and broken-container recovery (`target/storage-final-tests.log`).
- Cached-index benchmark: **1,279 containers / 34,533 slots**, 600 simulated frames, **one rebuild**, **52.42 ms total** in this debug-test build. This includes initial index/consolidation preview and bounded polling; it is not a gameplay FPS measurement.
- Inventory: **43 passed, 1 failed, 3 ignored**. The failure is the pre-existing canonical formatting check for compact entries in `assets/recipes.ron`; recipe values were not rewritten by this task.
- Block data: **5 passed**; world rules: **8 passed**; multiplayer replication: **6 passed**.
- Save/load final rerun: **15 passed, 0 failed** (`target/storage-snapshot-save-tests.log`). The earlier run passed 14 and hit a concurrent `spell_14` description-validation panic in wand migration; its owner corrected that description before this successful frozen-source rerun. Earlier version/header offset failures no longer occur.
- First native run executed the scanner fixture successfully: two containers, 1,105 copper preserved, 157 deposited, 50 withdrawn, one slot freed, sidecar roundtrip passed. The same invalid description then interrupted rendering. This partial receipt alone is not visual acceptance.
- Final native runs **passed at 1600x900 and 1280x720**, with screenshots inspected. Search, counts, source selection, all deposit commands, restock, and consolidation fit in the visible panel; secondary options and physical slot grids remain collapsible/scrollable. The chest UI now suppresses the overlapping hotbar overlay. Each run repeated the real transfer/conservation/sidecar checks successfully.
- Evidence: `target/storage-native-verified-1600/camera.png` and `target/storage-native-verified-1280/camera.png`; each directory contains stdout/stderr, frame data, and a manifest. Both use executable SHA-256 `E5AD8078B9F705FB22AFBB3B9B6E4E6D016E9E20A8FD7409B7F79A4225EE1818`.
- A separate copy of that verified executable is retained at `target/voxel-storage-scanner.exe` so later shared builds do not replace it.
- Concurrent source edits repeatedly interrupted validation. The first-stage executable was built from `target/scanner-review-source`; its scanner, inventory, policy, command, and UI source hashes matched the working files at that validation point. Frozen application check/build passed (`target/storage-snapshot-{check,build}.log`). Capture timing was extended to allow terrain streaming; these runs are visual evidence, not performance acceptance under competing builds.

The test logs are `target/storage-final-world-{inventory,block_data,storage,rules}.log` and `target/storage-final-multiplayer-replication.log`. Filesystem tests used a workspace-local temporary directory because the sandbox's default temporary-directory replacement was denied.

## Crafting follow-up

The scanner now shares cached, reserve-aware availability with its crafting preview. Recipe selection follows search/category filters, shortages show ingredient counts, and batch requests group repeated ingredients before staging the transaction. The original single-batch command remains compatible. The host validates the 1–64 batch limit, workbench reach, protected backpack slots, carried minimums, storage reserves, and full output capacity before publishing ingredient debits.

- Focused scanner suite: **24 passed, 0 failed**, including three new cases for batch conservation/protection, failed-batch rollback, and immediate reserve-cache refresh (`target/storage-crafting-tests.log`).
- The native review PowerShell script parses successfully. Local debug build passed (`target/storage-crafting-build.log`); the first native attempt executed the crafting transaction, then hit an unrelated `kinetic_vest` description-validation panic. A one-line copy correction removed hard-coded numeric prose while retaining the controls; no equipment mechanics changed. The subsequent build passed (`target/storage-crafting-rebuild.log`).
- Native crafting capture passed at **1280x720** (`target/storage-craft-final-1280/camera.png`), inspected for ingredient counts, batch selection, Max, Craft, and transfer controls. Three batches produced 12 Seared Bricks, leaving clay 14 / gravel 14 with reserve 2; the preview correctly shows 12 available of each and six further ingredient-limited batches. This is fixture execution and rendered-layout evidence, not manual input or two-client transport acceptance.
- The **All items** capture passed at **1600x900** (`target/storage-items-native-1600-verified/camera.png`), with the available quantity and all withdrawal controls visible. Both final captures use corrected Windows environment cleanup: `[NullString]::Value` removes disabled review flags instead of leaving empty-but-present values. The earlier 1600 capture unintentionally selected Craft, exposed this script issue, and is retained separately.
- The reviewed executable is retained at `target/voxel-storage-scanner-crafting.exe`, SHA-256 `DCA33F30EBFED191DF5DC883C9D51B49273AFD5F51AF2323AC3AA9E6F762781B`. It was built from the live shared checkout; the earlier first-stage executable and frozen source remain intact.

## Reproduce native review

Run `tools/Review-StorageScanner.ps1 -Output <new evidence directory>` after building the isolated target. It uses a disposable benchmark world/save, executes real deposit/withdraw/consolidation and persistence operations, opens the native scanner, records the executable hash, and captures HUD screenshots. Ordinary gameplay never installs the fixture.

Add `-Craft` to validate three storage-fed batches (12 Seared Bricks), assert the remaining ingredients and reserve, and capture the crafting tab. `-Width 1280 -Height 720` checks the smaller layout.

## Compact container layout, 2026-10-30

The previous captures showed functionality but the user rejected the panel's oversized presentation. The scanner now uses local 11-point body/control text, 18-point control height, and 26-point slots. Shared font choices and the global UI scale are preserved. The item grid fits twelve columns and scrolls after three rows. Sparse results shrink vertically. The backpack and separated hotbar stay visible; Sources, This chest, and More use menus, and the primary transfers occupy one row. Shift-click withdraws a stack. The selected item has a visible outline.

The first compact capture still filled the screen vertically because the outer window's scroll area retained unused height. The final window now sizes to content, with a screen-height limit for overflow. At 1280x720 the final Items fixture occupies approximately 572x514 physical pixels, versus roughly 700x720 previously, while showing the inventory. The Craft fixture occupies approximately 572x586 pixels, with all ingredients, batch controls, transfers, backpack, and hotbar visible.

- Local native builds passed (`target/storage-compact-build.log`, `target/storage-compact-fit-build.log`). No new automated tests were added or run for this layout change.
- Inspected native evidence: `target/storage-compact-fit-items-1280/camera.png`, `target/storage-compact-fit-craft-1280/camera.png`, and `target/storage-compact-fit-items-1600/camera.png`. The 1600x900 Items panel also fits with its full inventory and controls visible. Existing fixture transfer/conservation assertions passed; crafting again produced 12 bricks while preserving reserves. These are native layout captures and fixture transactions, not a hands-on interaction acceptance claim.
- Reviewed executable: `target/voxel-storage-compact.exe`, SHA-256 `1F4A1F0FEF4E216660F12DAD0EFBD9AD5111C6A63C1DAB9B347A05DB2068C46B`. Earlier scanner executables and screenshots remain separate.
