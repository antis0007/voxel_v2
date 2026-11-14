# Engine source completeness audit

The five candidates below were subsequently authorized and integrated. See
[integration.md](integration.md) for their current wiring and validation limits;
this document preserves the findings before integration.

2026-11-05. Read-only comparison of implementation sources; no tests, builds or
game runs. No branches were merged, and no source worktrees were changed.

Inspected the 28 registered worktrees, recent branch history (44 non-snapshot
commits since October 24), and dirty source patches in the recovery worktrees.
Also inspected the older chest branch. Unmerged ancestry and different filenames
were treated as leads, not proof of missing functionality. Runtime quality of
the stranded candidates has not been verified.

## Useful implementation candidates still absent

| Candidate | Preserved source | Current engine evidence | Integration constraint |
| --- | --- | --- | --- |
| Player statistics and advancements | `s9-records`, `df6228c5` | No player `world/stats` module, advancement table or Records page. Current adventure records store encounter flags; factory statistics measure production. Neither replaces this subsystem. | Adapt mining/crafting/combat hooks and sidecar persistence; audit old clock/map item IDs against current inventory and multiplayer. |
| Optional starter progression guide | `goal-track`, `af63a469` | `assets/goals.ron`, goal evaluation, persistence and Goals HUD/page are absent. Current quest tracking serves a different purpose. | Reuse useful condition/table logic while preserving optional tracking and the current UI; do not restore an always-on objective HUD automatically. |
| Data-driven biome family weights and biome-aware site selection | `biome-p1` (`9917bffc`, `ad18629f`, `d69af9a3`), `sites-enum` (`44cbbf20`) | No `assets/biomes.ron`, content biome table or weighted-family module. Current `landscape/biomes.rs` uses code-defined ecological variants; site kinds remain numeric. | Preserve current climate/geology and existing worlds. Old gates 56/66/69 cannot simply acquire new generation behavior now. |
| Abandoned Anchor districts and regional road network | `anchor-ruins`, `d013536b`, `fd920f9c` | District layout/hull/tower/port/road modules are absent. Current freight structures, lore and security robots do not implement this district generator. | Branch still uses collided gate 55; registry records pending 67. Adapt to current terrain, materials, placement and saves under a fresh applicable gate. |
| Four chest tiers and animated lids | `chests`, `6dbf8fa9` | Current `world/block_data.rs` provides a working 27-slot generic chest. The old branch adds wood/copper/iron/steel tiers, extra rows and lid animation; those extensions are absent. | Retain current scanner, storage and multiplayer behavior. Do not replace current container/save ownership with the old implementation. |

These are recovery candidates, not approved replacements or claims that the old
code is ready to merge unchanged. Some may have been superseded by product choices.

## Important code already represented in the shared engine

- Player edit fast lane, source patching and latency instrumentation from the
  latency branches are present in streaming and the player-edit lane.
- Resident source comparisons ignore disk-record churn while voxel ownership is
  unchanged, in both local sources and quality proxies. Simulation residency has
  the bounded pressure-admission escape. The dirty mining-backlog worktree is not
  holding a missing production fix in these areas.
- Horizon altitude hysteresis and retained refinement midpoint thresholds from
  scratch session `a91dea34` are present. Its mana HUD correction is also present.
- The pressure/moisture weather solver and storm spin/wind edits are now present
  in `worldgen/sphere/weather` and the orbital `shell_sim` consumer. Their source
  presence supersedes the earlier audit's pending weather-recovery status.
- Fine/coarse sky visibility volume code is present, including
  `render/sky_volume_cells.rs` and the coarse review switch.
- The stepped/faceted butte production changes in `D:/voxel-geo` are represented
  in the current vistas modules; remaining differences include timing scratch code.
- F3 functionality exists under `egui_ui/debug_overlay.rs`; the absent older
  `f3_overlay.rs` filename is not a lost feature.

## Differences that should not be imported automatically

- The old crown-culling patch inflates `tree.radius`; current code documents that
  the radius already includes the leaf envelope and searches the generator's full
  neighbouring-cell range. Copying the old multiplier would add conservative work.
- Gate-132 classic crowns intentionally roll back later crown generation. That is
  an alternative visual direction, not a universally beneficial missing fix.
- Random-seed sweep improvements and weather cloud checks remain partly stranded
  as test-only code. They were inspected but not copied or run under the user's
  standing instruction against adding/running tests without a request.
- Whole-tree snapshots and the water scratch overlay contain stale peer code and
  diagnostics. Their branch distance from HEAD is not a reason to merge them wholesale.
