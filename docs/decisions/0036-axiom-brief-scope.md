# 0036: Scope of the revised Axiom agent brief

Status: accepted by the owner, 2026-10-27.

## Context

The pasted "Axiom Revised Agent Brief" (M0 integration to M5 lighting, plus wand
and Meridian tasks) was audited against the repository in
`docs/plans/2026-10-26-axiom-brief-reality-check.md`. The audit found that HEAD
builds and tests (898 pass, 3 fail), that a third of the brief asks for systems
that exist, and five conflicts only the owner could settle (§11 there).

## Decisions

1. **Tests and gates.** Work under the brief runs and adds tests and local
   `just` gates. No hosted CI: the repository has no remote that would run it.
   The "do not add or run tests" line in the Axiom execution ledger does not
   apply to this work.
2. **Scope.** The audit's reordered plan (§12), not the brief as written.
   M0 narrowed to clone completeness and the HEAD failures; M1 is a frame-time
   baseline and gate on the existing `--benchmark` path; M2 amends existing
   contracts; Meridian distance waits for generator 65 to be committed.
   M3 (mixed-scale chunk octree, palettes) and M4 (30 tps, TOML, rapier3d)
   are not started: ADRs 0002, 0007, 0011 and 0018 stand, and simulation work
   stays in MASTER_PLAN M1. M5 starts as consolidation of the nine fog paths.
3. **Texel density.** Held items and the art bible use 4.5–5.5 mm per texel
   (`PIXEL_WORKSHOP_RESET_20261026.md`; the wand kit draws 5.04 mm). The
   48–64 px/m figure in the art production prompt pack is superseded for held
   items.
4. **Text register.** The brief's modded-pack register, written into
   `crates/app/src/egui_ui/UI_RULES.md` (rules 12, 14, 15 and the tooltip
   contract). Strings stay in `assets/*.ron`; no TOML string table.

## Consequences

- `just deps` joins `just verify`; `just frame-bench-check` gates render changes.
- Agents given the brief verbatim follow this record where they differ.
- Machines may carry one flavour line; flavour never states mechanics.

## Reversal

Revisit 2 if the owner supersedes ADR 0007 or a profile shows the chunk
layout, not cloud and shadow passes, dominates frame time. Revisit 3 if a
native capture set at the bible's reference poses shows 5 mm reads worse
than a coarser density.
