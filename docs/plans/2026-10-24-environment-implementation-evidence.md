# Environmental implementation evidence

This records an implemented portion of the approved biome/UI plan, not acceptance
of the entire overhaul. See ADR 0019 for ownership contracts and remaining work.

## Local checks

- `engine_sim` environmental tests: exact infiltration/storage at 1 m, 0.5 m and
  0.25 m; conservative drainage; proportional sediment transport; nutrient and
  dry-mass accounting through growth and decomposition; gravity retention
  while retained water remains available for plant uptake.
- `engine_content` environmental RON parse, validation and round trip.
- `engine_worldgen` drainage tests: acyclic shared downhill links at negative
  coordinates, agreement across resolutions, historical version bypass and
  water in ordinary generated channel sections.
- App environmental tests: topology, exact aggregate refinement, physical
  root access, excavation/release, blocked pairs, source/receiver admission,
  inventory-backed sediment recovery, compost, sealed/open construction
  displacement, persistent clocks/credits/stocks, exact fluid rendering,
  full-queue retirement and cached stacked-surface shading.
- Save tests include a populated save-31 footer and production loading of
  save 30 without environmental state; prior inventory/save fixtures retained.
- Crafting tests include atomic staging rejection and matching preview/commit
  admission. The latest combined app/content/simulation/worldgen check passed
  92 selected tests with 4 ignored (`environment water save crafting`).
- Slow soil tests verify equal integrated rate-limited flux across one/eight
  second intervals at every scale, prompt event advancement after edits, and
  no catch-up for the time a patch spent dormant.
- At the fractional-state limit, integral water needs no new fraction record
  and remains admissible. Saturated event queues retain steam at its source;
  both gas owners' event requirements are admitted before the transfer.
- Sleeping-world check services at most 100 contact probes over 12,000 fixed
  steps, creates no soil records and leaves no environmental time backlog.
  It is an overhead check, not a growing-disturbance benchmark.
- Release-fast application build and shader validation succeeded.

Intermediate isolated measurements (not integrated acceptance):

- Fixed/growing contact checks covered 1, 32 and 256 contacts for 60 seconds.
  The 256-contact run executed 15,360 soil steps in 26.873 ms and reserved
  449,536 bytes. These direct calls omit streaming/light preparation.
- An earlier release water benchmark verified 50,000 accepted paired transfers;
  setter-pair and transactional-pair timings were 524/526 ns, with no speedup.
  A dam-break check conserved mass through 141,760 steps and 94,406 updates.
  Activation/cache changes followed these measurements; they are intermediate
  correctness evidence, not a current integrated performance claim.

## Native observations

Actual application captures, NVIDIA RTX 4090 / Vulkan, 1280 × 720:

- `target/visual-direction/environment-ui-before-20261024/`: original body font.
- `target/visual-direction/environment-ui-after-20261024/`: readable HUD text,
  preserved icon detail and warm focus accents. Captures were inspected.
- `target/visual-direction/environment-inventory-20261024/`: workbench panel
  with readable labels and scrolling content. This fixture opened Workbench;
  it is not evidence of every inventory gesture or accessibility requirement.
- `target/visual-direction/environment-terrain-20261024/`: native generated
  terrain/water rendering. The selected camera faced coastal water; it does
  not establish river morphology or diverse-biome acceptance.
- `environment-channel-physics-20261024-01`: physics-enabled run found the
  queue saturation deadlock (65,536 pending events, tick never advanced).
  Read-only dependencies incorrectly required new event slots. Bulk-water
  activation and insertion of new events into overdue steps amplified it.
- `environment-channel-physics-20261024-02` through `-05`: retained failed
  throughput evidence. Fixes removed the deadlock, cached authoritative sky
  geometry and added retained soil water. Run 05 still ended at tick 22 versus
  target 36 (14 seconds behind), despite a 15.163 ms renderer p95 at 1280x720.
  Renderer frame rate does not turn that into a simulation acceptance pass.
- Run `-06` removed discarded drainage/climate calculations and increased the
  service's work-count ceiling within its unchanged time budget, but still
  ended 10 seconds behind. Slow soil integration/scheduling was added after
  this run; its mass and timing contracts have focused tests.
- Run `-07` used slow process scheduling and completed a 40-second measured
  capture (plus warmup). Environmental lag peaked at three seconds during
  startup and finished at tick 46 / target 47, with 26,681 patches, 24,698
  pending owners and 19,033,120 reserved environmental bytes. Pending owners
  include future slow events; they are not all overdue. Process peak working
  set was 1,328,762,880 bytes and peak private bytes 3,080,388,608.
  Renderer p95 was **19.581 ms at 1280x720**, so the 16.67 ms frame-time gate
  still fails. This is progress in simulated-time throughput, not full
  performance acceptance. The native image also exposed a low-level placement
  notification emitted by automatic physics; that message was moved to explicit
  player placement failures after this run.
- Run `-08` used the same route at **2560x1440** for 40 measured seconds:
  p95 **15.042 ms**, p99 18.302 ms; final environment tick 46 / target 47,
  23,774 patches, 22,022 pending owners and 17,276,736 reserved environmental
  bytes. Peak working/private bytes were 1,395,851,264 / 3,070,128,128.
  The placement-warning spam was absent in the inspected native capture.
  This single route meets the 16.67 ms p95 threshold; it does not establish
  every desktop scenario, long-run bounded growth or laptop performance.
  Automatic cloud scaling chose divisor 3 here versus 2 in run 07, so the
  resolutions are not a matched renderer comparison. Fraction-capacity and
  gas-admission hardening followed this capture and passed the local checks.

Each run records executable hash, working-tree hash, arguments, process memory,
frame samples and renderer logs. The checkout contains concurrent work; these
runs are not a controlled attribution of frame-time changes to this patch.
They do not establish the 1440p desktop target or low-end hardware performance.

## Outstanding acceptance work

Free water now retains exact quantities; its transport choices still use the
legacy coarse compatibility view. Complete coupled-step scheduling before
claiming deterministic replay. The native growing-disturbance workload must
meet simulated-time throughput independently of rendering. Also
validate growing disturbances, unloading/reload during propagation, native
maintenance and cultivation, ordinary-region diversity and all supported
scales. Individual trees, thermal phase physics, environmental visual cues,
rebinding, safe building undo and the remaining full UI/art pass are unfinished.
