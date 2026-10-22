# 0003 Budgets replace wall-clock deadlines; headless scenarios are the test harness

Date: 2026-10-22. Status: accepted.

Simulation passes compared `Instant::now()` with deadlines, so the work a tick did depended on machine speed. Every such loop now takes an `engine_core::Budget`. The window passes the same wall-clock budgets as before (unchanged behaviour); tests and the headless runner pass `Budget::UNBOUNDED`.

`--headless=<scenario.ron>` (`just run`) builds a seeded world, applies steps and checks state with no GPU, waiting for chunk generation each tick. Scenarios in `assets/scenarios/` have `insta` snapshots of their JSON summaries, including a world hash. Dependency added: `insta` (dev).
