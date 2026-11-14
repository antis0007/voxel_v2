# Tool system acceptance: Tinkers' Construct 1.12.2 benchmark

The target is comparable or better visual craft and ordinary play, not a matching
list of feature names. Reference the upstream 1.12 branch, not Tinkers 3 guidance:
https://github.com/SlimeKnights/TinkersConstruct/tree/1.12

## Required player outcomes

| Area | Acceptance requirement | Current evidence / remaining work |
| --- | --- | --- |
| Silhouette | Recognise each tool without its tooltip, in inventory and in hand | Twelve original modular silhouettes replace the universal pickaxe; native review pending |
| Materials | Head, binding and handle visibly reflect the actual assembly | Shared production texels use assembly material IDs; secondary plates and modifier-specific fittings still need richer art |
| Assembly | Choose real owned parts, see the result and resulting stats before committing | Part icons, live preview, traits and inventory-count validation implemented |
| Upgrading | Retain a favourite tool while changing its parts | Replacement consumes the new part, preserves identity, XP, levels, modifiers and absolute wear; broken tools remain broken |
| Repair | Broken tools survive and can be repaired with their head material | Existing repair path retained; durability bars and material cost visible |
| Modifiers | Understand costs, limitations and changed stats before spending | Affordability, validity and stat comparison shown; only four modifier families currently exist |
| Combat | Material/part choices affect actual damage and wear | Assembled attack stats now enter PvE and PvP base damage; shared wear/XP path handles mining and combat |
| Special tools | Hammer/excavator/scythe/bow behaviour earns the shape | Schemas are present; area harvesting, ranged assemblies and tool-specific action feel remain incomplete |
| Traits | Every advertised trait produces a verifiable gameplay consequence | Property-derived trait list exists; full trait gameplay integration is not yet established |
| Casting | Build a readable foundry, see finite metal flow and cooling, recover the right part | Existing thermal/casting implementation; require native construction/pouring review, reusable casts and alloy-selection UX review |
| Economy | Material alternatives and upgrades have reasons to exist beyond a larger number | Stat/trait data exists; balance, resource accessibility and repair costs require survival play |
| UI | One focused workspace, no wall of owned-tool panels or unsolicited success popups | Build/Service modes and one selected service tool; richer slot-first material interaction remains open |
| Information | Tooltips and previews agree with actual state | Material-dependent stats and modifiers in Shift details; damage conversion is shared with combat |
| Handling | Sound, swing, impact, heft and grip match the tool | Shared held sprite pipeline; mass-sensitive animation, tool-specific audio and grip review remain open |
| Reliability | No duplicate parts, free repair through swapping, lost XP, or mismatched client ownership | Authoritative inventory admission retained; runtime save/reload and multiplayer acceptance remain open |
| Integration | Recipes, workstations, loot, repair and automation form one useful survival loop | Existing systems remain connected; end-to-end survival acceptance remains open |

No claim of full parity follows from compilation or the exported art sheet.
Do not add tests or hosted CI unless requested. Review actual production renders
and ordinary survival interactions, including unavailable materials and a full pack.

## Review entry points

`VOXEL_TOOL_ART_SHEET=<absolute PNG path>` exports the production inventory/held
texels without opening a save. Rows: stone, copper, iron, steel with modifier
bands, copper parts, clay molds. Columns follow the tool/part asset table order.

`VOXEL_MENU_CAPTURE_FILTER=tools-` with `--capture-menu=<absolute directory>`
renders Build, Service and a compact station through production egui/wgpu.
These are fixture previews, not proof of successful survival transactions.
