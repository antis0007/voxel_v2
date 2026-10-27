# Player-freedom addendum 07 (parked)

Status: demoted to reference by the human on 2026-10-26
([decision 0034](../decisions/0034-player-freedom-addendum.md)). The brief is
[here](07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md). Nothing below binds a milestone.

## Claim rows, as removed from master plan §4

Source: [docs/NEW/07](07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md), supplied
2026-10-26 after the freeze; routed under existing owners by
[decision 0034](../decisions/0034-player-freedom-addendum.md), which the human
demoted to reference on 2026-10-26. Audit and slice design:
[2026-10-26 player-freedom note](../superpowers/specs/2026-10-26-player-freedom-economy-design.md).
The addendum also revises C-MC5 (no AFK farm or external guide), C-N3
(evidence-based secrets), C-N4 (useful emergent behaviour, not combination
counts), C-Z2 (meaningful density), C-S1 (machine experiments count), C-S4
(no unwarned loss) and C-F3 (optional permadeath leaves the default world
safe); those rows keep their obligations above.

| Claim | Owner | Milestone | Named scenario and acceptance obligation |
|---|---|---|---|
| C-PF01 | P14 + P12 | M4 | `player_freedom_home_without_farm`: two-hour newcomer builds a home, repairs, and runs a first machine with no hostile farm and no external guide; human cohort |
| C-PF02 | M16 + P05 + P11 + W04 | M4 slice; M5 full | `explorer_building_materials`, `tnt_material_recovery`, `trading_no_infinite_arbitrage`: ordinary stone, sand and metal reachable by caving, salvage, machines and trade; blasts recover declared mass; no infinite buyback |
| C-PF03 | M01 + M16 | M5 | `technical_factory_honored`: automated ore line scales past hand collection with no arbitrary cap; throughput, power and bottlenecks recorded |
| C-PF04 | W05 + W06 | M6 | `legendary_site_not_loot_only`: a site keeps a use after its caches open (water, power, route, equipment) and the restored state saves |
| C-PF05 | B09 + P14 | M8 | `science_discovery_via_drone`, `cooperative_discovery_credit`: discovery credit is never gated on actor identity; a robot's valid experiment is recorded |
| C-PF06 | N10 + X01 + G08 | M3 base; M8 | `humane_material_alternatives`, `golem_guard_vs_scrap`: nonlethal routes supply ordinary needs; a working guardian is worth more than its scrap; ingots come from metallurgy |
| C-PF07 | N08 + X02 + W05 | M6 | `regional_peace_victory`: a secured settlement stays safe across nights, reloads and generator versions; threats persist beyond it |
| C-PF08 | P02 + P03 + P04 + P06 | M3; M4 | `inventory_first_hour`, `starter_cooking_intrinsics`: no compulsory hunger loop by default (product decision pending), early containers, infinite repair with materials |
| C-PF09 | P13 + P02 | M3 | `normal_death_recovery`: standard death keeps unique gear and structures; respawn point survives a reload; no death spiral |
| C-PF10 | W03 + W02 | M6; M9 | `new_deep_frontier`: new depth or distance changes systems, not HP and palette; blind review across six seeds |
| C-PF11 | P14 + P15 + W06 | M4; M9 | `coherent_rare_hint_chain`: every procedural secret has two or more discoverable clue sources and an in-world experiment; no wiki dependency |
| C-PF12 | M01 + C17 + C16 | M1.5 | `afk_vs_adventure_factory`: identical factories produce identical ledgers with the player beside, far away, or the machine unloaded; automated now as `world/factory/authority_tests.rs` |

## Scenario rows, as removed from the claim scenarios README

Proposed by [decision 0034](../decisions/0034-player-freedom-addendum.md)
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
