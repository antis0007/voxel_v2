# Exact-output generation optimizations, 2026-11-10

Current generator remains287. No save format, physical cadence or default generator change.

## Accepted changes

- `drift/weather_samples.rs` caches immutable spherical stencil geometry per solve, retaining fresh evolving heights and identical sample reduction order. Approximately10.34MiB temporary memory. Tested5.7–6.9% lower generation time.
- `drift/history.rs` borrows large read-only geological columns instead of copying them. Transport/add bodies and floating order unchanged. Tested another10.9–12.2% reduction.
- `drift/episodes.rs` omits displacement scans only when no future reroute can observe them, or when the Boolean flag is already true. Untouched weathering columns reuse the soil query fluvial just computed at the same date; height arithmetic remains unchanged. Tested another2.7–4.3% reduction.

These percentages are sequential experiments, not additive FPS or total-loading claims. Fixed seeds74/731, Windows release-fast16 codegen units/no LTO; shipped-release gains are unmeasured.

Final combined surface comparison: seed74 mean85.29→81.59s, seed73191.72→89.25s. Old283 and286 full-world and published-history hashes also matched. Nine final targeted tests passed, including legacy283 pinned history,285 conservation/cadence/seams/quarter-metre repeatability, exact sampling, material queries and guaranteed untouched weathering cells.

## Remaining measured costs

Before the surface work: fluvial11.97s, weathering7.76s, height reconstruction9.31s, history scatter9.57s, initial/final routing4.63s combined. After preliminary combined work: fluvial10.22s/weathering7.04s. No intermediate reroutes occurred in the4-step287 history; its720 epochs and2880 surface steps remain real.

Next candidates must preserve live crust/height: fixed terrain-noise inputs, geometry/discharge terms within an unchanged route, history transport layout. Plate-edge caching is NOT rejected or accepted: its earlier test comparison was invalid because the test EXEs were identical.

## Mandatory experiment gates

Copied source can preserve old timestamps and fool Cargo freshness. A successful build alone is insufficient. Force changed private source timestamps current, assert the relevant compiler artifact has `fresh:false`, archive/bind source and executable hashes, and require distinct candidate/baseline EXEs for code-change comparisons. Weather cache used an explicit runtime bypass in one freshly built test binary; that is a deliberate exception, not a stale-build experiment.

Compare complete published history fields/events/budgets plus final world checksums. These do not serialize every latent state. Keep immutable evidence, include failed/invalid experiments, and use old-generator checks. Never trade fewer simulation steps for claimed exact-output speed.

## Delivery and evidence

Evidence/report: `D:\CodexArtifacts\axiom-generation-speed-20261110\REPORT.md`.
Latest playable: `D:\CodexArtifacts\axiom-generation-speed-20261110\native-build-v2\bin\axiom-generation-optimized-v2.exe`.
Use repository working directory. Fresh native compile and isolated headless smoke passed. Existing launchers and user saves were not changed; launchers do not automatically select this artifact. V1 remains archived.

No native visual/orbit review, FPS comparison, multiplayer WAN check or full workspace verification is claimed. Focused Clippy was blocked by existing core/fast_hash.rs:35 lint. Do not weaken that gate to present a full pass.


## Plate-edge reuse follow-up

Resolved histories now compute post-advance plate edges once per epoch and reuse them for crust updates, deep-time advance, resolved height reconstruction and episode source construction. Cache expires before post-relax preview queries; unresolved versions retain direct queries. Approximate temporary storage: 2.58 MiB. No generator/default/save format changes.

Fresh baseline and candidate compiler artifacts both report fresh=false and their EXE hashes differ. Repeated pre-disconnect seed74 generation: baseline81.208866/81.881059 s, candidate79.778543/79.202871 s; means81.544963→79.490707 s,2.52% reduction. These are CPU generation timings, not frame-rate measurements.

Computer disconnected during seed731 comparison;142.471010 s candidate timing is retained but excluded from performance conclusions. After reconnect throughput changed substantially: seed731160.986939→151.279498 s, legacy28330.419801→30.577652 s, legacy28634.244424→33.456435 s. Do not pool across connection epochs or claim a universal6% improvement. Legacy283 is neutral within run noise.

