# Bounded single-courier load

## Scope

The service courier now carries at most 16 units per pickup instead of 8. It remains one physical worker with one existing Supply cargo record. Pickup still takes the minimum of outstanding demand, load bound, available source stock and destination space. Existing inventory operations retain ownership, arrival checks and partial-delivery cargo. There is no new saved field, policy flag, resource grant, movement bonus, actor or save version.

The earlier two-courier prototype remains private. Old readers reject two Supply records and can ignore the farming sidecar. An opt-in flag or main-header bump alone cannot protect shared sidecars through older-manifest fallback. Isolating every manifest, sidecar and realm identity would require a separate save-boundary change. This increment instead keeps the existing one-courier contract.

## Measured native result

Matched format-68 service scenarios use the same retained disposable nine-home checkpoint and 64 real residents. The 8-unit baseline finished physical construction at 684,959 fixed steps; the 16-unit candidate at 640,229, 6.5% fewer. Both paid for the same 597 cells, retained exactly one courier, housed 44 residents and passed obstruction recovery, resource conservation and five full reloads. Full routine acceptance ended at 776,341 versus 708,061 steps.

Gameplay-call means were 0.0843 ms and 0.0887 ms, with p99 0.5629 ms and 0.5866 ms. The run mixes differ and this is not evidence of a CPU-per-call or frame-rate improvement. Construction throughput improves in this fixture; navigation, worker routines and geometry still limit progress. The load bound is an existing abstract item-count capacity, not a simulated lifting-weight model.

## Compatibility and authority checks

An immutable pre-change LOAD8 runtime loaded the candidate's real 16-unit cargo checkpoint. Full jobs, worker identities and exact position bits, primary and hub stock, cargo source/target and farming sidecar contents matched. The writer and reader retain format68 and JOB1. This proves the tested current-format downgrade path, not arbitrary historical versions or crash atomicity.

A first test-report comparison incorrectly round-tripped f32 positions through JSON f64 numbers and failed by one f64 ULP. The corrected harness compares exact u32 position bits; no simulation tolerance was introduced. Original failed evidence remains available.

Actual transport::execute tests use labeled synthetic stock/order bounds over a copied paid hub: demand/source/room 3/5/7 transfers3, 13/5/7 transfers5, and 13/15/7 transfers7. They retain nonarrival and repeated-call no-debit assertions. Separate engine storage tests withdraw16, deliver3, retain13 on a full destination, reload that cargo and move it once to a death drop. These are isolated fixtures, not sources added to a gameplay world.

The final candidate passes 148 selected city-job, storage and multiplayer tests. Group/full-world reload and 240,000 further daily-life steps pass; all44 housed residents eat, rest, socialize and resume work with64 live actors and conserved food/timber. Native service, growth and exact-bit compatibility records are in the task evidence. The ignored throughput tests require explicit disposable roots and copied checkpoints. Native timing runs do not change user saves or existing playable executables.

## Limits

Format68's pre-existing sidecar publication and cross-file interruption limits are not repaired here. This change does not silently migrate a world, provide two-carrier saves, resolve the separate format69 policy decisions, or certify the whole application. No original save or promoted alias is overwritten by the experiment.

Evidence: D:\codex-scratch\city-courier-load-20261107. Final integration and executable status are recorded in its REPORT.md.

## Aggregate gate

`just verify` remains blocked at all-target checking by five unrelated worldgen test compilation errors: planet configuration fixture, two removed configuration constructors, missing lobes helper and relief target module visibility. Clippy, full workspace tests and benchmark gates are not reported as passed.
