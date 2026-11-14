# City frame pacing: output waits and capture probes

Date: 2026-11-07. Shared source integrated and native verified. Existing playable aliases are not replaced.

## Changes

Only four runtime/source paths changed in the shared checkout:
- City launch modes (--city-quarter and --city-livelihood) default to quiet optional per-second/hitch profiles.
- VOXEL_BENCHMARK_PROFILE_LOG=1 enables those profiles; 0 disables them. Other review modes retain their old verbose default.
- --benchmark-profile-physics explicitly enables both the existing physics mode and profiles, overriding the environment setting. --benchmark-physics still enables exactly the same physics.
- Screenshot existence checks stop after a shot is already requested. Existing-file protection and capture-request behavior are unchanged.

Full timing CSVs, final summaries, graphics, simulation, resources, actor limits and save formats are unchanged. The four existing review launchers that wait for stream-profile readiness lines and the natural-river observer keep their normal defaults. An explicit inherited 0 deliberately suppresses those lines.

Private timing probes were not integrated. They collect bounded in-memory spans, with no per-span console/file output.

## Measured cause

The original 4.838-second frame was mostly outside its 6.9 ms redraw. A subsequent matched OLD-binary run reproduced a 4.068-second frame: redraw 9.237 ms, outside redraw 4058.784 ms, exact delayed GPU sample 13.066 ms. It had no surface retries and only 0.035 ms request-to-redraw dispatch.

The private probe then measured:
- A redirected run spending 12.031 seconds across its warmup/measurement profile block, with only 15.625 ms current-thread CPU.
- One GPU-memory print taking 958.354 ms; collecting those counters took at most 0.0021 ms.
- Screenshot path.exists taking as much as 354.826 ms, despite a typical direct-output total of 223.523 ms across 4332 redundant queries.
- A quiet-logging-only intermediate run still had a 70.131 ms frame, with a 63.438 ms capture-existence query.

These establish synchronous output/filesystem waits in the city benchmark driver. They do not establish that all player-visible stalls have one cause.

The same final private executable, same scene/cache and redirected C-drive output:
- Explicit verbose profiles: p99 5.361 ms, maximum 1484.135 ms; 10.630 seconds aggregate profile waiting.
- City default, two runs: p99 4.489/4.480 ms, maximum 21.117/22.591 ms.
- Direct D-drive output: p99 4.468 ms, maximum 5.689 ms.
- Exactly three screenshot existence queries in every final run, instead of roughly 4300. The explicit profile alias also restored the expected logs.

This is a removed diagnostic overhead, not a claim of a faster resident algorithm.

## Actual shared executable

Evidence root: D:\codex-scratch\city-frame-pacing-20261107

Executable: city-frame-playable.exe
SHA256: 2b946cf960e6f7d96acbfd996813387c823795e87185d8d6df2602ceccd2a222

1280x720, RTX 4090/Vulkan, four view chunks, fixed camera/weather, copied identical 66,360,698-byte pipeline cache, eight-second warmup, twelve-second measurement, fresh outputs:
- Fresh 64 residents, direct D: p99 4.531 ms, maximum 6.048 ms.
- Fresh 64 residents, PowerShell-redirection/C: p99 4.514 ms, maximum 19.674 ms.
- Developed copied town, 64 residents/44 housed, live physics, direct D: p99 4.639 ms, maximum 8.590 ms.

The developed old/current baseline before this change was already clean: p99 4.660/4.626 ms. The visible city was inspected before and after; this does not improve general FPS by removing geometry. These small, low-view-distance scenes are not a large-city or low-end-hardware benchmark.

Initial matrix and trace joins: matched-runs.json.
Private probes and final frame results: probe-and-final-runs.json.
Every run retains its arguments, executable hash, system counters, frame CSVs, logs and captures. The copied pipeline cache is identical; OS/driver caches are not reset. No compiler overlap was recorded. The old reproduced stall had over 34 GiB available memory. Aggregate disk counters cannot exclude filesystem-filter or metadata latency.

## Native correctness gates

Actual shared executable:
- Service expansion passed 597/597 paid cells, 64 physically moving/live residents, 44 housed, maximum one courier.
- Physical finish remains exactly 640229 ticks, matching the preceding verified binary.
- Food/timber conservation, obstruction recovery and all five full reloads passed, including courier/building cargo.
- Ordered resident meal/rest/social/work cycles match the preceding run. Final saved-resident JSON and growth-resident JSON are byte-identical.
- Group gate passed 8 productive actors, 104 paid blocks, 3 circles and 4 acceptances; cargo/exit conservation and full-world reload passed.

Focused exact-source tests:
- Five profile-option precedence/default tests.
- Four capture-gate behavior/query-count tests.
- Three bounded private-probe aggregation tests.

The full just verify gate was attempted and is still blocked by five pre-existing worldgen test compile errors (planet config field, two removed WorldSampleConfig::new calls, missing lobes, and the relief_gradients planetary import). It stopped before a complete workspace test pass. Those unrelated files were not changed.

## Preservation and integration

All four shared preimages were rechecked immediately before atomic file replacement. Original bytes and a per-file journal are retained in shared-integration-backup and shared-integration.json. No private probe code or restricted format69 code entered shared source.

Seventeen protected baseline files, including original saves and playable aliases, remained byte-identical. The 21-file generated service input and copied developed source root also remained unchanged. Runs used new disposable roots, never an original user save as a destination. This is not a claim to have audited every historical save.

The save format remains 68/JOB1 with its existing limits. Passing quiescent reload gates is not a new crash-atomicity guarantee.

## Remaining limits and next work

1. Startup and final-report console writes remain synchronous. One original run stopped inside report output, so it was not proof of a teardown hang.
2. The first sample admitted when warmup ends can include a pre-measurement stall. One 14.240-second sample had this condition. Full route traces retain it; it must not be presented as steady-state resident cost.
3. Synchronous gameplay Trace::mark output can contaminate the next stage because its timestamp is sampled before printing. Final attribution runs disable that trace and dense frame-stage logging.
4. Current real capacity is 64 workers.128/256 input currently falls back to 32; the aggregate 256 counter is not 256 active people. A 128 experiment must test actual food, housing, jobs, movement and conservation. Existing old readers ignore invalid oversized travel/farming sidecars, so ordinary saves must not be used for that experiment.
5. Existing whole service-tick cost is about 0.089 ms mean/0.606ms p99; planning-tagged ticks include other gameplay work. Dedicated per-decision timing is still needed before claiming a resident-computation bottleneck. Warm home checks around 1.1 µs do not explain second-long stalls.

Reproduction uses the saved run-probe.py, run-developed-final.py and run-city-native.py scripts in the evidence root. Every output label must be new. Native service/group scripts use generated inputs and separate save roots.