All completed current287 seeds74/731 and legacy283/286 pairs retained identical final terrain and complete published frame/event/budget hashes. Five focused unit tests and four history-clock tests passed; integration-test worldgen and test artifacts freshly rebuilt. Independent code review accepted the mutation window and confirmed all earlier surface guards/tests retained. This is not an exhaustive proof for every latent state or seed.

Evidence: phase-fresh-base/, phase-fresh-candidate/, phase-fresh-runs/, phase-recovery-runs/, build-phase-history.*, phase-*-tests logs and phase-integration-receipt.json. New native build is separate; existing launchers are not changed.


## Fixed province-noise reuse (V4)

Five seed/direction-only scalar terms are cached once per resolved solve and reused for resolved-height reconstruction. Evolving crust, climate, land, stresses, faults and folding are not cached. Approximately1.62MiB transient cache, released after solve. Existing uncached arbitrary-coordinate runtime and preview callers retain original equations. No generator/default/save change.

Fresh distinct baseline/candidate binaries; two new independent tests repeat original equations over every mesh node at sides4/17/84 across four seeds and compare complete Province bits with frozen equations across seven versions and plate mutations. Five earlier focused units and four freshly rebuilt history_clock tests also passed (11 total). Ten full generation runs preserved all final terrain/published-frame/event/budget hashes.

Controlled ABBA seed74: baseline136.735180/137.792914s, candidate129.230860/132.080093s. Means137.264047→130.655477s,4.81% reduction. Seed731149.346162→141.754370s,5.08%. Legacy28330.679286→30.926550s and28633.116752→33.294445s were neutral/slightly slower single pairs; do not claim an old-version improvement. Absolute times reflect slower machine throughput after reconnect, not a software regression versus earlier81s runs. These percentages are incremental and must not be added to prior percentages.

Source-only proposal authored separately, reviewed against original math before compile; production integration guarded against changed preimages. Records: province-base/, province-candidate/, province-runs/, build-province-history.*, province-integration-receipt.json. No full native FPS, WAN multiplayer or exhaustive latent-world-state verification claimed. Standalone V4 retains ordinary default multiplayer features; existing launchers remain unchanged.


## Weathering resistance query reuse (V5)

Weathering now supplies the exact same-date mobile soil amount freshly produced by fluvial to resistance_with_mobile, avoiding one redundant historical column scan. Zero additional cache storage. The default resistance method preserves its own fresh query; fluvial and ice callers remain unchanged. Height arithmetic, geological material thresholds and erosion formulas are unchanged.

Fresh distinct baseline/candidate binaries. Independent frozen resistance formula test covers every material branch, 240/720Myr history, mobile/material age boundaries and15/150Myr weathering thresholds. Together with prior query/noise tests and freshly rebuilt history_clock,12 tests passed. Ten current/legacy whole-world runs matched final terrain and complete published frame/event/budget hashes.

Seed74 ABBA baseline130.908341/130.326822s, candidate128.295484/128.045269s: mean130.617582→128.170377s (1.87%). Seed731145.400506→142.862813s (1.75%). Legacy28330.078823→29.665983s;28632.986802→32.504218s. Incremental CPU generation timings, not FPS or a universal performance guarantee.

Evidence: resistance-candidate/, resistance-runs/, build-resistance-history.*, resistance-integration-receipt.json; native-build-v5 contains separate playable delivery. Existing launchers and user saves remain unchanged.

### V4 no-window native probe

Verified V4 executable completed --horizon-probe-only on seed74/gen287 in130.97s, expected checksum83600c882b372c6a.63 bounded chart-origin query candidates plus visibility/coast diagnostics were produced. This region is ocean: no ridge/shore/forest targets, so canonical-sampler/source-proxy tables are empty. Recorded in native-v4-probe/summary.json. This validates native generation/query execution only, not visual quality, renderer performance, interactive play, or land-target coverage. No window, desktop input or player save was used.


## Ordered bounded parallel history maps (V6)

