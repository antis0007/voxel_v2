# Paid service-area expansion

Status: guarded shared integration, shared build and serial shared native service, old-growth and group gates passed. The normal-play upgrade uses the tested authority. Versioned executable and promotion evidence are in the task evidence index.

## Scope

The Growing settlement panel offers Expand Services when a growth-3 settlement is locally exhausted and residents remain unhoused. The button uses the same checked service-upgrade authority as native acceptance; it does not grant goods or change old saves automatically.

Growth mode 4 explicitly upgrades a settled version-3 colony after local infill has exhausted its service area and actual residents remain unhoused. It admits one paid construction depot and one eight-place timber house. It keeps the original food, farming, ordinary hauling and production stockpile. No people, building materials or terrain are granted.

The new depot becomes a construction source only after its chest and approach are physically complete. One courier transfers actual primary-chest stock in batches of at most eight. Each carried stack retains its source and purpose through interruptions and saves. Remote orders cannot consume primary-origin or unmarked legacy cargo.

Existing modes 1, 2 and 3 retain their previous behavior and project limits. Mode 4 preserves the concrete old project prefix and uses its own saved stage and content provenance. The global save header is unchanged.

## Bounds

- Four content candidates, processed one per planning poll.
- At most one missing chunk admitted per planner invocation.
- A route query reserves at most 1,100 nodes. Reported reservations are caps, not observed node expansions.
- One active courier and one funded construction project.
- At most 1,024 cells per new project, 4,096 cells in the project ledger, and 16 total projects.
- The active construction queue remains limited to 256 orders.
- Construction remains within 24 metres of its assigned chest; the existing colony navigation and activation envelopes are unchanged.

The canonical first candidate has 597 paid cells: 114 Log, 481 Planks and two Chests, including the depot, access and safety apron. The largest layer is 217 cells. Actual-source planner tests and native iteration 2 verified the bill and full paid construction; occupancy and complete daily-life acceptance remain required before promotion.

## Acceptance

The disposable native runner is Check-CityServices.ps1 in the task directory. It loads a copy of a compatible nine-house checkpoint, exercises the real fixed-step simulation and writes service-expansion.json. It checks physical cargo, ownership probes, obstructions, full checkpoint reloads, the prior homes and routines, all 64 actors, new occupant cycles, conservation and grounded routes. Stage timings are not frame-rate measurements.

The private city-repeatable-v2-20261107 checkpoint uses save version 67. The shared verified checkpoint uses version 68. They must be run with matching executables; headers must not be edited to cross versions.

## Limits

This is one finite service expansion, not unrestricted town planning. The house is an unfinished open-window timber adaptation of the existing one-storey district geometry. There is no new population, multiplayer or alternate inventory/job authority. Promotion requires native acceptance and a guarded shared-source merge followed by shared-binary regression.

## Retained failed run

Private iteration 2 (city-service-native-v2-20261107) reached 597/597 paid cells with food and timber conserved, 64 moving residents, actual production targets raised and restored, and courier/builder cargo reload and obstruction recovery checks. It did not allocate the eight new occupants within 1,000,000 steps, so the overall gate failed. Gameplay-only timing was mean 0.18783 ms, p95 0.3059 ms, p99 1.7509 ms; this is a failed scenario baseline, not FPS or completed gameplay.

An actual-source corridor regression reproduced level interpolation at Y=18 returning 17.999998, causing the collision query to sample the paid floor. The narrow fix preserves equal level height exactly while retaining X/Z interpolation, margins, support and headroom checks. Blocked doorways and missing support remain rejected. Iteration 3 also batches immutable service queries once per worker call, retaining live stock and capacity reads; its exact cached/uncached and debit probes must pass natively.

The focused Cargo test attempt is blocked by 28 pre-existing test-only API errors outside this increment, including egui, rendering and factory/mining/wildlife test callers. Focused actual-source tests and native executable checks do not establish a full workspace test or Clippy pass.

## Private iteration 3

Native service-expansion.json passed at 776,341 steps in 318.45 seconds. All 597 cells were physically paid at step 684,959; eight new residents were allocated at 684,989, increasing housing from 36 to 44. All 64 residents moved, all eight new residents completed ordered meal/rest/social/productive cycles, and prior households remained unchanged. Five full reloads, a retained obstruction, food and timber conservation, and an 89.87 m grounded stockpile/depot/house round trip passed. Actual production targets were raised and restored.

