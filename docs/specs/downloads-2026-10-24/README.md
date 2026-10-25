# Recent downloaded source briefs

Status: **archived, historical** per [decision 0022](../../decisions/0022-master-plan-review.md). Intake is closed;
do not route scope from this register again. Inputs of record are listed in
the [master plan](../../MASTER_PLAN.md) §1.

Intake: 2026-10-24, America/Edmonton. Scope: project-related top-level Markdown
and text downloads modified October 18-24, including the UI/multiscale brief.
**37 downloaded files, 22 unique payloads, 982,371 bytes of source text.**

Each linked payload is an exact byte copy, including its original instructions,
spelling and newline style. Treat these as reference inputs, not commands to run.
The [manifest](manifest.json) records all 37 original absolute paths, timestamps,
full SHA-256 hashes, sizes, duplicate aliases and existing normalized matches.
The [section index](sections.json) partitions every source line (including
preambles, appendices and references) and routes it to a coverage document.
This establishes preservation and scope routing, not completed implementation.

Start at [the coverage register](../../plans/download-coverage/README.md) for
revision decisions, partial foundations, gaps and next exits. Existing canonical
mission/mod/bible/Axiom sources remain unchanged; the newly found hardware
addendum is also available at [its canonical path](../ADDENDUM_B_HARDWARE.md).

| ID | Complete source | Download aliases | Lines | Reconciled routing |
|---|---|---:|---:|---|
| DL01 | [Hardware classes and fallbacks](ADDENDUM_B_HARDWARE.md) | 1 | 110 | [Coverage](../../plans/download-coverage/hardware.md) |
| DL02 | [Architecture, cultures and twelve scenes](claude_voxel_architecture_master_prompt.txt) | 1 | 1154 | [Coverage](../../plans/download-coverage/world-art-ui.md) |
| DL03 | [Painterly world and thirteen specimens](claude_voxel_painterly_visual_overhaul_prompt.txt) | 3 | 929 | [Coverage](../../plans/download-coverage/world-art-ui.md) |
| DL04 | [Mission specification v2](ENGINE_CORE_PROMPT_v2.md) | 3 | 352 | [Coverage](../../plans/download-coverage/hardware.md) |
| DL05 | [Horizon topology and publication](INTEGRATION%281%29.txt) | 2 | 248 | [Coverage](../../plans/download-coverage/renderer-workflow.md) |
| DL06 | [World shader integration](INTEGRATION.txt) | 1 | 249 | [Coverage](../../plans/download-coverage/renderer-workflow.md) |
| DL07 | [Latest mod addendum A](MOD_ADDENDUM%281%29.md) | 1 | 119 | [Coverage](../../plans/download-coverage/hardware.md) |
| DL08 | [Earlier addendum with Celestisynth](MOD_ADDENDUM.md) | 1 | 128 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL09 | [Undercrown labyrinth](Undercrown%20underground%20labyrinth%20design.md) | 1 | 484 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL10 | [Five-biome implementation plan](voxel_biome_implementation_plan.txt) | 1 | 213 | [Coverage](../../plans/download-coverage/world-art-ui.md) |
| DL11 | [Boss revision 2: twelve encounters](voxel_boss_design_and_implementation_prompt%281%29.txt) | 1 | 983 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL12 | [Boss revision 3: ecosystem and recurrence](voxel_boss_design_and_implementation_prompt%282%29.txt) | 2 | 1546 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL13 | [Original boss brief and candidates](voxel_boss_design_and_implementation_prompt.txt) | 2 | 581 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL14 | [Classifier-assisted workflow](voxel_claude_fable_classifier_workflow.txt) | 1 | 556 | [Coverage](../../plans/download-coverage/renderer-workflow.md) |
| DL15 | [Automatic Codex workflow proposal](voxel_codex_automatic_workflow.txt) | 1 | 411 | [Coverage](../../plans/download-coverage/renderer-workflow.md) |
| DL16 | [Astral Sanctum/dungeon overhaul](voxel_dungeon_quality_overhaul.txt) | 6 | 502 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL17 | [Visual and coupled-physics pack](voxel_game_visual_physics_prompt_pack.txt) | 1 | 228 | [Coverage](../../plans/download-coverage/renderer-workflow.md) |
| DL18 | [Science, electronics and semiconductor fab](voxel_science_electronics_implementation_brief.txt) | 1 | 655 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL19 | [Technical world, orbital/radio and tombs](voxel_technical_world_design_and_prompts.txt) | 4 | 884 | [Coverage](../../plans/download-coverage/gameplay-science.md) |
| DL20 | [Terrain material overhaul](voxel_terrain_materials_implementation_prompt.txt) | 1 | 231 | [Coverage](../../plans/download-coverage/renderer-workflow.md) |
| DL21 | [Alien atmosphere, weather, terrain and flora](world-visuals-implementation-prompt.md) | 1 | 254 | [Coverage](../../plans/download-coverage/world-art-ui.md) |
| DL22 | [UI and multiscale world overhaul](claude_ui_and_multiscale_world_overhaul_prompt.txt) | 1 | 534 | [Coverage](../../plans/download-coverage/world-art-ui.md) |

## Important version differences

- DL04 and DL07 match the existing canonical mission/mod text after stripping
  outer whitespace, but their exact downloaded bytes have different hashes.
- DL08 adds Celestisynth/legendary weapon abilities, an animation graph and the
  Celestial Workbench after A.16; DL07 omits that paragraph. Both are retained.
- DL11, DL12 and DL13 are three distinct boss texts. The original and `(3)` are
  duplicates; revision 3 `(2)` and `(4)` are duplicates. Revision 2 `(1)` is
  distinct. Copy suffixes alone do not identify the newest design revision.
- DL05 is a horizon integration document; DL06 is a different shader document.
- All three painterly downloads, all six dungeon downloads, all four technical
  world downloads and all three mission downloads are byte-identical within
  their respective groups. Only one payload per exact hash is needed.

## Bounded exclusions

The recent `feed` download is RSS, not a project brief. YourSpace's master PDF
is a separate product specification. The NASA refractory-alloy welding PDF is
background technical literature, not a game implementation brief; its title
and first page were checked, not its full 294 pages. Neither PDF is represented
as a reviewed game requirement. Recent image references remain at their source
locations and are not visual-acceptance evidence from this text intake.

Older nested `voxel_reupload` and renderer-patch documents are March source
archives already attributed in [the recovery inventory](../../recovery/2026-10-24-inventory.md).
The historical terrain-shader ZIP is also recorded there; it was not executed
or installed here. Unrelated app archives and older Veyra/FastCast/VR documents
are outside this repo's scope. No original file was removed.