Fresh V5 profile (seed74, schema3) measured127.526s total/history115.053s. Top costs: scatter14.909s, crust8.911s, height6.337s, weather8.489s, surface56.148s; surface children fluvial15.827s/weathering12.487s/events6.372s/ice5.790s. This profile guided the change, not stale earlier timings.

Resolved height and weather output maps now use the existing ordered map_vertices helper. Worker budget is computed once per solve, reserves one logical core and caps at4;42,338-node history mesh uses3 ordered batches. Climate/sea reductions, shared accumulations, erosion and ice/deep writes remain serial. All workers join before state changes. No new dependencies, unsafe code, schema/default/save changes. This improves latency using spare CPU capacity; total CPU/energy and concurrent gameplay interference were not measured.

Fresh distinct binary screen seed74: baseline130.154939/130.477222s; candidate120.604751/121.071062s. Means130.316081→120.837907s,7.27% faster. Seed731144.100078→133.720454s,7.20%. Legacy28329.750703→28.420780s;28632.465530→32.091016s (single pairs). All10 runs preserved final terrain and complete published frame/event/budget hashes.

Fifteen focused tests passed: existing8 exactness units,3 new parallel-map tests,4 history_clock tests. New tests force and assert3 distinct ordered worker batches at side84, compare production height/weather against frozen serial equations, and compare resulting ice/deep climate state. Fresh test artifacts archived separately.

Evidence: v5-profile-binary/, v5-profile-seed74.log, parallel-candidate/, parallel-screen/, parallel-regression-runs/, parallel-test-binary/, parallel-integration-receipt.json. Clean source: parallel-delivery-source. Separate playable: native-build-v6. Existing launchers not changed. No interactive/FPS/GPU/WAN validation claimed.


## Stable destination gather experiment: not integrated

A stable destination CSR gather retained every source/slot contribution, including zero/duplicate entries, and joined all bounded workers before publishing history/crust. Two direct tests compared every intermediate Transfer and latent history-record/bed bit with frozen scatter, including history=None, old inputs unchanged, scratch reuse and actual worker observation. Fourteen fresh-process runs across287(seeds74/731),283,286,278,280 retained exact world and published-history/event/budget hashes. Fresh distinct binaries and CPU-time receipts preserved.

Repeated seed74 mean122.942236→117.476849s (4.45% latency gain), but mean process CPU132.5625→137.875s (+4.01%). Seed731133.918687→129.021099s (3.66%). Some older versions were slightly slower. It costs6.14MiB reusable scratch and adds significant transfer complexity. This is not a computational saving; it exchanges more CPU for a modest latency gain. Without gameplay contention validation, it was not adopted as the default. Shared production and latest playable remain V6. Artifacts: gather-candidate/, gather-runs/, gather-evaluation.json, gather-bench-source; proposal is private/unintegrated. Initial zero-test filter was correctly rejected; corrected filter ran2tests before benchmarking.


## Immutable climate basis (V7)

Cache three raw seed/axis/direction terms per coupled solve: annual base, winter anomaly, and pre-clamp wet sum. Live land/interior subtraction, winter multiplication order, wet clamp, custom recipes, rain shadows, EBM/sea reductions and ice/deep writes are unchanged. Public planet_climate::at remains unchanged as an independent comparator and for runtime sampling. About 0.97 MiB solve-local storage, no added concurrency.

Fresh distinct binaries; thirteen focused units and four freshly built history_clock tests passed. Fourteen fresh-process benchmark runs across current287(seeds74/731) and legacy278/280/283/286 preserve final-world and complete published-history/frame/event/budget hashes. This does not prove every internal state for every seed; direct weather tests also compare ice/deep results.

Repeated seed74 baseline123.696003/121.067657s versus117.394062/116.331994s: means122.381830→116.863028s,4.51% lower elapsed time. Process CPU mean130.039063→121.171875s,6.82% less. Seed731135.717895→132.723367s (2.21%), CPU144.875→138.234375s (4.58% less). All four legacy pairs were slightly faster, but are single-pair observations. Unlike the rejected gather, this reduces total work, not only latency. Results are machine/session-specific and incremental onV6, not a guaranteed FPS gain.

