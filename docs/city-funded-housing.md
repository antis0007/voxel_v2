# Demand-funded housing increment

Status: guarded-integrated into the shared checkout and verified on the actual shared-source executable, with the previous playable preserved. Both independent private checkpoints also passed.

This opt-in local colony increment proposes one four-bed house after six paid bootstrap houses are complete. Existing unhoused workers create demand. It does not spawn residents, clear terrain, grant building materials, increase the worker cap or claim general city planning.

## Authority

The proposal reuses the existing house planner and ascending-layer construction queue. Three content-defined candidate parcels are checked against current terrain, previous projects, production ground, finite timber, the stockpile radius and a bounded route to the paid street. Candidate validation never places a block.

The initial candidate costs 90 planks and one chest. Production requests actual missing materials through existing bills. Funding protects physical stock in the existing chest; it is not a second inventory. Construction fetches, recipe ingredients, market payment and adjacent assemblers respect the reservation. Carried material remains physical and is allocated to specific queued cells. Released claims do not refund stock. Player withdrawals or lost supply leave a visible deficit.

The saved proposal, chosen project, source chest, retry deadline and work counters survive reload. Growth polls use fixed simulation ticks. A completed home is assigned through the existing household ledger. Existing occupants retain their homes and their daily routines while the new project is built.

## Acceptance

`Check-CityHousingGrowth.ps1` in the task directory copies the previously verified paid 64-resident checkpoint into a fresh disposable directory. It never loads a live user save.

The checker requires exact material accounting, one admitted project, pause behavior, blocked-site fallback, all-sites-blocked refusal, protected material consumption, partial/cargo/final reloads, an intervening player obstruction and a grounded street-to-interior walk. It samples actual actors, rejects duplicate or extra identities, and records per-stage timings separately from render performance.

The verified result is 24 to 28 housed residents with the same 64 workers, leaving a real deficit of 36. The test requires new daily-life cycles for all four new occupants and continued cycles among all 24 existing occupants while construction is deliberately interrupted.

## Limits

One authored local service area, three bounded candidate parcels and one additional house are supported. This is not automatic multi-district generation, new employment, immigration, unrestricted terrain planning, warfare or multiplayer authority. Further growth should reuse validated district geometry while retaining explicit needs, physical supply and access checks.


## Verified private evidence (2026-11-07)

Executable: `task/city-growth-roof-v4.exe`, SHA256 `1B203E3150B1D81ED9F4FC6207CE78015C57D5834F3B9D31B9C742FD400E006F`. This combines housing with district/roof commit `f763a73`; optimized development build, not a release benchmark. Compilation completed in 10m 07s under concurrent build contention. No terrain generation files changed.

- `task/city-housing-growth-v4-private-20261107/housing-growth-check.json`: PASS, 158220 fixed ticks, 44.941 seconds measured wall time.
- `task/city-housing-growth-v4-construction-20261107/housing-growth-check.json`: PASS, 160291 fixed ticks, 120.961 seconds with other builds and automated captures active.
- Both runs: exactly 64 live workers, each physically walked at least 2 m, no sampled jumps, 91 paid cells (90 planks and one chest), four new occupants, housing 24 to 28, deficit 36. All four new occupants completed meal/rest/social/work cycles. All 24 existing occupants completed rest/social/work cycles after funding and before construction finished.
- Both runs preserved identities, kinds and first rematerialized poses within 0.025 m after partial-cargo and final full checkpoint reloads. First-pose sampling uses the fixed-loop hook after travel admission and before adventure movement; tolerance was not relaxed.
- An intervening stone obstruction survived 48000 ticks; no replacement cell was paid into it. Construction resumed after the test removed its own obstruction. Site fallback, all-sites-blocked refusal, paused admission, reservation withdrawal/spending/transfer negatives, malformed save rejection and a 7.449 m grounded street-to-interior walk passed.
- Timber and food balances held, including physical cargo, dropped items, tree regrowth and placed materials. The first run began with three canonical logs in an active felled tree. Its occupied cells also appear in the world lookup and are now excluded from raw terrain totals before adding canonical body material once. This corrects a checker double count, not production inventory.

Planner work in both runs: two candidate validations, 720 inspected cells and two bounded path requests with a combined maximum node budget of 2200. The selected site is revalidated immediately before funding. It is not path-searched repeatedly while waiting for stock. The former v3 run issued 351 requests; do not infer an FPS improvement from these contended wall times.

