# Repeatable demand-funded growth

Status: combined shared executable, serial default/alternate-profile native gates, legacy regression and matched captures passed. Playable promotion is tracked separately in the task evidence index.

## Scope

Growth version 3 replaces the two-parcel goal with repeated demand review. It ranks current buildable envelopes by connection distance to already paid frontage, then footprint-center distance to the stockpile and deterministic provenance. Only one home is proposed and funded at a time. Completion must reconcile actual cells, the outstanding queue and cargo claims before another survey begins.

The original six-house colony, 64 physical residents, crops, forestry, single stockpile and 24 m supply radius remain unchanged. A 16-project ledger ceiling bounds saved state; it is not a demonstrated capacity. The original local layout has room for three additional small homes under the current reservations. Its available ground then runs out, leaving 28 residents unhoused.

The old two-home ordering spent the only remaining near parcel on a longer access spur. Existing version-1 and version-2 cities retain their exact semantics and geometry. Version 3 never moves or removes those homes or roads. Select VOXEL_CITY_GROWTH=3 only when enabling a new growth state.

## Bounded planning

Each fixed-tick planning poll examines at most 64 of the 9,604 origin/orientation tuples. It keeps at most 16 ranked candidates and a resumable cursor, not a persisted terrain grid. Physical columns are memoized only within that poll. A full ranked sweep precedes admission, so the first arbitrary feasible candidate cannot consume a better near-frontage site.

Routing uses the existing district street router and profiles. A poll makes at most two route requests. At most 24 requests are permitted per retry episode; a budget pause retains its ranked cursor. Each route reserves a conservative 20,001 expansions because the shared route's existing limit test aborts after 20,000. This figure is a cap, not measured expanded nodes.

A true empty geometric sweep reports No Space. Missing usable paid anchors reports Street Disconnected. Failure of limited frontage-route probes reports Street Search Limit instead; it does not pretend all possible connections were exhausted. Both retries use fixed simulation ticks. Material waiting performs no survey or route work. Funding revalidates the actual footprint, paid anchor and access cells against current ownership and terrain.

All building cells, access, chests and material withdrawals use existing production, inventory, reservations and construction. There are no terrain edits, resource grants, copied scenery roads, extra residents or refunds created by replanning.

## Save contract

Version 3 requires infill schema 2; older growth versions continue requiring schema 1. Saved records include append-only project indices, exact building geometry, paid anchor project/order indices, access, parcel provenance and bounded search progress. Malformed cursor, shortlist, schema, geometry and commitment records are rejected. Coordinate bounds are checked before height arithmetic.

The native checker verifies mid-sweep and shortlisted full reloads, unfunded/funded sidecars, actual cargo with obstruction, a full reload between every home, and final reload. First resident poses are checked before movement with the existing tolerance. The whole jobs sidecar must remain within 256 KiB.

## Integration boundary

This planner shares the district router and profiles. The separate styled district fixture still uses a different construction/material authority and its saved state forbids a frontier colony. It is not installed or converted by this increment. Styled multi-material buildings, new service centers, larger supply areas and arbitrary terrain expansion remain separate work.

The concurrent voluntary-groups slice is opt-in and has its own native gate. Its presence in a combined executable is not evidence that groups were exercised by the default city run.

## Initial diagnostic evidence

The first immutable private binary was task/city-fetch-batch-20261107/city-fetch-batch.exe, SHA256 7AA81E51392944199AC5548DF63C96174E1AAB99667DE68919B3549490E2DBD7. It includes the independently tested stable construction-batch optimization, commit 90eb1e6.

The diagnostic run task/city-repeatable-v1-20261107 completed all three 88-cell homes, housing 24 to 36. All 64 actors remained active, all 12 new occupants cycled, and all 24/28/32 prior occupants completed ordered daily cycles during the respective obstructed builds. All grounded round trips, original-home stability, physical payment, food/timber balances and reloads passed. The overall gate remained failed because its final observation landed during a background resurvey rather than the required exhausted pause. That acceptance timing condition was corrected for the next build; the failure is retained rather than relabelled as a pass.

The initial run measured at most 64 candidate checks, one route request and 3,595 counted cell inspections in one poll. Planning-tick gameplay mean/p95/p99 was 0.581/1.050/2.777 ms, max 6.730 ms. These are contended native stage observations, not rendered FPS or a ship-release benchmark.

## Retained timing failure and bounded stress contract

The first actual shared run, task/city-repeatable-fresh-shared-v3-20261107, completed three paid homes and passed active-actor, access, payment, conservation, new-occupant and reload checks. It failed the existing-resident cycle timing condition: resident 20 missed project 6; residents 15, 20 and 28 missed project 8. Each had eaten, rested and socialized, then resumed productive work later. The failed JSON remains unchanged. This is evidence of latency outside that observation window, not proof of permanent starvation or fairness.

The later v3-only stress checker retains a real Stone obstruction for at least 48,000 ticks. It records every still-missing resident at that boundary and each resident's first qualifying cycle-completion step. It releases the obstruction only after every prior resident completes an ordered meal, rest, social visit and subsequent productive action, or fails at an immutable 144,000-tick deadline set when the obstruction was placed. That is a maximum 20 simulated minutes. It does not add work or goods, reset baselines, or credit activity after project completion. Version-2 checks retain their original fixed 48,000-tick obstruction.

