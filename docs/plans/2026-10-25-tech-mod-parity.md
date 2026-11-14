# Tech-mod parity: machines, conduits, tools (Mekanism/Thermal/IC2/Create + Tinkers)

## Stock-driven semiconductor automation (2026-10-26)

Placed fabricators can now maintain a chosen lab-stock target. In
Semiconductors, choose Maintain on an operation, set Target, and Load Feed.
Repeat controls future batches; the machine's existing power switch and
installed controller pause work in progress. Clear Order returns unused sand
and lets an already-paid batch finish. Ordinary resource inputs stay in the
machine buffer. The focused fabricator takes priority over other nearby ones.

Use separate fabricators for Silicon Smelting, Zone Refining, Crystal Growth,
Wafer Preparation and the supporting operations. Downstream orders wait for
their typed inputs and restart as finished stock is consumed. Coal, copper,
steel, fibre and boards enter through existing item pipes, feeders and storage;
Load Feed also transfers whole items from the pack. Incoming pipe packets
reserve buffer space during loading. Sand is a separate 64-block magazine;
it is not silently converted into a resource-table entry. Typed intermediate
stock still uses the shared bounded lab inventory, not physical item transport.

Targets count the first output plus pending batches, and run in whole batches,
so the final batch can exceed the requested target. Every co-product must fit
the 4096-unit stock cap. At most 64 persistent orders and 64 active supply jobs
exist. Admission runs at 4 Hz with rotating deterministic priority. It checks
all raw inputs, lab inputs, power and output reservations before consuming any
materials; active work uses the existing paid 120 Hz clock. An order owns its
fabricator even while waiting, preventing ordinary component recipes from
consuming the same inputs or energy.

Save 36 retains target, repeat state and sand inventory; older chip saves
default to no orders. Order reassignment carries sand, progress and paid energy
to a nearby free fabricator, including recovery after a source disappears.
Ordinary resource buffers remain at their source. Packing a machine refunds
its sand in the same inventory-capacity transaction as its other contents and
removes its order; an unfinished paid batch remains available for reassignment.
The look-at overlay shows batch progress or stock toward the target.

Verification is source review and application compilation only. No tests,
gameplay sessions or UI previews were run. Existing dirty terrain/rendering work
is preserved; this increment does not enable generator 56.

## Powered semiconductor chain (2026-10-26)

Implemented source path: sand + coal → crude silicon → zone-refined silicon →
single-crystal ingot → polished coupon wafers → revision-specific masked device
and interconnect operations → passivation → wafer probe/binning → packaged
controller → existing powered machine socket and sensor/control behavior.

The Component Fabricator now hosts an exclusive semiconductor work queue.
Eight operations prepare silicon, wafers, mask glass, metal targets and hood
filters, or order prepared reagents. Inputs come transactionally from the pack
and the bounded lab inventory. Outputs enter lab stock only after paid work.
Materials, timings, power, progress and stock are visible under Semiconductors
on the machine, designer and Recipes screens. Designer presets remain usable.
Release a revision, choose Open Bench or Clean Hood, start a lot, then install
a tested package from the target machine's existing Controller section. Trial
Lot also uses this route; the live path no longer packages a chip instantly
from a circuit carrier and copper.

Lab stock has eleven typed supplies, capped at 4096 each, separate from the
22-resource legacy table. There are at most 64 supply batches and one active
batch or wafer lot per fabricator. Pending output and prospective die counts
reserve capacity before inputs are consumed. Each lot reserves its maximum
possible packages against the 1024-part cap; failed admission leaves stock and
the immutable revision/mask library unchanged.

All work uses authoritative 120 Hz world steps. Jobs prepay 0.25 s quanta from
the fabricator's actual integer energy buffer; debits enter factory power
statistics. Wafer work draws 4 kW. No energy, a disabled machine, a holding
controller or a removed fabricator stops progress. Ordinary processor work
cannot simultaneously spend that machine's buffer. Jobs and lots can be
reassigned without resetting progress, spending inputs again or rerolling yield.
Old unassigned lots require Resume and subsequent paid work.

Save writer 35 persists stock, jobs, energy credit, assignments and per-lot work
clocks. Version 34 loads with empty lab stock and no assignments; existing
installed/stocked parts and historical lots remain. Historical lots keep route
version 0. New lots use version 1, which explicitly deposits a gate conductor
stack before its pattern rather than labeling oxide as a functioning gate.
Save validation checks bounds, unique assignments, route histories, clocks and
reservations. No generator gate is added or activated by this feature.

