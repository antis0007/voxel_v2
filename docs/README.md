# Docs

Start with `CLAUDE.md` at the repo root.

- `ARCHITECTURE.md`: data flow, tick order, scale/LOD, how an edit propagates, where things live.
- `TESTING.md`: `just verify`, headless scenarios, snapshots, content round-trip tests.
- `PLAN.md`: the revamp phases with measured before/after numbers and the ranked debt list.
- `AUDIT.md`: the 2026-10-22 audit the plan was built from (a snapshot, not kept current).
- `adr/`: short records of the decisions made during the revamp.
- `design/`: design references that code still follows. They describe intent, not current code; verify against the source before acting on a detail.
  - `WORLD_LORE.md` (voice rules for player-facing text), `PIXEL_ART_DIRECTION.md`, `VISUAL_DIRECTION_EXECUTION.md` (review evidence policy), `SPELLS_AND_DISCOVERY.md`, `POWER_MULTIBLOCKS_AND_RECIPES.md`, `UNDERCROWN_SCHEMA.md`, `EXPEDITION_DEPTH_DESIGN.md`, `MIRAGE_AND_COSMOS_UPDATE.md` (benchmark flags), `CODE_ADVISOR.md` (local review hints).
- `licenses/`: third-party notices.

Dated session notes, superseded plans and the old `superpowers/` specs were moved out of the repo on 2026-10-22 to `C:\dev\voxel_v2_scratch\docs-archive-2026-10-22`; git history before that date still has them.
