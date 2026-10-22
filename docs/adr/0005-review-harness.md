# 0005 Review harnesses stay until a scenario reproduces each scene

Date: 2026-10-22. Status: accepted, in progress.

The game ships ~4k lines of review modules (`*_review.rs`, `enemy_review`, `material_studio`), ~70 `VOXEL_*` environment hooks and ~40 `--benchmark-*` flags that set up scenes for visual captures driven by `tools/Review-*.ps1`. They are the project's visual evidence workflow, so they are not deleted blind.

Plan: a `--benchmark-scenario=<ron>` bridge sets world state from a scenario file before the existing capture loop; each `Review-*.ps1` is converted to a scenario plus the generic capture flags; the per-scene module and its env hooks are deleted only after its script runs through the bridge with the same scene. Ranked in `docs/PLAN.md`.
