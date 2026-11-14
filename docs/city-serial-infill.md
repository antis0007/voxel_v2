# Serial demand-funded infill

Status: guarded-integrated, verified on the actual shared-source executable and promoted through the existing city launcher. The previous v5 executable is retained.

## Playable scope

Version 2 growth keeps the original six-house bootstrap and its 64 physical residents. Unhoused residents create demand for two additional four-bed homes, funded and constructed one at a time. Both parcels and their paid access spurs are generated from current clear ground and existing paid frontage. Housing rises from 24 to 32, leaving 32 residents unhoused. The visible final state is `Infill Limit: 2 Homes Complete`, not a claim that demand is satisfied.

The planner reuses `quarter/layout/route`, planning profiles and the existing small-house geometry. It does not use the separate styled district fixture, grant roads or materials, clear terrain, move old houses, increase the 24 m service radius or add residents. The original colony content is unchanged. A proposed compact-layout experiment was dropped after correcting an overly conservative geometry audit; it is archived outside the repository.

One saved plan records seed, profile, exact origins, orientation, stair side, rectangular reservations, entrance, paid street anchors, access cells, provenance and parcel-to-project indices. Exact geometry and order validation rejects corrupted or reordered saves. Version 1 growth remains supported with its existing one-house semantics.

Only one project is funded at a time. Completed projects and household assignments remain stable. Before funding, the parcel is revalidated against current terrain, forestry, production reservations and paid structures. A changed unfunded proposal can be discarded and reconsidered without a material refund. Exhausted searches retry on fixed ticks; material waiting does not repeatedly route streets. Existing chest reservations, cargo claims, production bills and construction remain the only material authority.

## Reload defect fixed

The new between-home checkpoint exposed a saved worker mid-step at feet `(0.49975303, 18.81529, 2.4242027)`. The old restore path required ground within 0.2 m and left resident 31 absent. The saved roster remained 64, but only 63 actors returned.

Saved workers now restore at their exact collision-free, loaded pose, including an airborne pose. Unsafe or unloaded poses wait rather than teleport home. Ordinary newly spawned residents and guards still require support. Normal gravity resumes after restoration. No pose tolerance or roster requirement was relaxed.

The native probe covers sidecar serialization, exact identity/kind/pose, duplicate prevention, normal gravity and landing, solid obstruction, unknown chunks, deferred retry and unchanged ordinary/guard support. Replaying the actual failed checkpoint restored all 64 actors after 60 ticks with zero first-position error.

## Verified private evidence

Executable: `task/city-neighborhood-v3.exe`, SHA256 `5D654536C64481FF0E0FB7881EA8705D169E103AF32C836013EC754BF6850049`. Build: existing dev dependency cache, application opt-level 1, debug information disabled, 64 codegen units. Native build completed in 5m 03s with 424 existing warnings. This is not a ship-release or rendered-performance benchmark. Private saves are version 67; shared storage is version 68 and must not be overwritten.

- `task/city-worker-reload-v3-20261107/worker-reload-regression.json`: PASS targeted restoration/collision/loaded-world/duplicate/support probe.
- `task/city-worker-reload-v3-20261107/worker-reload-check.json`: PASS exact formerly failing checkpoint, 64 actors, zero first-pose error.
- `task/city-neighborhood-legacy-v3-20261107/neighborhood-check.json`: PASS from the existing six-house checkpoint, 222691 ticks, 43.527 s measured wall time.
- `task/city-neighborhood-fresh-v3-20261107/colony-check.json`: PASS unchanged bootstrap, 206130 ticks, 763 paid cells, six connected interiors, 64 actors, food/timber conservation and cargo/full reloads.
- `task/city-neighborhood-fresh-v3-20261107/neighborhood-check.json`: PASS fresh continuation, 235471 ticks, 39.578 s measured wall time.
- Existing v1 regression on immutable v1 of this increment: `task/city-neighborhood-v1-legacy-growth-20261107/housing-growth-check.json`, PASS 160291 ticks, 91 paid cells, housing 24 to 28, 64 live residents and conserved food/timber. The final shared binary still needs its own regression.

Both two-home runs verify 182 new paid cells (180 planks and two chests), including access. Every new local plank/chest cell must belong to a paid order; all original geometry remains. Food and timber accounting includes physical cargo, drops, placed cells, tree regrowth and canonical felled bodies. All 64 identities move, no sampled discontinuity is accepted, and all eight new occupants complete their daily cycles.

Each active build is obstructed for 48000 ticks, including a full save/load with actual claimed cargo. All 24 existing occupants complete a new ordered meal, rest, social and subsequent productive-work cycle during the first build; all 28 do so during the second. The checker does not combine old counters from different cycles. Completed geometry and home assignments are frozen before later admission. It verifies unfunded and funded sidecars, first/second cargo reloads, a between-home reload and final reload. First rematerialized poses are observed before adventure movement with the unchanged 0.025 m tolerance. Grounded player-physics round trips to both new interiors and back to the stockpile pass (57.320 m and 64.448 m in the fresh run).

## Bounded work and timings