Three guarded source files integrated only after compatibility tests. Clean delivery source: climate-delivery-source; evidence: climate-candidate/, climate-runs/, climate-evaluation.json, climate-integration-receipt.json. Existing launchers, generator defaults and saves remain unchanged. Separate native V7 build validation is recorded under native-build-v7. No interactive visual/FPS/GPU/WAN test performed. Workspace-wide gate/Clippy not claimed; existing unrelated baseline issues remain.


## V7 reprofile and rejected history-prefix experiment

Fresh V7 seed74 diagnostic total121.931778s/history109.154986s, exact expected world/history hashes. Main costs: scatter16.412477s, crust8.901743s, prepare7.732490s, surface58.909450s (fluvial16.605249s, weathering13.240001s, events7.037452s, ice6.028878s). Height2.653300s and weather1.313495s are no longer primary bottlenecks. Evidence v7-profile-binary/, v7-profile-seed74.log; instrumentation is private, not production.

TransferPrefixes scanned actual stored nonzero bits and skipped only canonical +0 tails, preserving all future bins, signed/nonfinite values, source-slot order and original finish. Two direct frozen-reference tests compared every latent record/bed bit after each transfer and finish; thirteen existing units passed. Fourteen world/published-history comparisons across287 seeds74/731 and278/280/283/286 remained exact. Fresh binary evidence: prefix-candidate/, prefix-test-binary/, prefix-runs/.

Rejected: repeated seed74 baseline117.4129975s versus117.7688455s (0.30% slower); CPU119.4453125 versus120.9140625s (1.23% more). Seed731 latency improved0.68%, insufficient for extra complexity. Do not confuse fewer theoretical arithmetic operations with lower actual execution cost. Prefix scan/metadata/dynamic-loop codegen overhead are possible causes, not individually measured. Production, saves, defaults, launchers and latest playable remain V7. No further native rebuild was performed. Decision and full receipts: prefix-evaluation.json.


## V8: persistent complete geology reuse (2026-11-10)

Integrated 31 guarded source/manifest files, with backups and source hashes in `cache-integration-receipt.json`. No generator/default/save-format/launcher change. Clean executable: `native-build-v8/bin/axiom-generation-optimized-v8.exe`; SHA256 `5006f401642330ba488a52d763678c7f845cd4ff4bcdaf05279a6e6810b4f4dd`. Existing user launchers do not automatically select this artifact.

Actual native no-window seed-74 probe: cold 119.843 s / 125.859 CPU s; warm 11.546 s / 15.672 CPU s; repeat 11.551 s / 15.828 CPU s. All six generated diagnostic files match bit-for-bit. Peak working-set observations: 439435264 / 403931136 / 400023552 bytes. Evidence: `native-v8-cache-probe/`. These measurements include the CPU probe, not interactive loading, rendering or FPS. First unseen seed/recipe/build remains a cold solve.

Cross-process worldgen seed-74 cold/warm world/hash: `83600c882b372c6a` / `2799ade7b3a20272`; seed-731: `a2cc4103ba238ffc` / `693b873931a1bf13`. Seed-731 wall 136.096 s cold -> 13.685 s warm. Complete 241-frame histories preserved. Evidence: `cache-runs/`.

Validation: 28 focused tests plus four legacy `history_clock` tests; fresh app cold-save/warm-load test using actual admitted voxel edit and session APIs; all pass. Saved files unchanged by warm read. Native smoke: 180 ticks, hash `64d10e01681ceed9`, zero failures. Evidence: `cache-final-regressions/`, `cache-save-resident-runs/`, `native-build-v8/`.

Two earlier test fixtures failed before save: one placed Stone underwater, the next omitted required resident-chunk admission. Both failures were retained and the fixture corrected to dry procedural spawn space with actual residency and strict commit checks. No assertions were weakened, and no cache defect was inferred from those failures.

