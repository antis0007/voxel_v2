# Completed-home guard

The production change is one Boolean short-circuit reorder in `routine::home`. It checks whether construction is unfinished before evaluating `households_live`, avoiding a redundant bootstrap geometry scan in a completed settlement. Incomplete settlements still evaluate the same predicate. No cache, resource, actor-count, save-schema or physical eligibility change is introduced.

## Controlled private measurement

Nine native runs used the same completed 64-resident, 44-housed checkpoint and 240,000 fixed steps each, with three old, three candidate and three returning-old runs. All resident bytes, pre-rest checkpoint bytes and home-eligibility signatures match exactly. The four unrelated dirty source files were verified unchanged.

- Mean gameplay call: old 0.050450 ms, candidate 0.049102 ms, returning old 0.050844 ms.
- Candidate run range: 0.048895 to 0.049435 ms. Both old groups remained above 0.050388 ms.
- Warm actual home queries: about 2.01 microseconds before and 1.39 microseconds after, about 31% lower.
- Redundant guard alone: about 0.63 microseconds. The six bootstrap projects contain 763 cells plus 763 additional chest-height rows on a full pass.

The whole-call reduction is modest: 2.7% against the initial group and 3.4% against the returning control. It is not rendered FPS, individual-call tail latency, or evidence about unsupported larger populations. Query timing percentiles describe nine batch-average costs; the isolated original/reordered expression comparison is supporting evidence only.

## Diagnostic and validation scope

The optional native-only `VOXEL_CITY_HOME_PROFILE=1` report runs after the daily gameplay report has been written. It measures real read-only `home`, `home_complete` and `households_live` queries with bounded repetitions and consumed outputs. It preserves saved job state. Structural row counts are labelled and are not presented as observed runtime call totals.

Private app compilation and native gates passed. The unchanged default service scenario completed 597 paid cells at step 684,959 and the private two-courier scenario at step 610,799; both final jobs/resident files are byte-identical to their respective controls. Legacy three-home growth (264 paid cells), group construction (eight workers, 104 paid cells), full reload and conservation checks also pass.

Independent read-only review confirmed the guard is behavior-equivalent for valid states because the skipped predicate is pure metadata inspection. Physical home completion, entry and route clearance, housing overrides, following and guard checks remain unchanged. Daily negatives directly exercise obstruction/following with cargo; separate colony tests cover roof and guard cases. No new unloaded-home regression is claimed. The workspace-wide test/Clippy suite was not rerun; previous unrelated test-target API errors remain outside this patch.

Long-tail farming, pickup and actor-reactivation stalls remain unresolved. Candidate daily calls included isolated maxima up to 154 ms. Average improvements do not establish smooth frame times.

## Artifacts and separation

Evidence is in the parent task directory `city-home-guard-opt-20261107`: `measurement-summary.json`, `after-and-old-return.json`, `private-regressions-v1.json`, the source manifests, and each native run directory. Private baseline executable SHA256: `44BD023354423184E264787578DC967C5192666F6B5269314CC458A061857772`. Candidate SHA256: `551C77C08931517F887664B3B26592685BE520FD468C1DD762B7D77748136E49`.

This change is independent of private courier prototype commit c2010e6. Do not integrate that prototype with this patch: its old-reader farming-sidecar/fallback safety gap is unresolved. Only the home guard, optional native profile hook/module, and this document belong to this change. Shared integration and promotion require their own native verification against save header 68.
## Shared verification and promotion hold

The guarded shared patch contains only this one-line production reorder and native diagnostics/documentation. The native shared candidate is `city-home-guard-shared-v2.exe`, SHA256 `023E8C5107614B4EC0A5CE5DE139C36FD8B095219C6D8CF79EABAFC023EB5B68`. Shared save header 68 and the source storage hash are unchanged; the private two-courier policy is excluded.

Three shared old runs and three candidate runs each completed 240,000 daily steps. Mean gameplay-call cost was 0.053394 ms before and 0.051984 ms after (2.64% lower). All resident and pre-rest checkpoint bytes match. All 64 physical actors remained present; 44 housed residents completed the exercised meal/rest/social/productivity cycle. This is native call timing, not FPS.

The initial shared suite passed paid service expansion (597 cells; 805,079 steps to physical completion and 949,801 total acceptance steps), legacy growth (264 cells; 417,211 steps), group work (104 cells, eight productive workers), full reload/conservation checks and automated fresh-save startup. Service jobs and resident output exactly match the previous shared control. The startup capture was visually inspected; it is not a completed-city quality assessment.

**Promotion remains blocked.** An extra candidate service repeat exceeded the existing 30-second full-checkpoint wait at the courier-cargo stage. One bounded fresh-save retry also timed out, at the paid-hub stage. The timeout was not changed. No state mismatch or corruption was reported, but the failed runs did not complete their reload verification. A returning old executable passed the same service scenario but had a 2,398.09 ms gameplay-call maximum; the first candidate service run had a 505.99 ms maximum and one 6,218.20 ms reload. These wall-time spikes and save-wait failures have not been causally isolated. Do not treat the average improvement as a latency or reliability fix.

Both `city-growth-playable.exe` and `city-colony-playable.exe` remain on previous SHA256 `3E3D77850A902A97754D69C1D55158D461958CC1C0A37D41EAB112D7B5C6F648`. The shared source optimization is integrated; the candidate executable is not promoted. `latency-decision.json` explicitly disables the prepared promotion script. Failed evidence is retained under `shared-service-repeat-v2` and `shared-service-retry-after-timeout-v2`.

Shared evidence: `shared-checks-v2.json`, `shared-performance.json`, `shared-service-latency-repeat.json`, `latency-decision.json`, `shared-integration.json` and the run folders in `city-home-guard-opt-20261107`. A stale cross-checkout workspace dependency cache initially caused 66 missing-API compile errors. Only 18 generated workspace fingerprint directories were moved to a recoverable backup; rebuilding actual shared dependencies then passed in 148.38 seconds. No engine source repair or cache deletion was used. The D: cache now belongs to the shared checkout and must not be blindly reused for the private checkout.

Next priorities are to isolate checkpoint publication and farming/pickup/reactivation stalls, then establish an older-reader-safe isolated save namespace before promoting concurrent couriers. Further tiny decision micro-optimizations should not take priority over these failures. Workspace-wide tests and Clippy were not rerun in this slice; the native suites are not a whole-workspace pass.