# Docs index

Read `AGENTS.md` first. This index says how far to trust each note.

A note records the code it was written against. Check its claims against the source
before acting on them. Launchers named in these notes (`run-*.cmd`) are not in the
repo any more; they pointed at `target/` review builds.

## Start here

- `ATMOS_CLOUD_REWRITE_STATUS.md`: clouds, current status.
- `ENGINE_AUDIT_AND_REFACTOR.md`: engine layout.
- `PIXEL_ART_DIRECTION.md`: pixel grid and palette rules.
- `VISUAL_DIRECTION_EXECUTION.md`: visual review contract and evidence policy.
- `CODE_ADVISOR.md`: local review hints.
- `UNDERCROWN_SCHEMA.md`: undercrown data contracts.
- `WORLD_LORE.md`: world premise, offices, open mysteries and voice rules for all player-facing text.
- `SPELLS_AND_DISCOVERY.md`: spell tables and discovery.
- `POWER_MULTIBLOCKS_AND_RECIPES.md`: power, multiblocks and the recipe browser.

## Design and reference

Still useful, but verify against the code:

- `FACTORY_UPDATE.md`, `LIVING_FACTORY_UPDATE.md`, `TINKERING_FORGE_AND_CATALOGUE.md`
- `WORLD_OVERHAUL.md`, `WORLD_SIMULATION_SCHEDULER.md`, `SANDBOX_ARCHITECTURE_IMPLEMENTATION.md`, `SANDBOX_ENGINE_AUDIT.md`
- `MATERIAL_STUDIO_AND_REFERENCE.md`, `BESTIARY_RIG_PIPELINE.md`, `BOSS_SERIES_STATUS.md`
- `MERIDIAN_ORBITAL_SERVICE.md`, `MERIDIAN_PHYSICS_REFERENCE.md`, `MERIDIAN_MEGASTRUCTURES.md`
- `HUD_DESIGN_AUDIT.md`, `GRIMOIRE_UI.md`
- `HORIZON_VALIDATION_AUDIT.md`, `HORIZON_CONTINUITY_REPAIR.md`, `HORIZON_DYNAMIC_PROXY_AUDIT.md`
- `RENDERER_REWRITE_EXECUTION_PLAN.md`, `ADAPTIVE_GPU_RESIDENCY.md`, `GPU_READBACK_SAFETY.md`
- `CLOUD_ARCHITECTURE_OVERHAUL.md`, `CLOUD_SAMPLING_QUALITY.md`, `CLOUD_LIGHTING_AND_PARTICLE_DISSOLVE.md`

## Dated session notes

Each describes the build it was written against and what was measured then. Do not
treat them as current instructions.

Everything else at `docs/` top level, including the `*_UPDATE`, `*_REVIEW`, `*_FIX`
and `*_PASS` files (for example `WAND_AUDIO_AND_MATERIAL_FIX.md`,
`VISUAL_STYLE_UPDATE.md`, `CLOUD_SEAM_FIXES.md`). `superpowers/` holds specs and plans
from the superpowers workflow.

## Superseded

`docs/legacy/` holds superseded plans and prompts, kept for history. Do not follow them:

- March 2026 cloud, shadow, weather and engine plans, and the cloud banding fix plan.
- `agent_prompts.md`, `cloud_debug_agent_prompt.md`: one-off task prompts.
- `implementation_plan.md`: an old cloud plan with stale absolute paths.
- `root-2026-03/`: March 2026 cloud fixes and LOD plans.