These are authored gameplay batch sizes and durations, not literal chemical
stoichiometry or real fabrication schedules. Reagent Order is an explicit
board-for-prepared-supplies exchange; it does not claim to synthesize dopants
from tin or process gas from coal. Zone refining, crystal/wafer preparation,
and repeated pattern/etch/treatment stages inform the sequence, while device
behavior still comes from verified macro templates rather than transistor or
atomistic simulation. Physical gas plumbing, detailed thermal windows, a full
reagent refinery and automated lab-stock logistics are beyond this chain.

References: [SUMCO wafer production](https://www.sumcosi.com/english/products/process/),
[BARC zone refining](https://www.barc.gov.in/technologies/zru/index.html),
[ASML fabrication overview](https://www.asml.com/en/company/stories/2021/semiconductor-manufacturing-process-steps).

Verification: application compilation and whitespace checks only. No tests,
gameplay runs or visual acceptance were performed at the owner's request.

## Inventory and progression increment (2026-10-25)

The owner's follow-up prioritizes inventory/crafting and the item browser;
separate technology and chip-design panels retain their current behavior.
The Downloads science and dungeon briefs are references, not authorization to
execute every embedded prompt or run their requested acceptance tests.

Implemented:
- One shared item catalogue beside the survival grid, covering both grid and
  machine recipes. Recipe/uses details open without replacing the inventory;
  ingredient transfer remains explicit and transactional. Narrow screens stack
  the catalogue below the pack. Advanced destinations move behind More.
- Nine capability branches over existing content: Building, Equipment,
  Metallurgy, Power, Logistics, Processing, Biology, Control and Arcana.
  Search accepts AND terms and #category, ^station, $material and @source.
- Production plans can include reusable stations once per tree, recursively
  costing their construction against the same stock as recipe inputs. Connected
  power, utilities and multiblock completeness remain runtime conditions; the
  calculator does not claim possession means an operational factory.
- A workbench circuit board (2 copper, 1 tin, 2 fibre) bootstraps a fabricator
  without mandatory salvage. Powered fabrication yields 2 boards from 2 copper,
  1 tin and 1 fibre. Boards and wild essence are expanded as manufactured
  intermediates rather than forcibly ending their recipe trees as gathered loot.
  Existing resource IDs, inventory encoding, worlds and generator gates remain.

This is a board/control bootstrap and existing-recipe graph overhaul, not a
complete semiconductor economy. The existing wafer supply exchange is a gameplay
abstraction, not silicon purification or chemical processing. Future IC routes
must distinguish purified feedstock, single-crystal substrate, wafer preparation,
device/layer operations, interconnect/passivation, test and packaging. Lithography
patterns resist; it does not manufacture a complete transistor in one exposure.
Primary references checked: SUMCO production processes
(https://www.sumcosi.com/english/products/process/) and ASML process overview
(https://www.asml.com/en/company/stories/2021/semiconductor-manufacturing-process-steps).

Validation: application library/binary compilation only; no tests or live visual
acceptance. Fine storage adoption into normal gameplay remains a separate open
integration item; this UI/progression work does not establish fine CA or chipping.

Owner request (2026-10-25): every single-block machine, cable and the worktable
read like Mekanism (bolted steel casing, framed status windows, tier-coloured
conduits); feature parity with the big tech mods, implemented better and at
variable voxel scale; full Tinkers' Construct-style tool mechanics.

Spec of record: `docs/specs/MOD_ADDENDUM.md` A.2 (Tinkers), A.9 (Thermal),
A.10 (Factorio), A.12 (IC2/BuildCraft), A.13 (Mekanism), A.15 cross-cutting.
Master plan rows: M01, M04–M07, M15–M17 (`docs/MASTER_PLAN.md` §2). The plan
places tools at M3 and the deep chains at M4–M5; this document is the ordered
work list for reaching them, not a change to the tiers.

## What exists (2026-10-25 survey)

- 27 `factory::Kind`s in `assets/machines.ron`: generator (coal/arcane),
  crusher, furnace, alloy kiln, feeder, storage, pipe, cable (3 tiers),
  fluid pipe, pump, tank, washer, bioreactor, cultivator, fabricator, mining
  drill, battery (4 modes), transformer, assembler, coke oven and blast furnace
  bricks, induction casing/port/cell/provider (matrix multiblock).
- Integer power network with tiers and transformers (save v21), induction
  matrix, item packets in pipes, fluid pipes with water volume.
- Wands are the only part-based item (`adventure/wands.rs`,
  `egui_ui/tinkering.rs`). No pickaxe, hammer or armour exists at all.
- Models: `mesh/models.rs` draws every node procedurally; no textures.
  Review fixture: `VOXEL_TORCH_REVIEW_MODE=factory --benchmark-torch-review`.

## Phase 1 (this session): look

1. Cables: square black casing, tier stripe plus grey flanks, junction cubes
   with a framed bolt socket on each open face. Done in `mesh/models.rs`.
2. Shared machine casing `mesh/models/machine_casing.rs`: bolted steel
   panels to 0.62 m, corner posts, trim rim per machine family, front window
   with status bar and progress lamps, round port sockets on the other sides,
   hazard chevrons. Per-kind machinery still drawn above the casing. Done.
3. Generator: fins, copper top plate, exhaust cap, fan guard, output studs.
   Assembler: plank worktable, steel cheeks, tool rail, vise. Done.
4. A/B captures: `target/machines-review/{before,after}/`.

5. Conduits as voxel assets (owner choice 2026-10-26): 8 voxels per metre,
   text `.vox` files in `assets/models/factory/` (loader
   `mesh/models/voxel_asset.rs`, drawn by `mesh/models/conduit.rs`). Cable arm
   2 x 2 voxels with tier stripe; item pipe 4 x 4 steel tube round a glass
   core; fluid pipe 4 x 4 with medium stripe; junction blocks, framed sockets
   on arm-less faces, flanges where an arm enters a machine, lit extractor
   mouth. Single-block machines use the same grid: `shell_closed`/`shell_open`
   plus a status `bar_*` overlay (seamed panels, trim-coloured window, framed
   ports matching the conduit sockets); a kind with an authored mesh (furnace)
   keeps it. Per-kind parts are voxel modules in two stepped frames (generator
   rotor and stack, crusher rollers, drill, washer, assembler worktable and
   arm, pump riser, tank with four water levels, battery charge gauge). The
   earlier float-cuboid casing and glyph column are removed.
6. Side configuration (Mekanism configurator, Thermal side config): a conduit
   stores one closed bit per face in `Node::target` (`factory/sides.rs`, no
   save bump; decode accepts 0..63 on conduits). Closed faces split networks,
   drop adjacent machine ports, block packet routes and extractor pulls.
   Shift+F on a conduit face toggles it (`Action::ToggleSide`).

## Next, in order

1. Machine face modes (Thermal): per-face input/output/off for machines. Needs
   a field outside `target` (assembler and battery use it): batch with the
   next save bump (v35) together with tool items and smeltery kinds.
2. Pipe sorting (Forestry/Thermal): per-conduit-face filter, reusing the
   `filter` field and the side bits.
3. Quarry (BuildCraft): done in this pass as drill modes, no save bump.
   `MiningDrill` mode 0/1/2 is a 3 m shaft or a 9 m or 15 m quarry
   (`factory/quarry.rs`: footprint, cursor offsets, limit; mode 0 keeps the old
   cursor layout, tested). The frame, bridge, carriage, rod and head are voxel
   assets driven by the excavation cursor. Open: yield is still ores only
   (other blocks vanish, there is no stone resource), power does not scale
   with hardness, and marker-block areas and the RFTools builder (copy a
   region, place it) need a new area field, so they wait for the v41 bump.

Open: multiblock bricks unchanged on purpose; the Workbench world block's
texture lives in `world_shader.wgsl`, which another session is editing.

Constraint (2026-10-25): another session owns the inventory/crafting UI
increment above (`inventory.rs`, `recipes.ron`, `egui_ui/*`), so the Tinkers
save/inventory wiring in Phase 2 waits for that work to land; the physics half
(molten metals as liquid cells) does not touch those files and goes first.

## Phase 2: Tinkers-style tools (M3, save bump)

Foundation crate `engine_tools` (started this session, dormant): parts,
materials, trait rules keyed on properties, assembly stats, xp/levels, repair,
encode/decode. Remaining:

1. Save: `ToolAssembly` in the inventory item enum; `SAVE_VERSION` 35 with a
   decode test for v34 files. One bump, bundled with the smeltery blocks.
2. Smeltery multiblock: seared brick kind + controller + drain + faucet +
   casting table/basin as `Kind` rows; molten metals as real liquid cells at
   temperature using `thermal.rs` latent fusion; alloys as `reactions.ron`
   rows on molten liquids; casting = confined liquid region + mold cell.
3. Part builder and tool station UI (`egui_ui/tool_station.rs`), reusing the
   wand tinkering layout rules.
4. Mining: tool `mining_level`/`mining_speed` gate and speed `BlockId`
   hardness; `ToolKind` in `creative.rs` extended; durability loss, repair.
5. Modifiers: `assets/tools/modifiers.ron` through the common evaluator (M17).
6. Armour parts (Construct's Armory) after tools work.

## Phase 3: machine depth (M4)

Each row is data plus one bounded code change; order by dependency.

1. Augments on machines (Thermal): speed/efficiency/secondary-output rows in
   `machines.ron`; evaluator shared with tools (M17).
2. Ore processing depth (Mekanism 2×–5×): enrichment chamber, purification,
   chemical injection as `Kind` rows; chlorine/sulfuric acid/hydrogen as real
   gas/liquid cells in `materials.ron` with reactions; electrolytic separator
   producing H₂/O₂ (explosive mix is a reaction row).
3. Inserter family and belts/undergrounds/splitters as transport cells (M05,
   M06), Factorio semantics, filters on faces.
4. Steam: boiler + turbine using the heat network (M11, M15); magmatic dynamo
   from temperature delta; waterwheel/axle kinetic starter (M08).
5. Voltage/loss/overvoltage on cables (IC2), machines fail loudly when
   over-tiered (M07).
6. Digital miner: area scan + filter, real holes, replace-with option.
7. Conduit bundling (Ender IO): item/fluid/power/signal in one cell.
8. Cardboard box: pack a node and its inventory as an item.
9. Tesseract/link network (M10), digital storage and autocraft (M18, M19).

## Rules that bind every phase

- New `Kind`, resource or item variant = save version bump; batch them.
- Content rows in `assets/*.ron`, never Rust tables; run the `dump_*_ron`
  tests when a format changes.
- Visual change = before/after capture from the factory fixture.
- Variable voxel scale: all geometry in cell units times
  `VoxelScalePreset::meters()`; nothing hardcodes 1 m.


## Tinkers integration audit — 2026-10-28

The scope remains Addendum A.2, ADR 0022, and master-plan P04/M04/M17.
The initial smeltery prototype is **not Phase 2 completion**.

| Requirement | Current source state | Remaining integration |
| --- | --- | --- |
| Parts and material-derived traits | `engine_tools` has schemas, weighted stats, property rules, XP, repair and codec | Connect persistent assemblies to item ownership, transfers and gameplay; do not replace this with tier-only picks |
| Real molten cells and heat | Copper, tin, iron and bronze now have authored phase pairs and fusion energy; sparse thermal conduction and latent storage replace fixed-rate cooling | Controller fuel/heat must use the shared factory owner; current input interaction still creates hot liquid immediately |
| Alloy reactions | `reactions.ron` has an exact 3:1 copper/tin region recipe; sparse solver applies it to a connected molten region, conserving cell volume and solver sensible heat | Packed CPU/GPU pair kernels do not execute region recipes; full physical mass/enthalpy acceptance remains open |
| Smeltery multiblock | Prototype floor/wall check and finite cell pouring | Register components as factory Kind rows, shared structure ownership, scalable dimensions and proper controller UI |
| Molds and parts | Table/basin collect cooled metal only | Persistent molds, part casting, differentiated table/basin output, failure/spill conservation; remove the sub-25cm restriction |
| Part builder / tool station | Not integrated | Visible part choices, each part's stat contribution, assembly, repair and sharpening through the existing tool model |
| Mining / leveling | Legacy tier picks remain separate from ToolAssembly | Material-derived harvest/speed, wear, broken tools, XP and repair across local/server authority |
| Modifiers / armor | Tool model stores modifier IDs; shared M17 evaluator remains pending | Common evaluator and authored modifier costs/tradeoffs; armor follows working tools |
| Presentation | Larger inventory/catalogue slots and native crafting-material icons; generated workshop sheets are references | In-game review of station layout, components, casting and held parts; concept sheets do not establish parity |

Region reaction rows deliberately do not compile to ordinary two-cell rules:
a 3:1 recipe must never silently become 1:1 on another simulation backend.
The current save header increment for appended materials does not establish
assembly serialization. Bundle the actual new item/factory layout with its
versioned readers before claiming that Phase 2 save work is done.

Validation policy for this request: local compilation only; no tests or hosted
CI requested. Runtime and conservation acceptance are still outstanding.