The verified plan uses one survey, 9604 cheap cached candidate checks, 37523 counted terrain/anchor inspections across survey and revalidations, and nine street-route requests. The combined reserved route node budget is 180000; this is a cap, not a measured node count. Routes are not rerun while waiting for materials. At most 24 route requests are permitted per survey; no-space retry is 7200 fixed ticks.

Fresh continuation gameplay mean/p95/p99: 0.083/0.165/0.302 ms; maximum 7.574 ms. Physics/finish mean/p95/p99: 0.060/0.060/1.746 ms; maximum 29.602 ms. Legacy continuation gameplay mean/p95/p99: 0.098/0.186/0.355 ms; finish: 0.069/0.065/2.088 ms. Native negative planning probes, covering multiple intentional mutations and obstructed surveys, take 58–61 ms in total. These contended headless workload observations are not FPS, comparative speed claims or evidence for hundreds of residents.

Matched fixed-weather automated captures were visually inspected at `task/city-neighborhood-{before,after}-v3-20261107/saved-world.png`. The two added houses, paid access and 24-to-32 housing change are visible. The panel's Nearby count is its distance-filtered roster relative to the review camera, not the full simulation count tested above.

## Reproduce

Run from the task directory, using a new output path each time:

```powershell
.\Replay-CityWorkerReload.ps1 -Executable city-neighborhood-v3.exe -OutputRoot <new-task-local-output>
.\Check-CityNeighborhood.ps1 -Executable city-neighborhood-v3.exe -Checkpoint city-colony-step-fixed64-20261107\disposable-saves -ExpectedHomes 2 -OutputRoot <new-task-local-output>
.\Check-CityNeighborhood.ps1 -Executable city-neighborhood-v3.exe -Fresh -ExpectedHomes 2 -OutputRoot <new-task-local-output>
```

Failed v1 evidence is retained, including the 63-actor reload, a deliberately incorrect one-home expectation, and the unneeded compact-layout experiment's doorway regression. The final patch removes the experiment rather than weakening existing access tests.

Limits: single-player, one-metre local service area, at most eight total home projects and 64 workers. This is not unrestricted urban planning, immigration, additional jobs/services, a full Songs of Syx port, warfare or multiplayer authority. No numerical gameplay-quality rating is claimed without player testing.

## Verified shared integration and launch

Private feature commit: `cca29a6d544494620629680434a6bba087d78c09`. The 28-file package was applied only after an idle compiler window and exact current-file hash checks. The narrow saved-worker patch was composed onto the current animation file, not copied over it. Shared storage remained version 68; onboarding, panels and unrelated source were preserved. Originals and hashes are retained in `task/city-neighborhood-integration-v3-20261107/manifest.json`.

Actual shared executable: `task/city-neighborhood-shared-v4.exe`, SHA256 `25C87FD4DE8BE3165CD07412417D3535D2A71AED4CE5B702E8EDB7713FD40698`. Build: release-fast dependency cache, explicit application opt-level 1 and 32 codegen units; 3m 14s, 428 warnings. No ship-release performance claim.

Final shared gates:
- Fresh bootstrap: six paid houses, 763 cells, 64 actors and all access/conservation/reload checks pass.
- Fresh and existing version-68 checkpoint continuations: both pass 235471 ticks, 182 paid cells, 32 housed residents, all 64 moving identities, all eight new daily cycles, all 24/28 prior residents cycling during each build, strict first-pose/cargo/between/final reloads and exact food/timber balances.
- Native airborne-worker, collision, unknown-chunk, duplicate and support regressions pass inside both runs.
- Previous one-house mode passes 166231 ticks, 91 cells and housing 24 to 28.
- No-growth daily control passes the full 180000 ticks with all 24 housed residents cycling and conserved resources.
- The 27-plan district matrix, six grounded roof round trips and paid courtyard construction (441 cells, completed-worker route replays) pass.

Fresh shared continuation: 35.765 s measured wall time; gameplay mean/p95/p99 0.070/0.150/0.255 ms, maximum 6.549 ms; finish 0.061/0.054/1.704 ms, maximum 23.621 ms. Planning-negative probes total 41.375 ms. These are contended headless stage observations, not rendered FPS or comparative performance.

`task/Play-CityGrowth.ps1` now starts the original fresh 64-worker scene with version 2 growth. Existing version-1 growth saves are not automatically migrated. Both `city-growth-playable.exe` and `city-colony-playable.exe` match the verified shared binary. The prior immutable `city-growth-shared-v5.exe` and `Play-CityGrowth-before-serial-infill-v4.ps1` remain available.

Matched final overview captures, visually inspected: `task/city-neighborhood-{before,after}-overview-shared-v4-20261107/saved-world.png`. Both new homes and their paid access are fully visible; the panel shows 64 nearby residents and housing 24 to 32.

All exact paths, checks and executable provenance are indexed by `task/Axiom-City-Neighborhood-Verified-20261107.json`. Source whitespace checks and `just session-report 95f35d4` pass. The session report includes pre-existing untracked baseline source; its whole-tree LOC/test totals are not this increment. The whole workspace test suite is not claimed as run.