Known limits: generator 287 only; ~129 MiB per entry; two-entry/512 MiB disk cap; complete cache misses on executable changes; no semantic-invalid-entry repair; explicit directory separation is contractual. Bounded encoder growth reduces avoidable requests but does not bound process RAM. Full workspace Clippy remains outside this focused acceptance due the previously recorded unrelated fast_hash failure. No interactive visual review or multiplayer/FPS claim.


## V9: exact terrain stencil reuse (2026-11-10)
Integrated `World::ground_base` reuse of its existing immutable four-node stencil; no terrain equations, reconstruction rules, generator IDs, saves or cache schema changed. Original scalar/vector accumulation order is retained. Independent review and frozen Ground comparisons passed for275/277/278/279/283/286 and287seeds74/731, plus four fresh history_clock integration tests.

Three fresh-process production padded-chunk runs per variant:1m median34.236s to31.371s (8.37%lower); quarter-metre10.808s to9.368s (13.33%lower). Every padded block matched. These are sequential core generation timings, not FPS or full app streaming benchmarks. Six legacy test logs and independent binary bindings are archived under ground-stencil-legacy and chunk-stream-*.

Separate playable: native-build-v9/bin/axiom-generation-optimized-v9.exe, SHA256765ea23120e3a20702a5a8697a828bb6be181084661733f3009132b2c6419a7f. Fresh compiler artifacts/source manifest retained. Headless smoke180ticks, hash64d10e01681ceed9,0failures. Existing launchers were not changed; no interactive visual validation claimed.

First-use profiling uncovered a larger civilization/ruin planning fan-out:421fine-relief stages, largely triggered during block/structure evaluation rather than the chunk's own terrain footprint. Query-pruning candidate is still PRIVATE and unaccepted pending full six-chunk repeated, quarter and frozen-district tests. Do not treat the first underground chunk speedup alone as total saved work: later surface generation requests some deferred plans.


## V10 query pruning integrated
All current/quarter six-chunk exactness and performance gates plus four frozen original district/route cases passed. Shared-source baselines checked before any writes;4files backed up and atomically replaced. Generator/save/cache layouts and launchers unchanged. Clean pruning-delivery-source excludes timing/benchmark code. Native V10 build follows; do not claim playable completion before its receipt.


V10 playable build completed and fresh-bound to pruning-delivery-source. Executable native-build-v10/bin/axiom-generation-optimized-v10.exe SHA25663767de024fa21db2442f091a6a67294f5bae8ebcad74c4f28a51135c310640a. Headless smoke180ticks, expected hash64d10e01681ceed9,0failures. Six-chunk core generation compared with V9:1m36.548163s to28.950476s(20.788%lower);quarter29.912787s to9.472530s(68.333%lower). Three fresh measured processes each, exact235824-block output per fixture, including true river surface in1mfixture. Four frozen district/route cases2871m,287quarter,2831m,2411m passed; support/large-coordinate tests passed. Not FPS, interactive visual validation or universal load-time claims. Launchers and user saves unchanged. Compiler/source/binary/test receipts retained. Current build/timing lane released at completion.


## V11 fine-stage query reuse integrated
All nine frozen Stage cases, fixed-node poisoning and permutation coverage passed. Both voxel-scale chunk gates passed. Three source/test files integrated with baseline guards and backups; no new buffers or world/save/cache schema changes. Separate native build follows; not yet a playable-completion claim.


## V11 completed: one resistance query per fine-relief node/pass
The separate playable native-build-v11/bin/axiom-generation-optimized-v11.exe is fresh-bound to fine-delivery-source. SHA256: bc192417837fae2eb81ca9a1322982f1064ea77ee5f39a752c4f06581d375bf1. Headless smoke passed 180 ticks with zero failures and the expected hash 64d10e01681ceed9. Existing launchers and user saves remain unchanged.

Measured against V10 in three fresh-process six-chunk runs per variant:
- 1 m: median 28.809103 s to 26.186705 s, 9.1027% lower core generation time.
- Quarter-metre: median 9.605485 s to 8.847495 s, 7.8912% lower core generation time.
Every one of the 235,824 padded blocks matched in each fixture. These are sequential core-loading measurements, not FPS or universal time-to-play claims. There is no new cache, buffer, world schema, save schema or generator-ID change.

