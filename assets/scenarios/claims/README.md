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
