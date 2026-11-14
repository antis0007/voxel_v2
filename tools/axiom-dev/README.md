# Axiom Dev Panel

Run from this checkout with Node.js 22 or newer.

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\Open-AxiomDev.ps1
```

Keep the launcher window open while editing or playtesting. It prints the local browser URL if the browser does not open. Use `-NoBrowser` to open the printed URL yourself. The server listens on a random loopback port and requires the session key in the printed URL. No packages, accounts, or AI services are required.

Unattended improvement, world system switches and the live-engine loop: [AUTOPILOT.md](AUTOPILOT.md).

## Find the authoritative catalog

`node tools/axiom-dev/catalog-cli.mjs list` lists the native-validated authoring
contracts. `describe tool-materials` returns the content path, Rust schema, runtime
reader, checker and activation rule. Edit values in that catalog, not a new copy.
Forge's validator build list and save-time identity guards now derive from
`catalog-contracts.mjs`. Rust remains the authority for schema and semantics.

This registry covers spells, tool parts/schemas, tool materials, reactions and Fabricator processes.
It is not a claim that every engine feature is registered or hot-reloadable.
Portal validation retains its separate protocol. Adding a contract requires a native
checker and reader, then the contract tests; adding an ordinary row does not.
Contract edits invalidate checker receipts, so rebuild validators before publishing.
Gather Context also includes the relevant contract as read-only, revision-pinned
evidence in the existing panel and AI requests. Autonomous repair cannot edit the
contract or validator infrastructure to weaken its own checks. Native dependency
scanning currently covers core, content, audio and tools; a new owning crate needs
its dependency coverage added as well as its contract.

Fabricator parsing, planning and resource pickers consume a generated native
metadata projection. `node tools/axiom-dev/build-validators.mjs` regenerates it and
refreshes checker receipts. Workshop previews now use native validation. Existing
resource 24 (feathers) is supported throughout authoring; 25 is rejected. Restart
Forge and reload the browser after schema/metadata changes. See [Workshop](WORKSHOP.md).

## Live engine workbench

**Playtest → Engine workbench** starts isolated playable worlds, inspects the
running engine and exposes live ambience, mana recovery, sparkles, mist and colour
controls. Compare native captures or recovery samples, save named presets, undo
or reset previews. **Models → Apply to game** replaces a loaded factory voxel
model without rebuilding. These previews are separate from saved content.

Agents use the same capabilities through `node tools/axiom-dev/engine-cli.mjs`.
Start with `help`, `describe ambience` and `inspect player location`; results are
compact JSON with native revision checks. **Gather Context** can attach actual
world/player/location observations and the launched binary's identity alongside
source evidence. See [Engine Workbench](ENGINE_WORKBENCH.md) for exact commands,
units, supported live edits, undo scope and rebuild requirements.

## Catalog and studios

- **Field studies**: `node tools/axiom-dev/field-study-cli.mjs inspect` lists the eight bookcase projects, prerequisites and supply costs. `preview Lesson` and `apply Lesson` read JSON with `xp` and ordered `counts`; apply requires the preview's revision and approval. The shared workspace writer keeps backups and rejects stale edits. `assets/field_studies.json` is also available in Workspace. Rebuild to apply changes. Learned projects accept repeat supply batches for one quarter of their XP price (rounded down, minimum one).

- **Graph → Dimensions & portals** authors same-dimension portal openings and typed Fold links, with contextual coordinates, facing, aperture, return rules, route preview, undo/redo and revision-checked publication. It saves `assets/portal_layout.ron`, which the game uses after a rebuild. Start with `node tools/axiom-dev/build-portal-validator.mjs`. See [portal layout authoring](PORTAL_LAYOUT.md) for runtime limits and the shared human/AI CLI. The default catalog is empty; custom cross-dimension transfers and new generators are not provided by this flow.

- **Wand Lab** (top bar link, or `/wand-lab`) is the magic workshop. Every number comes from the game's own deck evaluator through headless `--wand-lab` processes (no window, world or save); derived views say when they extrapolate.
  - **Deck**: drag spell cards in, reorder with drag or arrow keys, lock slots, undo/redo, share builds as `WL1.` codes, and watch the cast order animate on the cards and in the cast strip.
  - **Performance**: damage per second (peak and 30 s held-fire average), mana per second against charge, time until the pool runs dry, a per-bolt cast timeline, the mana-pool curve, side-view bolt flight from each shot's speed, life, gravity, bounces and path, cast-block modifiers, and time/casts/mana to kill every `assets/bestiary.ron` enemy (optionally several in a row).
  - **Compare**: up to six builds overlaid as cumulative damage over 30 s, with the best value per metric highlighted.
  - **Optimise**: deterministic native hill climbing for sustained, peak, burst or per-mana damage, optionally requiring endless sustain; locked slots stay, the pool is the palette filter.
  - **Balance sheet**: every castable projectile alone and every modifier before a reference bolt, sortable, with bars and outliers coloured against the class median. **AI proposals** suggests stat changes from the sheet; nothing applies until added to the draft.
  - **Spell editor**: edit spell numbers and **Score draft**. A second lab process loads the draft with `--spells=<draft>`, so every view re-scores without a rebuild and tiles show the change against the shipped table. **Save to spells.ron** writes it with a backup; rebuild to ship it in game.
  - **AI designer** proposes decks, scores each natively and refines once. **Equip in playtest** mints the build into the open Engine-workbench world's hotbar (`{"op":"wand","build":{...},"equip":true}`). The AI features need `AXIOM_DESIGN_ENDPOINT` and `AXIOM_DESIGN_MODEL`.
  - Damage is a direct-hit estimate: status, crits, chains, splash and shuffle order are not modelled. The lab needs a game build made after `crates/app/src/dev_panel/wand_lab.rs` last changed. Builds live in `target/axiom-dev/wand-lab-builds.json`.
- **Workspace → assets/spells.ron → Graph** opens Spell Wiring. Choose any spell, edit its Cast, Delivery, and Library blocks, then **Stage changes** and review the file diff before saving. The same spell rows supply Gear categories and the Field Guide's Spell Index. Rebuild the game to use saved changes. Spark keeps its separate flight preview.

- **Catalog → Items → Material settings** opens the material popup. The same popup is available on material records in Workspace and the Reference Map. Adjust color, surface finish, physics, and simulation values directly, or use a quick mutation. Valid edits autosave with backups. The popup keeps pending values available if the saved file changes underneath it.
- **Make material variant** copies the source properties into a new draft. **Create & save variant** appends the block ID and material row together, leaving existing IDs in place. The popup shows the remaining ID capacity. New block IDs need a rebuild and relaunch. Variants inherit authored properties. Native machine logic, special models, and world generation rules still belong to their original block IDs.
- **AI mutation** uses the configured design model and read-only project context. Describe a change to generate validated settings. The popup shows the rationale and source locations. On an existing material, **Generate & autosave** applies the result. In variant mode, the generated settings go into the local draft until you create the block.

- **Catalog** groups records into **Crafting**, **World**, **Art**, and **Project**. The browser has **Slots** and **List** views. Drag the divider to resize it or use the arrow keys while the divider is focused. **Layout** controls density, pixel headings, and the browser side. These options are saved per checkout.
- **Recipes** opens the recipe book with input slots, quantities, and edit actions. It includes crafting, machine assembly, and powered Fabricator recipes. Other processing and fuel routes appear in the native game index.
- **Recipes → + Fabricator Recipe** creates a powered process from existing resources. Set up to three inputs, the output, seconds per batch, and power. The draft appears in the book and Recipe Tree. Use **Review & save**, then rebuild to use it in-game. **Edit Process** opens the catalog row for tuning. Recipe IDs are saved in placed machines, so keep existing rows in place and append new ones.
- **Recipe Tree** shows every indexed output and its recipe alternatives. Open an output to follow Inputs or Uses, change the target quantity, and select a route per ingredient. The Inputs plan accounts for whole batches and reuses surplus. External supply and cycles are shown separately. **Copy Context** captures the selected plan and source paths.
- **Crafting Web** opens from Recipe Tree, a recipe, or an item. Focus any indexed item and show inputs, uses, or both. Adjust the number of hops to explore alternate crafting, assembly, and Fabricator routes. Drag card headers to arrange the canvas. Drag a crafting wire onto another item to reconnect it, or drag an item’s arrow grip onto a recipe to add it as an ingredient. The inspector offers the same actions without dragging. Assembly and Fabricator nodes open their own definitions.
- **Copy recipe** or Ctrl+C captures a node as a template. **Paste as branch**, Ctrl+V, or **Branch route** opens the recipe creator with the ingredients, pattern, output, and description filled in. Pan by dragging empty space (or hold Space and drag); zoom with Ctrl+wheel, the − and + buttons, or the + − 0 keys; **Fit** or F frames every node. Nodes snap to an 8 px grid. Hovering or selecting a node lights its wires and dims the rest. Arrow keys move focus between linked nodes, and / opens Find item. Right-click empty space for view and route actions. **Copy tree** puts an item’s default production tree on the clipboard as compact text, one recipe per line, for AI or CLI prompts. **Swap ingredient in tree…** replaces one ingredient across every editable recipe below an item: pick the ingredient, pick the replacement, review the affected recipes, then apply. Each recipe is one undo step in the draft; nothing saves until review. Right-click a card, or press Menu or Shift+F10 on its header, for the same actions plus Focus. The copied recipe survives closing the Crafting Web, and Copy recipe also puts its product on the system clipboard so it pastes into Wiring. Pasting a copied item into the Web starts a new recipe that uses it. Change the ingredients or pattern to make a distinct route. Drop an item on **+ New route** to start with that ingredient. Changes use the recipe draft’s undo history and save review.
- **Audit all routes** scans the indexed catalog. The inspector checks missing descriptions, unindexed items, supply dependencies, sprite mappings, comparable route costs, and bounded single-input conversion loops. It can draft a factual description or cap a detected loop’s yield. **AI improve recipe & art** attaches the recipe, mapped sprite sources, and balance and downstream context for a coordinated proposal.
- **Generate AI balance proposal** uses the configured design model to propose changes against nearby alternatives and downstream recipes. Review the before and after route and rerun findings in the inspector before adding a proposal to the draft.
- Right-click a slot for its actions. On a focused item, **R** opens Recipes, **U** opens Uses, and **A** toggles a bookmark. Drop records into **Bookmarks**, then drag bookmarks to reorder them.
- Recipe icons in the book and tree can be dragged into the **Item Tray**. Right-click or press **Shift+F10** for item actions on recipe icons and Wiring material cards. The same item opens its recipe tree, editable sprite, or AI scope. In Wiring, **Use Tray Item** arms it for an input or the output.
- **Crafting → Items → + Food** starts with an existing food. Set its nutrition and tint an existing sprite for its icon. Review and create the food, then use **+ Recipe** on its item card. The creator appends a stable food ID and saves the registry, food, and sprite together with backups. Rebuild before using the new item in-game.
- Drop an item on **+ Recipe** to set the output. In the editor, drag palette items into inputs or the output. Drag filled pattern slots to move or swap them. Right-click or press Delete to clear a slot. The same palette is available in **Balance Recipe** from a recipe’s context menu.
- **Art → Sprites** shows categorized thumbnails for individual editable sprites. Select a thumbnail to paint it. **Edit sprite** on an item or recipe also jumps directly to the sprite.
- **World → Animals** puts stats and habitats on the animal card. Edit habitats by biome and spawn weight. Drop a bookmarked animal onto a biome to start a habitat, or use **+ Animal** from the biome. **New animal** can reuse a cow, boar, deer, or marsh fowl body. Cave salamanders use separate karst placement rules.
- **World → Biomes** shows the winning climate family at the chosen temperature and moisture. Click to sample the map and open **Terrain and rock rules** to tune elevation, slope, wetness, rock requirements, and preference. The map compares the family rules at sample conditions. It is separate from a generated world map.
- **World → Deposits** edits the generator’s coal, iron, copper, tin, and crystal deposit rules. Use the depth diagram and distribution controls for the selected deposit. Related ore items link back to the editor.
- **Creatures** groups the rest of the bestiary by role. Right-click to edit stats and disposition or edit spawn rules from the detail card.
- **Locations** links ruin definitions, generated site sources, and saved building plans. **Build a location** opens Anchor Studio for building design and game blueprint export. Use the separate Anchor Studio launcher once if the local compiler is missing.
- **Models** shows native factory voxel models and supported GLB previews. Select a voxel model to paint layers, add colors, edit glow, and orbit the 3D preview. GLB previews show static geometry. Use Workspace to replace a GLB.
- **Hand poses** edits wand presets and the shared held-item settings for blocks, torches, blades, and the empty hand. Drag the preview to move the hold point or use centimetres and degrees. Preview geometry is static. The native game applies animation and equipped art.

Catalog changes use the same **Review & save**, backups, and file conflict checks as the graph. Embedded assets and source take effect after rebuilding Axiom.

## Development graph

- **Graph → Wiring** opens a recipe as a live 3 × 3 pattern or shapeless ingredient list. Click a cell to search for its material, or drop an item from the palette. Drag a filled cell or its wire to move or swap it. Drop items onto the board to make loose cards, or use **+ Material** to make several at once. Drag a card to a cell when it belongs in the recipe. **Ctrl+C / Ctrl+V** copies a material to a cell or the board, and copies the full input pattern from the recipe title. **Delete** clears a cell or removes a card. Loose cards and layout stay in the open browser session. **Undo**, **Redo**, and **Review & Save** act on the recipe draft.
- **Graph → Advancements** connects the actual tree parent fields. Drag a parent's output onto a child's input, or drag a connected line to another parent's output. The inspector edits the name, target, and tree parent, and rejects parent cycles. A parent controls placement in the records tree. The trigger controls unlocking. Existing IDs and row positions stay fixed.
- **Wiring → Action Blocks** lets you drag or click actions into a stack, reorder them, and preview the combined result. Place, remove, swap, or replace ingredients, rotate or flip a shaped recipe, and change its output. **Add to draft** makes one undoable edit. Use **Review & Save** to publish it. **Share with AI** imports or exports a command for the open recipe.
- Drag node headers to arrange either canvas. **Reset Layout** restores the default arrangement. Node positions are session-local and separate from the recipe and advancement data.
- **Reference Map** opens on Axiom’s systems. Select a node to follow its folders, files, records, and links. Drag the canvas to pan. Scroll to zoom or use **Fit**. The generated code and catalog links are navigation.
- **Reference Map → System Boards → + New system board** gives any feature a control panel. Name it, then pin catalog settings, simple Rust constants, records, or project files. You can also right-click a node and choose **Pin to System Board**, or drag it onto a board node. Choose **Open controls** to search the pins, edit values directly, or open a record in its inspector. **Review board layout** saves the pins and notes as a Markdown file in `docs/axiom-notes`.
- **Review system changes** shows the full drafts for up to five pinned files together. The bundle uses existing backups and revision checks, and appears in **Drafts → Candidate bundle history**. **Ask AI about this system** attaches the board and up to four pinned files to the AI workbench. Use the existing Playtest tools to try draft overrides where supported or rebuild for embedded changes.
- Right-click a node to create a linked note, connect it to an existing note, or attach it to the AI workbench. Drop a source node onto a note node to add the link to the note draft. **New Tinted Food Variant** on a food node opens the variant creator with the source food and icon selected.
- **Ctrl+K** finds files, notes, catalog records, fields, and simple Rust constants. Try `Spark damage`, `basin blend`, or a filename. **Find a setting** limits the results to tunables.
- The **Inspector** edits the selected node. Boolean settings use switches. Common numbers have exact inputs, nudge buttons, and units where known. Recognized normalized RGB tuples have a color picker. **Source** opens the entire file for arbitrary edits. Rust constants with computed values, duplicate names, or version gates are edited through Source.
- Use **Review & save** or Ctrl+S for the selected file. Graph and Workspace share the same drafts, undo history, backups, and conflict checks. Rebuild for changes to embedded data and source.
- **New note** creates Markdown in `docs/axiom-notes`. Use `[[Another note]]` to link notes or **Connect file or setting** to insert a project link. Linked settings are editable in the note’s inspector and each linked file has its own review button. Saved backlinks appear on the graph. Use **Update links** to refresh a note draft after typing links manually.

## Create an idea

Choose **Create** on the graph or type **Create recipe**, **Create spawn rule**, or **Create advancement** in Ctrl+K.

- **Recipe** uses searchable existing items. Choose hand crafting or a workbench, list ingredients for a shapeless recipe, or paint slots for a shaped recipe. The form checks names, grid size, stack quantities, and conflicts with existing recipe inputs.
- **Spawn rule** selects an existing creature, realm, time, optional biome, and relative weight. The new rule is appended to preserve the ordering of existing rules.
- **Advancement** selects an existing stat or level trigger, sets a target, and optionally links to a parent advancement. The panel creates a stable ID from the name.

**Create draft** adds the new row to the graph. **Review & save** writes it to the catalog. For recipes, **Playtest recipe draft** launches the existing panel build with the draft loaded through Axiom’s recipe override under `target/axiom-dev/recipe-preview`. Open or create a solo world there to try the recipe. Close the game and relaunch the preview to pick up a newer draft. Spawn rules and advancements are embedded catalogs and take effect after a rebuild.

## Tool-material progression

`node tools/axiom-dev/material-progression-cli.mjs inspect copper` shows the existing
harvest gates. `preview copper 2` compares gained/lost access before a native-validated,
reversible save. Use an optional tool schema to distinguish mining classes. The
[progression workbench guide](MATERIAL_PROGRESSION.md) covers review, apply, revert,
HTTP access and rebuild requirements. This slice does not add a new graphical panel.

## Bronze alloy workbench

Open **Reference Map → Materials and alloying**, then **Bronze alloy workbench → Open controls**. The existing System Board pins bronze tool stats and the copper/tin volume counts. Edit the values and use **Review system changes** to publish both catalog drafts as one native-validated bundle. The graph follows bronze's tool-material → solid block → molten product → reaction dependencies. Existing IDs and indexed tool rows stay in place.

Humans and AI can use the same publication path from the checkout:

```powershell
node tools/axiom-dev/alloy-cli.mjs inspect bronze
'{"material":"bronze","changes":{"durability":480},"batchCounts":[2,1]}' | node tools/axiom-dev/alloy-cli.mjs preview
```

Repeat the request with the preview's `versions` and `approval` to `apply`. It rechecks native semantics and saved revisions before writing through Candidate bundle history. Revert uses that existing history and refuses changed files. Keep material/part/schema IDs and rows in place; saved equipment uses them.

Bronze kiln quantities and its label now derive from the same molten batch recipe as the smeltery. Kiln work/power remains 24 ticks/10 power; its electrical process does not acquire physical temperature simulation from this edit. Steel's separate kiln mode is unchanged. The command adjusts existing block-linked tool materials with an existing batch reaction; it does not create native blocks, ores, resources, sprites or new mechanics. Rebuild and relaunch the game after saving. See [the implementation and review guide](../../docs/engineering/forge-material-alloy-authoring.md).

## AI workbench

**Improvement Queue** runs up to three independent briefs in order. Open **Set Up Improvement Run**, select Engine, Optimization, or Art refinement, and choose an indexed source and a focused brief for each. The selected provider receives each task and the gathered source evidence. Engine and optimization tasks use Audit & Repair. Optimization results include a proposed workload and metric for measuring the change. Art tasks propose a small revision for visual approval.

The queue runs on the local panel server, so it keeps working if you switch tabs or close the browser. A blocked task leaves the other lanes running. **Stop Queue** cancels the active pass and skips remaining tasks. The latest queue stays reviewable while the server is open. Use **Review Candidate** to inspect the result, **Stage Drafts** to work on it, and **Open Editor** to tune it. Sprite proposals show before and candidate previews ahead of staging. The queue does not publish changes, and native builds, performance measurements, and visual approval are separate evidence.

From the checkout, run `node tools/axiom-dev/wiring-cli.mjs help` for the command schema. Use `recipes`, `items`, `spells`, and `advancements` with an optional search term to find exact names and IDs. Use `recipe "Anvil"` to inspect its cells and output.

Pipe a recipe stack, spell stack, or advancement connection to `preview`. It reads saved catalogs and returns before and after values. To publish the reviewed result from the command line, send the same request with the returned `approval` value to `apply`. It recomputes the change and checks the saved revision before writing with a backup. Spell Wiring and the command line share the same field schema and validation. For a recipe draft in the browser, paste the recipe command into **Share with AI** and preview it there.

```powershell
'{"recipe":"Anvil","blocks":[{"op":"rotate"},{"op":"mirror"}]}' | node tools/axiom-dev/wiring-cli.mjs preview
'{"child":"place_block","parent":null}' | node tools/axiom-dev/wiring-cli.mjs preview
'{"spell":96,"blocks":[{"op":"set","field":"cost","value":16}]}' | node tools/axiom-dev/wiring-cli.mjs preview
```

**Audit & Repair** gathers context and uses attached sources as its editable scope. With no attachments, the runner selects up to three saved implementation excerpts from the gathered evidence. The first pass selects one defect with a unique source quote, trigger, and impact. The next pass proposes a bounded repair. The four-pass option audits the candidate excerpts and uses the fourth pass to repair a concrete follow-up defect, if one is found. **Open Defect** goes to the finding in the editor. With staging enabled, syntax-checked repairs can enter the draft queue automatically. Native changes require manual staging, and project saves use bundle review.

Automated callers can POST a task to `/api/agent/context`, then POST the same task with the returned `contextId`, `mode: "repair"`, `sources: []`, `passes: 4`, and the provider and model to `/api/agent`. Poll GET `/api/agent` for that run ID. The result includes `autoSources` with saved anchors, the finding, the proposal when available, and validation evidence. A failed repair can still report a finding.

The [Axiom Dev Toolkit specification](../../docs/plans/AXIOM_DEV_TOOLKIT.md) defines the proposed idea-to-candidate workflow, feature backlog integration, and model feedback loop.

- **Import Mod** accepts a description, a local JAR up to 48 MiB, or both. A feature name or class name in the description helps prioritize the evidence. The local scan fingerprints the archive and samples loader metadata, assets, data files, and JVM field and method instructions. **Inspect Class** examines a specific class. With a JDK on PATH or `AXIOM_JAVAP_PATH` configured, it includes disassembly. Otherwise it shows instruction references. Inspection never loads the mod into Minecraft.
- **Texture Study** reads a PNG from the selected JAR or a local file. It previews the reference locally and creates a fresh machine panel, ore vein, or rune with a similar palette. Export the generated PNG or stage it in an existing Axiom icon slot for review. A Mod Port run can include the texture path, dimensions, and palette.
- **Model Study** converts vanilla JSON cuboids and GeckoLib geometry to static Axiom voxel prototypes. Orbit the result, download the native text VOX, or stage an 8³ draft in an existing factory slot. **Playtest Model** loads that draft from a separate world folder when the playtest executable has preview support. Build once to get the support, then relaunch for subsequent model drafts. Materials are approximate and animation, UVs, and custom Java renderers need separate work.
- **Import Modpack** scans up to 120 local JARs or description lines. It orders declared dependencies, flags missing dependencies, samples feature and recipe evidence, and can generate one rough model per JAR. Export the full report or prepare **Modpack Synthesis** with Axiom lore and progression attached. Recipe analysis groups equivalent inputs, suggests a conservative yield for conflicts, and compares Fabricator costs per output. Synthesis proposes which routes to keep, merge, defer, or drop. Enable automatic synthesis and staging to run without intervening between passes. Reviewed bundles use the normal save flow.
- **Mod Port** uses declared entry points, mixins, local calls, Java lambda targets, and matching resource IDs to build a bounded behavior trace. It reports sampled allocation sites and backward branches as leads for profiling. Intake also inventories up to four small bundled JARs without loading their code. **Inspect Class** accepts `archive.jar!class/path` for a class inside a bundled archive up to 4 MiB.
- The port workflow uses the trace to suggest project owners. With sources attached, its three passes identify the player-facing loop, compare native alternatives, and review workload, persistence, streaming, and multiplayer. The proposal specifies a baseline and candidate measurement on the same workload before claiming a speedup. **Stage valid edits on completion** prepares browser drafts. **Review Bundle** saves a multi-file candidate together with backups.
- **Useful** and **Needs Repair** record a human outcome beside local run receipts in `target/axiom-dev/agent-receipts.jsonl`. **Improve Toolkit** uses recent receipts to propose a focused tooling change and a way to measure it. Toolkit proposals have their own manual staging and save review.
- **Campaigns** loads an editable brief and starting sources for crafting, machines, magic, creatures, animation, materials, civilization, or dimensions. **Run history** shows recent passes, verification outcomes, and human reviews.
- The workbench reconstructs candidate edits against pinned saved files and checks JSON, RON structure, and JavaScript syntax before staging. It returns a failed check to the next allowed pass and rechecks saved files when you stage. Unsaved baselines and changes that require native validation remain pending and require manual staging. Syntax results are shown separately from proposed gameplay checks.
- A static sample can miss behavior behind obfuscation, reflection, injected control flow, dependencies, and nested code. Narrow the description and inspect relevant classes before drawing conclusions. Candidate edits are limited to attached project excerpts.

- **Ask AI** attaches a recipe, dependency plan, graph node, or the source around the Workspace cursor. In Workspace field details, **Ask AI** starts a task scoped to the selected value and includes its field help. Describe the outcome, run the task, and use **Stage Drafts** and **Open Editor** to tune the result visually. **+ Source** also accepts indexed engine, content, and Dev Panel files. Expand a scope row to see the exact excerpt.
- **Gather Context** searches saved sources for the task and optional symbol names. It shows read-only excerpts with paths and line numbers, relevant test source and design notes when found, and related diagnostics from the panel's recent build output. Expand a result to open it or attach it separately as an editable source. Runs require fresh context if an included file changes on disk.
- Context prioritizes attached lines, named records, and Rust constants. Attaching a saved System Board also follows its pinned links as read-only evidence and shows where each pin came from. Audits refresh context automatically and keep the symbols entered for the current task. Automatic repair scope requires a focused excerpt, such as a matching symbol or diagnostic location. Saved editable excerpts are checked against disk before a run.
- **Flaw Audit** uses that evidence for a read-only review. It asks for severity, a concrete trigger and consequence, and the smallest check that would confirm the finding. Symbol matches and risk markers are leads. Existing test source and historical diagnostics are not evidence that a check ran on the current revision.
- **Run** starts an on-demand Codex CLI session for the selected task, provider, and one to three passes. Prototype, Systems Review, and UI Review use distinct scopes of review. Later passes can refine or correct the earlier proposal. Each pass has a 150-second limit and **Stop** cancels the current run.
- Use an authenticated Codex CLI on PATH, or set `AXIOM_CODEX_PATH` to its executable before launching the panel. A blank Codex model uses the CLI default. Select Ollama or LM Studio and enter a local model name to use the CLI’s local provider path. The chosen provider receives the task and attached excerpts. The CLI starts in a temporary directory with shell, apps, hooks, delegation, and web search disabled.
- **Stage Drafts** checks exact edit anchors and rejects stale excerpts, overlapping replacements, and invalid RON or JSON. **Drafts** in the header gathers staged files across tabs for individual review and save. Source proposals still need the relevant build or runtime check. **Copy Task** and **Import Proposal** support work from another Codex session or model using the same review path.
- After staging, **Tune Candidate** opens each change in its visual editor when available, including Recipe Wiring, Advancements, sprites, and factory models. **Keep Variation** remembers a tuning in this panel session. Switching variations preserves edits to the previous draft. **Refine Current Draft** scopes a follow-up run to the edited files. **Review Current Bundle** previews the exact version you tuned before saving.
- **Review Bundle** publishes a candidate's staged files together and records their original and published versions before writing them. Open **Drafts → Candidate bundle history** to inspect or revert the bundle. A revert stops if any file differs from both recorded versions or has an open unsaved draft. Files already at their original versions are left out of the revert.

## Workspace

- Search across the project by filename or path. Ctrl+P focuses search and Enter opens the first match. **Data**, **Sprites**, and **All files** organize browsing.
- **Quick open** lists common files and the last eight opened files. The launcher resumes the last file and remembers the selected record or sprite tile. Changes still need **Review & save**.
- **Fields** opens a record directly when the file is a list of records. Click its name to search this file by name or ID. **All records** opens the full list. Hover a field for its source key, notes, or units where available. **Actions** contains raw value editing and record or field operations.
- **Nodes** generates a view from the selected structure. It links recognized catalog references such as recipe blocks to materials. Click a node to follow it. Use **Edit selected** to open its fields. Relationships outside the known catalog rules can be inspected in **Source**.
- **Source** edits supported project text, including Rust, WGSL, and TOML. **Find loaders** looks for direct asset-path references in the code.
- Review the changed region with Ctrl+S. Saves check the original file hash and keep a copy under `target/axiom-dev/backups`. Open **File info** to review backups, reload the file, or find its game loader.
- The panel previews common image formats and replaces existing binary assets up to 24 MiB. Use an external editor for binary models, art, and audio. Project text is limited to 4 MiB per file. The file browser excludes saves, dependency trees, and build output.

RON and JSON saves check syntax. Spell tables, assembled-tool schemas, tool-material stats and reaction recipes also pass through the same native schema and semantic checks used by runtime loading. Build the checkers once with `node tools/axiom-dev/build-validators.mjs` (Cargo and the pinned Rust toolchain are required). Single saves, AI candidate bundles, and backup restores validate before publication; missing, changed, or stale checkers block those catalog writes with a rebuild instruction. Other catalogs still rely on the game for type, ID, range, and gameplay validation. Native checks establish data validity, not whether a new spell behavior or engine primitive is implemented. The panel keeps file order and surrounding source during field edits. Keep stable IDs and row order in catalogs that use them.

## Forge Spark fixture

Open **Workspace → Spells** to stage Spark's launch speed, compare saved and draft values in disposable native worlds, and review or undo the RON edit. The [Spark work card](FORGE_SPARK.md) gives the launch, trace and rollback procedure.

Open **Graph → Residents and settlement life** for resident appearance, schedules, work and defense. Open **Workspace → Residents** for named Fernwake records. The [resident work card](RESIDENT_WORK.md) describes how to inspect the existing native work loop and which decision records to use when changing it.

## Spell review

From the checkout, run `tools/Review-StormSurge.ps1` with a current native build.
Its disposable pool scene prints a passing receipt only after the production
caster discovers Storm Surge, round-trips the observation through the progression
format, and casts across connected water without reaching the isolated pool.

Run `just lint-descriptions` before building. The Dev Panel's RON syntax check
does not enforce the game's copy rules.

Use `tools/Review-Grimoire.ps1 -Screen wands -SpellId 98 -Codex` with the same
executable to capture the game's card renderer. Both wrappers retain the review
manifest and logs in `target/visual-direction`.

Run `python tools/gen_spell_icons.py --spell 98 preview.png` to print one
validated Rust sprite and render its PNG without rewriting the generated sheet.
Use the printed arm for a targeted update when other sprite edits are present.

## Sprite editor

Sprite details trace item users into indexed recipes, including Fabricator processes and machine assembly. Usage filters separate known, shared and unknown references. Status badges link to their native effect and foods applying it. Tool masks link to the real schemas and pickaxes or weapons composed from them. Legacy pickaxe item icons are identified because native tool composition takes precedence.

Supported static PNGs in the authored texture folders are paintable after PNG header, size, bit-depth and animation checks. Registered generated atlases and comparison/reference art stay outside the editable default. **Lock paint alpha** protects the current silhouette during brush, shape, fill, replace and eraser actions. PNG Palette Whackifier locks preserve exact RGBA colors; its seed/settings use palette algorithm 1.

After saving a tool mask, **Native preview** renders it with the game's Rust `engine_tools` composer, without rebuilding the entire game. It records source and renderer hashes and keeps saves separate. Blank columns belong to schemas using other native art paths. **Playtest → Native art evidence** also renders the existing compiled game's sheet and names its build. The saved-mask preview verifies the shared icon composer; held-tool framing and playability still require a rebuilt game playtest.

Open **Art → Sprites** for individual source sprites or **Workspace → Sprites** for the editable sheets. The library defaults to **Editable source**. Use the asset-type filter for generated and reference art, or to isolate modified and bookmarked sprites. Double-click a source sprite to paint. Generated entries open their registered generator when available.

- Paint with pencil, eraser, flood fill, color picker, line, rectangle, ellipse, dithering, or replace color. Use brush sizes of 1, 3, or 5 pixels, horizontal and vertical symmetry, and filled shapes. Flip, rotate square sprites, or add a one-pixel outline with the active color.
- Drag with **Marquee** to restrict painting, filling, or color replacement, and to copy, cut, or clear a region. Choose **Paste** and click its destination. Tool shortcuts work while the canvas has focus.
- **Color lab** compares the original and adjusted sprite. Change hue, saturation, brightness, contrast, gamma, or a color preset across the marquee or whole sprite. The six **Palette Whackifier** previews use seeded OKLCH shifts with locked swatches, neutral protection, and a pin for exploring nearby variants. Tool masks adjust the shared material ramp.
- **Review & save** validates and saves the authored sheet with a backup. Forge recovers unsaved sheet edits when you reopen the file. If the source changed on disk, **Export Source** saves a separate copy of the draft so you can reconcile it. **Export PNG** writes the current sprite as a standalone image.

- **Icon sheets** open individual 16×16 sprites. Click the sprite name to find one by name or ID. Edit palette colors or add a key, then paint with it. The outline swatch is shared across its sheet.
- **Copy with tint** borrows another icon. Pick the source and hue, preview the result, and apply it to the selected destination draft. Sprite-sheet reviews show before and after art.
- **Tool sprites** open the game’s 16×16 part masks. Pick a material to see its four shades and edit the shared ramp. Choose a tool that uses the mask to preview it with all its parts in the selected material.
Rebuild and relaunch the game to see saved changes to embedded sheets.

## Playtest

**Location ambience → Inspect ambience** reads the native location profile, mana
density, actual mana/health regeneration, water, plant coverage and sparkle
strength. The same command is `/ambience-status`. **Reference Map → System Boards
→ Location mana and ambience** pins the recovery caps and source profiles for
review before rebuilding. See [design and evidence](../../docs/design/LOCATION_MANA_AMBIENCE.md).

Choose **Build / rebuild** to compile current files into a dedicated playtest executable. Embedded catalog and source changes take effect after rebuilding and relaunching. **Launch existing build** starts the current release-fast binary when available.

Playtest worlds live in `target/axiom-dev/worlds`. Create or open a solo world in the game before sending commands. The panel uses the same command rules as solo chat. Creative mode enables Give and construction tools. Spectator enables noclip flight. The existing build uses clipboard commands. Keep the panel build open to see game replies in **Output**.

The live controls use the launched game's standard input and output. Hosted, joined, and cellular sessions do not accept panel commands.

## Design refinement

In Playtest, enter a brief and any measured evidence, then run three design
passes. The workbench drafts, critiques, and revises the proposal automatically.
The output stays in the workbench for review.

Before launching the workbench, set `AXIOM_DESIGN_ENDPOINT` to the full chat
completions URL and `AXIOM_DESIGN_MODEL` to its model name. A local HTTP endpoint
runs between workbench playtests and builds. For review during play, point the
workbench at an HTTPS endpoint on a separate machine and set
`AXIOM_DESIGN_OFF_HOST=1`. Set `AXIOM_DESIGN_TOKEN` if the endpoint needs a bearer
token. The request contains the entered brief and prior passes from this review.

## Evidence-driven improvement

**AI > Propose Next Move** selects a bounded investigation from current sources and prior outcomes. **Run Bounded Loop** claims and investigates up to three moves, runs a registered check when available, and stops for candidate review. See [Evidence-driven improvement](EVIDENCE_LOOP.md) for the shared API, duplicate-work handling, provenance and verification limits.

**Catalog > + Food** and **Make food variant** now use the same revision-checked preview/apply commands as the [food workflow CLI](FOOD_WORKFLOW.md). New icons have independent slots, nutrition changes are explicit, and recipe withdrawal retains item identity.