All nine frozen Stage cases passed: versions 275, 277, 278, 279, 280, 283, 286 and 287 on seed 74, plus 287 on seed 731. Tests compare every Stage field bitwise over the full 32/16/8/4 m parent chain, assert routing permutation coverage, exercise historical-column absence and prove fixed-node limits are not consumed. Production and test changes were integrated only after gates passed, with baseline checks and backups.

No owned build/test process remains. The next useful tooling improvement is a distinct headless worldgen workload in the existing Foundry ingestion pipeline, rather than spoofing its 20-resident/frame acceptance evidence. Detailed cold-builder timings should remain opt-in. No further road-pruning shortcut is justified without preserving off-query topology dependencies.


## Foundry headless workload receipts integrated

The existing Foundry CLI now accepts explicit headless-chunks and headless-worldgen workload kinds. It validates successful harness completion, seed/version/scale where logged, ordered chunk coordinates, coverage/counts, finite positive timings, expected checksums and raw evidence hashes. Full configuration/source/build/hardware identity remains explicitly runner-supplied provenance, not independent runtime attestation. Instrumentation profile plus declared/observed diagnostic kinds are comparison identity, preventing profiled/unprofiled comparisons. Nested builder durations are never added to parents; missing parent IDs remain unknown.

Reviewed v2 patch b745190445dd98d5d861d647f2d29733cfca47f6011919e7b7893c27c9bd402f integrated after unchanged-baseline guards and backups. Ten new headless tests and eleven existing native parser tests passed both before and after integration. Windows CRLF caused strict Git context failure; whitespace-aware applicability and byte-identical baseline guards passed. Receipts/backups: D:/CodexArtifacts/axiom-foundry-headless-20261110/integration-verified. This is CLI/report/agent-packet support, not a Forge web panel or runtime performance improvement. No engine binary, launcher, save or cache schema changed.

Fresh private V11 profiling: 489 relief stages, warm total25.105s, inputs13.307s, passes9.076s, nested regions1.738s. Complete six chunks26.360s, checksum686900d727e4a31d unchanged. Instrumented attribution, not acceptance timing. Fixed-node historical-input pruning is still screening; no acceptance/integration claim.


## Fixed-node historical inputs: neutral screen, not integrated

Fresh production-only candidate skipped history_strata interpolation only for immutable fixed fine-stage nodes. Three alternating fresh-process six-chunk runs per variant retained every235824block and checksum686900d727e4a31d. V11 median26.218931s vs candidate26.063000s, only0.5947%lower, with overlapping timings. This does not justify an optimization/acceptance claim. Candidate remains isolated; V11 production is unchanged. Prepared quarter/legacy/memo test drivers were deliberately not run after this neutral screen. No separate native rebuild. Benchmark session1775 finished with no owned build/test process.


## Relief inputs and river attempt attribution

A fresh private combined profiler completed cold and warm runs with the same235824block checksum686900d727e4a31d. Across489Stages it observed3081144input nodes and48138sampled nodes (~1/64), with stage-hashed sample phases. Actual fixed-node share11.05%, sampled fixed share10.93%. Measured whole inputs13.253s cold/13.164s warm, almost entirely the node loop; post-input only~0.05s. Sampled group ranking: ground, historical columns, volcano/projection, remaining inputs, exposure/noise. Scaled sampling overshoots whole-loop by~3-6%, so it is ranking evidence, not precise exclusive attribution.

All six attempted river tiles were accepted; three candidates were overlap-skipped. Initialization was22.8-29.2ms per accepted attempt, while solve spans were1031-1643ms. Network elapsed6.181s includes overlapping attempts; do not sum their wall times. Boundary preflight is not justified on this fixture.

Foundry now also parses these river/meander and sampled-input diagnostic types, distinguishing formula-derived operation counts from measured counts and sampled durations from whole-stage wall time. Sampling stride participates in comparison identity. Twenty-five focused Python tests passed after guarded integration; receipts under D:/CodexArtifacts/axiom-foundry-river-diagnostics-20261110/integration. Profilers remain private, never in production.