This is a disclosed change in the stress-test contract, not a relabeling of the earlier fixed-window failure. It does not prove a bound on normal unobstructed scheduling latency.

## Shared executable evidence

The combined shared v4 executable is task/city-repeatable-shared-v4.exe, SHA256 42DA10C5E014AD17540E23BF7BF7FC921C672CB255624D6D8B8ECE8C8AE993A6. Its release-fast build passed in 3m52s with 428 warnings. Shared save version 68 and protected onboarding, terrain, wildlife and panel work were preserved. The combined source manifests record exact hashes and separate group integration provenance.

The fresh original bootstrap passed: 206,130 ticks, six homes, 763 paid cells and 64 physical residents. The continuation in that overlapping test run stopped at the third cargo checkpoint's existing 30-second wall-time save deadline. Its failure remains at task/city-repeatable-fresh-shared-v4-20261107. No deadline was increased; a new serial continuation uses the exact immutable fresh bootstrap checkpoint and same binary.

The alternate Organic profile 2, seed 424242 passed at task/city-repeatable-organic-shared-v4-20261107: 377,671 ticks, three 88-cell homes, housing 24 to 36, all 64 actors moving, all 12 new occupants cycling, all prior 24/28/32 residents cycling during the respective builds, conservation, grounded access and all reload phases. Obstruction holds were 53,702 / 48,660 / 48,000 ticks. The 48,000-tick snapshots retained resident 20 as late for each of the first two projects. The final jobs record was 61,445 bytes.

That contended Organic run measured gameplay mean/p95/p99 0.168/0.321/2.333 ms and physics/finish 0.135/0.165/3.544 ms. Planning-tick gameplay measured 0.756/2.765/5.467 ms, max 9.617 ms. At most 64 candidates, one route request and 3,595 counted cell inspections occurred in a poll; five exhaustion episodes and 28,797 scan-free cooldown ticks passed. These are native stage timings under concurrent workload, not rendered FPS or a clean performance comparison.

The default serial continuation passed at task/city-repeatable-default-serial-v4-20261107 using the same binary and unchanged save deadline: 377,671 ticks in 92.988 seconds, all the same physical construction, daily-cycle, conservation, access and reload assertions. Gameplay mean/p95/p99 was 0.115/0.239/0.574 ms; physics/finish was 0.093/0.082/2.396 ms. Planning-tick gameplay was 0.493/0.813/2.548 ms, max 7.909 ms. These are simulation-stage observations, not rendered FPS. The overlapping save timeout is still a retained reliability limitation; the successful serial retry does not prove that contention problem fixed.

Exact old version-1/version-2 growth retention and all 64 saved poses passed on v4. The 27-plan district matrix, six grounded roof routes and full 441-cell courtyard construction also passed. Groups passed their separate actual-shared eight-worker gate and full-world reload; the city growth checks leave groups disabled. See task/group-shared-v4-final/SUMMARY.md for that independent scope.

## Reproduction

Use Check-CityExpansion.ps1 with a new task-local output and either -Fresh or an existing six-house checkpoint. ExpectedHomes=0 runs until repeated true exhaustion; it requires at least three physical additions. Existing v1/v2 checks remain separate regressions. Never edit a save header to cross private version 67 and shared version 68.

Native acceptance is the executed gate for this increment. Workspace-wide tests, workspace Clippy and the performance-baseline suite were not run. Four focused Rust test functions were added but are not claimed as executed; native probes cover malformed state, cursor progression, blockage, demand, material starvation and reload. The independent construction batch owner ran its differential selector tests.

The unchanged version-2 two-home continuation passed on the final executable: two homes, 182 paid cells, eight new occupants cycling and conservation/reloads preserved. Evidence: task/city-repeatable-v2-regression-shared-v4-20261107.

Matched 1280x720 automated loaded-world captures were visually inspected at task/city-repeatable-before-v4-view-20261107/saved-world.png and task/city-repeatable-after-v4-view-20261107/saved-world.png. The before view shows six houses and Housed 24/64. The after view shows the three additional near-frontage houses, living forestry/farming, and Housed 36/64. The visual style remains an early block-built prototype.

The normal fresh launcher passed its automated start/render smoke and was visually inspected at task/city-repeatable-default-launch-v4-smoke-20261107/bench-000.png. It correctly begins with 64 physical residents, zero houses and finite authored timber. This short startup check is not a full interactive playthrough. Old saves and old immutable executables are retained.

An additional optional 650,000-tick preview was stopped and its logs retained at task/city-repeatable-launcher-v4-smoke-20261107. Read-only inspection found that the existing quarter/check.rs preview helper omits main.rs's frame-level dropped-stack settling and stops when the current planned project list completes. Therefore that fast-forward helper is not accepted as continuous-growth evidence. The native city checker supplies the same dropped-stack fall rule used by the normal window loop. Centralizing that rule and fixing preview completion semantics remain follow-up work; this increment does not claim the preview defect fixed. No unrestricted city, full society, multiplayer or numerical gameplay-quality claim is made.