For the first run, fixed gameplay mean/p95/p99 was 0.136/0.218/0.604 ms and physics/finish mean/p95/p99 was 0.105/0.082/2.687 ms. Gameplay timing includes materialization observation and report writes on the few reload ticks. Save/load and admission are reported separately. These are headless workload samples, not rendered frame times or a hundreds-of-residents result.

Reproduce from the task directory:

```powershell
.\Check-CityHousingGrowth.ps1 -Executable city-growth-roof-v4.exe -Checkpoint city-daily-life64-v4\disposable-saves -OutputRoot <new-task-local-output>
.\Check-CityHousingGrowth.ps1 -Executable city-growth-roof-v4.exe -Checkpoint city-colony-step-fixed64-20261107\disposable-saves -OutputRoot <another-new-task-local-output>
```

The private executable uses save version 67. The shared checkout and promoted daily-life executable use version 68. Do not change save headers or copy the private storage module over shared storage; recompile the guarded source changes in the shared checkout before using version-68 checkpoints.

Prior failed v3 evidence is retained. The two checker corrections were reviewed independently; no production movement speed or tolerances changed. Native checks exercise the new save validation cases; the separate `#[cfg(test)]` variants have not been claimed as run.

Regression: `task/city-growth-v4-daily-control-20261107/daily-life-check.json` passed all 180000 requested ticks with growth disabled, 64 live residents, all 24 housed residents cycling and exact food/timber accounting. Matched fixed-weather before/after captures under `task/city-housing-{before,after}-overview-v4-20261107/saved-world.png` were visually inspected; the new house and 24-to-28 housing count are visible.

`just session-report f763a73` and scoped whitespace checks passed. The report counts pre-existing untracked baseline source, so its whole-tree LOC/test totals are not this increment's contribution. The no-argument report cannot resolve a `main` branch in this private repository.

## Shared integration and launch

The 27 housing files from private commit `8cd88f1`, plus the 18 district/roof files from `941b4e0` and `f763a73`, were applied only after exact baseline checks and an idle compiler window. Shared storage remained byte-identical at version 68. Terrain, breaking-tool and unrelated files were not overwritten. Exact originals and all file hashes are in `task/city-growth-integration-v4-20261107/manifest.json`.

`task/city-growth-shared-v5.exe` built from `C:/dev/voxel_v2_backup` in 5m 42s using the existing D: dependency cache. SHA256: `D68E68B123DD203771C7495C70E45621747807DD87B3C0C373CEA2B6B922EBB6`. It uses the release-fast profile with an explicit root opt-level 1 and 16 codegen units; it is not a ship-release benchmark.

- Shared funded-housing gate: PASS, 160291 ticks, 91 paid cells, housing 24 to 28, all 64 moving actors, all 24 original occupants cycling during construction and all four new occupants cycling after completion. Both resource balances, strict identity/pose reloads and obstruction recovery passed.
- Shared fresh baseline: PASS, 206130 ticks, 763 paid cells, six homes, 64 live/moving residents, conserved food/timber and full checkpoint recovery.
- Shared no-growth daily control: PASS for all 180000 requested ticks, all 24 housed residents cycling and both resource balances preserved.
- Shared districts: courtyard 441 paid cells plus roof return, six native roof round trips, 27 plans, timber 530 cells with eight trades, terrace 450 cells, original market 561 cells with eight trades and nine legacy layouts/walks all passed. Old closed-roof sidecar compatibility remained intact.
- The shared housing run used two path requests and 720 cell checks. Gameplay mean/p95/p99: 0.115/0.177/1.586 ms; finish: 0.099/0.067/2.522 ms. Wall time 36.13 seconds. Runs overlapped other work; these are workload observations, not FPS or comparative speed claims.
- Matched shared-source before/after screenshots were visually inspected at `task/city-housing-{before,after}-shared-v5-20261107/saved-world.png`.

`task/Axiom-City-Growth-Verified-20261107.json` links all 13 recorded gates and captures. `city-growth-playable.exe` and `city-colony-playable.exe` both match the verified shared executable. The previous executable is retained as `city-colony-playable-before-growth-v5.exe`.

Run `task/Play-CityGrowth.ps1` for a new 64-resident, six-project settlement with the one-house demand expansion enabled. Every run creates a new task-local disposable save, preserving existing saves. `task/Play-CityDistrict.ps1 -Style Courtyard` opens the separate authored district fixture with actual paid construction; it is not the 64-resident growth scenario.

```powershell
.\Check-CityHousingGrowth.ps1 -Executable city-growth-playable.exe -Repository C:\dev\voxel_v2_backup -Checkpoint city-colony-shared-accepted64-20261107\disposable-saves -OutputRoot <new-task-local-output>
```