## V12 historical-column stencil reuse integrated
All seven frozen-column/memo/version cases and fixed-limit test passed. Both voxel-scale chunk gates passed. Four source/test files integrated with baseline guards and backups; arithmetic/order, cache and save schemas unchanged. Separate native build follows; not yet a playable completion claim.


## V12 completed: local historical-column stencil reuse

The separate native-build-v12/bin/axiom-generation-optimized-v12.exe is freshly built from history-stencil-delivery-source; executable SHA256 b9499ebcf02bb37c3742d1f00890572619b73ab0cf37e92008448cbb81a6e1ae. Both app and worldgen compiler receipts were fresh:false. Headless smoke passed 180 ticks with zero failures and expected hash64d10e01681ceed9. Build4m42s. Shared app sources were checked byte-for-byte against the baseline before delivery. Existing launchers and user saves remain unchanged; this is a separate artifact, not the selected launcher target. No interactive/FPS/visual acceptance is claimed. Its executable-bound geology cache is naturally cold on first use of the new binary.

Production change is one function: after the existing memo miss and successful Drift-column lookup, reuse the same immutable World-mesh interpolation indices/weights for each sampled field. The exact four-term multiply/sum order, empty-edifice handling, dynamic volcanic sampling, fit/trim/deposit, early returns and memo semantics are preserved. No new persistent buffer, schema or generator-ID change. Test-only owner helpers and frozen reference are included in the guarded four-file integration receipt.

Three fresh-process measured runs per variant, six padded chunks each, all235824blocks identical:
- 1m median27.263392s ->25.084823s, 7.9908% lower core generation time.
- Quarter-metre median9.120864s ->8.535124s, 6.4220% lower. This fixture's exact surface coverage is not independently confirmed.
Foundry raw-evidence comparison at axiom-foundry-history-stencil-comparison-20261110 reports 1m means26.959047s ->25.116459s, -6.8348%, run-bootstrap interval[-8.3245%,-4.8333%]. World-solve change is inconclusive; do not claim a solve/FPS/whole-startup gain. These are bounded sequential core-loading measurements.

All seven frozen-column compatibility cases passed:278/279/280/283/286/287 seed74 plus287 seed731. Tests compare every26bed x6scalar bit and duration plus all RockColumn scalars, forced trim/deposit/positive-lava, cold/hit memo, empty history/edifice and missing Drift. Existing fixed-limit test also passed. Independent source review accepted exact arithmetic and lifetime semantics.

No owned compiler, test or benchmark process remains. Regional persistent relief reuse is only a read-only feasibility audit, not an implementation or speed claim.


## V13 regional reuse completed

See `docs/reviews/2026-11-10-regional-relief-cache.md`. Six-chunk repeat-generation medians: 1 m: 24.921 s -> 1.234 s; quarter metre: 8.394 s -> 0.509 s. Cold population +7.3%/+20.0%, explicitly retained. Full exactness, corruption and actual saved block/water/inventory gates passed. New separate V13 built and smoked; V12, launchers and user saves preserved. No FPS claim.

## V14 completion: call-local surface sampling
Integrated exact same-coordinate sampling reuse after independent reviews and 12 current/legacy actual-world regressions. Default cold regional chunk construction median improved 26.873 to 23.540 seconds at 1m (12.4%) and 9.634 to 8.472 seconds at quarter scale (12.1%). These are regional generation timings with solved geology loaded, not entire initial planetary generation or FPS. Quarter whole-process cold CPU fell 22.578 to 21.328 CPU-seconds; warm CPU did not improve.
Fresh separate V14 executable passed headless smoke and the real two-process private-save block/water/inventory gate. Existing launchers, V13 and user saves are unchanged. Full report: docs/reviews/2026-11-10-call-local-surface-query.md. Raw measurements and validation are retained under node-sampler-*; Foundry packet is D:\CodexArtifacts\axiom-foundry-node-sampler-20261110. The retained-parent proposal was rejected and is not included.