Gameplay-call timing was mean 0.25223 ms, p95 0.4496 ms, p99 1.6219 ms, max 377.6661 ms. Finish-stage mean was 0.09524 ms. These are stage observations with a retained outlier, not rendered FPS; no net speedup is established against iteration 2 because the completed routines and observation interval differ. The per-worker snapshot probe counted 593 project rows and 64 worker rows once, with exact cached/uncached decisions and live debit/deposit checks.

Nine standalone actual-source accounting/membership tests and eight final actual-source planner/corridor tests passed. The after view is city-service-after-v3-20261107/saved-world.png, matched to the retained city-service-before-v1-20261107 view. The renderer reported a capture-flush timeout, but the requested screenshot and two benchmark frames were saved and visually inspected.

## Final private candidate

city-service-private-v4.exe (SHA256 5E60EA1F2F077ED3E1FC71A7DCCA2FB1126CF41F1496E5864F21A8BEA9E1AFF3) passed the same complete gate, including the new two-worker delivery/death/cache-reset checks. Physical completion, occupation step, total steps and both resident-cycle records exactly match iteration 3. The UI authority was invoked twice to verify idempotence. Merely loading an old save does not upgrade it.

The final run took 349.54 seconds. Gameplay-only mean/p95/p99 was 0.27723/0.5256/2.3045 ms, with a 440.3137 ms maximum retained in the report. Timing variation prevents claiming a net speedup; the exact bounded operation counts are the demonstrated optimization property.

Use the matching new executable for saves after service upgrade. Preserving the global save header means old saves remain readable by the new implementation; it does not make new service cargo and growth state readable by older executables.

Final private old-growth regression passed: three additions, 264 paid cells, housing 24 to 36, all 12 new occupant cycles, conservation and reloads. The independent eight-worker group regression also passed: 104 paid cells, three circles, four accepted offers and exact full-world checkpoint. Evidence: city-service-legacy3-private-v4-20261107 and city-service-groups-private-v4-20261107. The repository session report completed; broad benchmark baselines were not run.

## Actual shared acceptance

Shared save version 68 remains unchanged. Source was applied once after an idle compiler check, exact preimages and git apply --check, preserving current main.rs terrain/network/torch changes and the jobs.rs onboarding hook. Private source commit: 61446e4326fe285371274fcb155808155b746e21. The package city-service-integration-v1-20261107 retains the scoped patch, original bytes and postimage manifest.

city-service-shared-v1.exe (SHA256 A1DBDE7F4E487D4DD7A6D6D9279B99E2FDE24F5670A23FE9BC59B1F87308275E) compiled in 4m03s. Its service gate passed at 949,801 steps in 381.27 seconds with all 597 paid cells, 64 moving residents, housing 36 to 44, all eight new ordered cycles, conserved food/timber and five exact full reloads. Physical completion was step 805,079, with occupation at 805,109. The fixed 1,000,000-step deadline was not extended. The player-sized grounded round trip was 89.87 m.

Shared gameplay-call mean/p95/p99 was 0.24794/0.5273/2.2182 ms, with a 392.4804 ms maximum retained; finish-stage mean was 0.09261 ms. These are not rendered FPS or a speedup claim. The different initial private/shared checkpoints do not establish deterministic equivalence between those runs.

The same shared executable passed the old version-3 growth gate: three additions, 264 paid cells, housing 24 to 36, all 12 new resident cycles, 64 live residents, conservation and reloads, in 417,211 steps. Its independent group gate passed eight productive workers, 104 paid cells, three circles, four acceptances and exact full-world reload. Groups were not enabled in the 64-resident service run. Evidence folders: city-service-native-shared-v1-20261107, city-service-legacy3-shared-v1-20261107 and city-service-groups-shared-v1-20261107.

The actual shared after image is city-service-shared-after-v1-20261107/saved-world.png and was visually inspected. The private before image city-service-button-v4-20261107/saved-world.png verifies the visible Expand Services control. Shared rendering includes unrelated newer atmosphere/UI work, so these images do not establish matched render performance.

The service query snapshot remains an eager, bounded per-worker-call index. Skipping inactive service stages or sharing immutable membership indexes may reduce further work; neither is claimed implemented or benchmarked here. Full workspace tests, Clippy, rendering performance and unrestricted urban growth remain outside the verified claim.
