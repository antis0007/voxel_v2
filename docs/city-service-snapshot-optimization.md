# Deferred city service snapshots

Status: integrated into the shared checkout, all native gates passed, and promoted to the existing city playable aliases. The previous release is backed up.

## Change

Service accounting previously built a 593-order and 64-worker index before stockpile validation, routine service, retry waits and ordinary delivery walking. Preparation is now deferred until claim reconciliation, group build review, non-Store task validation or task selection actually needs it. Repeated preparation within one worker call is idempotent.

Completed idle service stages with no queued or claimed build work skip indexing. Completed stages with queued repairs retain membership indexing. A valid ordinary Store path without claims can also skip it; this is not a claim that every carrier skips it. Global claims still require an index before reconciliation.

Worker-entry and exit clearing are unchanged. Stock quantities, capacity and actual debits remain live. No persistent cache, gameplay switch, save-format change or alternate inventory authority was introduced.

## Private equal-workload result

Both binaries ran 240,000 fixed ticks from independent copies of the same completed 11-project, 44-housed, 64-resident private checkpoint. Both passed daily-life, physical movement, conservation and save/reload checks. Final daily-residents.json and pre-rest-checkpoint-residents.json are byte-identical; all compared correctness fields match exactly.

Gameplay-call mean/p95/p99 changed from 0.23324/0.4125/1.209 ms to 0.13824/0.2385/0.825 ms. Mean gameplay cost fell 40.73 percent in this paired private run. Total runner wall time changed from 90.94 to 76.40 seconds; finish-stage means were 0.10467 and 0.10608 ms. These are completed-settlement simulation measurements, not rendered FPS, total-game speedup or proof about active construction.

Evidence: task/city-service-snapshot-opt-20261107/private-comparison.json and its two completed-workload directories. Candidate city-service-snapshot-private-v1.exe has SHA256 39B694E3709BC4BAAF966671FB2DBB60AAFDA8C65315CE768E6A5FEE106DE3FB.

## Correctness contract

The native probe compares eagerly indexed and deferred versions of the same noncommitting worker pulse, including saved and transient state, physical stock, storage revision and dropped cargo. It covers safe walking Store and retry waits. Additional query comparisons cover active preparation once, completed idle metadata and a queued remote repair. The metadata and borrowed-unit setup are explicitly test fixtures, not a claim of constructing a second house.

The full physical service, old growth, voluntary groups and save/reload gates are required before promotion. No full workspace test or Clippy pass is claimed. The prior focused Cargo test attempt was blocked by existing unrelated test API errors.

## Private full regression

The same candidate passed the complete physical service gate at exactly 776,341 steps, matching the prior build. Physical completion, occupation, new and existing routine cycles match; growth-jobs.ron and growth-residents.json are byte-identical. The deferred probe skips 593 project and 64 worker rows in both eligible walking and retry cases, with equal saved/transient state and unchanged inventory. Active work prepares once, and completed queued work still indexes.

The old three-home growth gate and eight-worker voluntary-group gate also pass, including their conservation and reload checks. The group fixture has 104 paid cells, three circles and four accepted offers. Evidence: private-service-equivalence.json, private-service-v1, private-legacy3-v1 and private-groups-v1 under task/city-service-snapshot-opt-20261107.

## Actual shared result

The shared release-fast candidate passed the same 240,000-tick completed-settlement workload. The old binary ran once before and once after the candidate. Gameplay-call mean was 0.10580 ms versus 0.21187 and 0.18666 ms, a 43.32 to 50.06 percent reduction. Candidate p95/p99 were 0.1924/0.6316 ms versus old 0.3502/1.7539 and 0.3143/0.9508 ms. Runner wall time was 56.00 seconds versus 92.02 and 78.18 seconds. Baseline variability is retained rather than hidden. Both resident outputs are byte-identical across all three runs; all compared correctness fields match. No compiler was active during the serial timing runs.

The full shared service gate passes at the identical 949,801 steps: physical construction at 805,079 and occupancy at 805,109. All 597 paid cells, 44 housed of 64 moving residents, new/existing ordered routine cycles, obstruction recovery, production targets and conservation match. growth-jobs.ron and growth-residents.json are byte-identical to the prior release. All five full reloads passed. The completed input checkpoint's original file hashes were rechecked unchanged.

The old-growth gate passes at 417,211 steps with 264 paid cells, three homes and 12 new resident cycles. The separate group gate passes with eight productive actors, 104 paid cells, three circles, four accepted offers and exact full-world reload. Group behavior was not enabled in the 64-resident service run. A fresh normal-launch automated smoke capture was visually inspected. No new visual design was introduced.

The six-path patch applied once after exact preimage and idle-compiler checks. It preserved the shared onboarding hook, all unrelated edits and save header 68. A separate editor all-targets check ran during compilation but finished before measurement. Native compilation passed in 223.16 seconds with 428 existing warnings. Source hashes were unchanged through acceptance and promotion.

Source commit: c909c28760eb2ec3a4965bcd22dea7499c7e98fb. Candidate city-service-snapshot-shared-v1.exe and aliases city-growth-playable.exe / city-colony-playable.exe have SHA256 3E3D77850A902A97754D69C1D55158D461958CC1C0A37D41EAB112D7B5C6F648. Play-CityGrowth.ps1 is unchanged. Prior A1DBDE7F release aliases are retained under task/city-service-snapshot-opt-20261107/playable-backups.

Evidence index: task/Axiom-City-Snapshot-Optimization-Verified-20261107.json. Paired measurement files: shared-comparison-1.json and shared-comparison-2.json. Service state comparison: shared-service-equivalence.json. All are under task/city-service-snapshot-opt-20261107 unless stated otherwise.

## Reproduction

Compile from the shared checkout with cargo rustc --locked --offline --profile release-fast --target-dir D:\voxel-cloud-target -p minecraft_style_voxel_prototype --bin minecraft_style_voxel_prototype -- -C opt-level=1 -C codegen-units=32 -C link-arg=/STACK:8388608 -o a unique task executable. Set CARGO_INCREMENTAL=0 and CARGO_BUILD_JOBS=4. The evidence package's run_shared_acceptance.ps1 shows the exact serial check arguments; use new output names when repeating it. It intentionally refuses to overwrite previous evidence or benchmark while a compiler is active.

This is a bounded reduction in completed-settlement simulation cost. It is not an FPS result, proof of a general active-construction speedup, increased population, unrestricted city expansion or a full workspace test/Clippy pass. Existing unrelated test API failures and the older preview-helper settling limitation remain outside this change.
