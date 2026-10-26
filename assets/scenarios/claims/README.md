# Claim scenarios

One scenario per bible claim, named as in the [master plan](../../../docs/MASTER_PLAN.md) §4
(for example `material_only_orichalcum.ron` for C-MC3). Each scenario states the
claim ID, the systems it exercises and its pass condition.

Active in M1–M3, per [decision 0022](../../../docs/decisions/0022-master-plan-review.md):

| Claim | Scenario | Kind |
|---|---|---|
| C-MC3 | `material_only_orichalcum` | automated |
| C-MC5 | `first_automation_hints` | automated (bot) |
| C-Z3 | `gate_three_routes` (M3 slice) | automated |
| C-Z4 | `grounded_room_corpus` | human review |
| C-S1 | `primer_siphon_mastery` | automated |
| C-N5 | `noita_physics_review` | captures + human review |

Other claims get no scenario work until their milestone. `just claims` runs the
automated ones; it is created in M3.

Proposed by [decision 0034](../../../docs/decisions/0034-player-freedom-addendum.md)
(player-freedom addendum, master plan §4 C-PF rows). Only the first is
automated; it runs as a Rust test because the runner has no machine step or
ledger check yet (`Step::Machine`, `Check::MachineOutput` are the missing
pieces).

| Claim | Scenario | Kind |
|---|---|---|
| C-PF12 | `afk_vs_adventure_factory` | automated: `crates/app/src/world/factory/authority_tests.rs` (`just test authority`) |
| C-PF09 | `normal_death_recovery` | automated storage fixture (M3) |
| C-PF02 | `explorer_building_materials`, `tnt_material_recovery`, `trading_no_infinite_arbitrage` | automated once inventories are checkable (M4, M5) |
| C-PF01, C-PF08 | `player_freedom_home_without_farm`, `inventory_first_hour`, `starter_cooking_intrinsics` | human playtest (M4) |
| C-PF03 | `technical_factory_honored` | automated throughput (M5) |
| C-PF04, C-PF11 | `legendary_site_not_loot_only`, `coherent_rare_hint_chain` | automated plus human cold run (M6, M9) |
| C-PF05 | `science_discovery_via_drone`, `cooperative_discovery_credit` | automated (M8) |
| C-PF06 | `humane_material_alternatives`, `golem_guard_vs_scrap` | automated plus playtest (M8) |
| C-PF07 | `regional_peace_victory` | automated persistence (M6) |
| C-PF10 | `new_deep_frontier` | human review across seeds (M9) |
