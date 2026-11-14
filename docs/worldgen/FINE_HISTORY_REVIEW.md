# Fine geological history preview 285

Generator 285 is an explicit review candidate. Planet's default remains 283;
existing generator gates and save formats remain unchanged.

## Scope

The existing 282/283 solver already used real 250 kyr surface updates and
analytic reservoir-threshold dates for eruptions and earthquakes. It transported
plates every 5 Myr and published only those endpoints. This change makes plate
transport, boundary reclassification, climate and published history advance every
1 Myr. Surface erosion still runs every 250 kyr: 960 updates across 240 Myr,
not five times as much work. Quake and magma events retain their exact model dates.
This is a multirate game model, not calibrated earthquake or eruption prediction.

Deformation rates, coastal exchange and plate acceleration are scaled with elapsed
time. Plate splitting remains on its original 10 Myr cadence. Hotspot tracks use
the recorded finer Euler stages. Erosion retains transported sediment, cover,
rock resistance and weathering rather than cutting each snapshot independently.

Replay keeps 241 real endpoints with 29 by 29 texels per cube face. The physical
84-side mesh is unchanged. Only retained display vertices are sampled for the
preview. The texel payload is 82,693,848 bytes, compared with 144,442,200 bytes for
49 legacy endpoints. At most four dated events are retained per frame. This
lower display resolution needs native visual review; it is not a terrain LOD.

Island weathering now uses the island's own elapsed age, rather than aging a
young volcano at the age of an older neighbour. Receiving halo cells still use
their source's age. Coral population relaxation and per-step growth/weathering
caps use elapsed time, so a smaller step cannot create more sand or change the
isolated population response under fixed forcing by repeating a fixed factor. Existing reef
substrate, current/resource feedback and paired sand transfers are preserved.
These corrections do not constitute a fluid-dynamics coral ecosystem. The coupled
resource/growth bake is not established as timestep-converged.

## Evidence

The first production history-only seed-74 audit measured 13.58 seconds for 283
and 28.97 seconds for 285. Removing discarded full-resolution preview work
reduced 285 to 22.45 seconds with the same sampled history hash. This is about
nine extra seconds for finer plate physics, not a full-world loading benchmark.
Shared-machine timing is provisional. The regional terrain refinement, coast
bakes, streaming, save loading and rendering are outside this timing.

Both audited versions produced finite, nonnegative sediment/crust fields. The
maximum sediment accounting residual was 0.005627 m3 for 283 and 0.020336 m3 for
285; maximum continental-volume residual was below 0.000000005 km3.
The 285 replay retained 960 selected events, with original fractional dates.

Reproduce with:

    cargo run --locked --offline --profile release-fast -p engine_worldgen --example geology_history_review -- 283 74
    cargo run --locked --offline --profile release-fast -p engine_worldgen --example geology_history_review -- 285 74
    cargo test --locked --offline --profile release-fast -p engine_worldgen --test history_clock --test island_clock --test erosion_clock -- --test-threads=1

The dedicated tests cover legacy history checksums, temporal endpoints, event
chronology, finite fields, sediment/continental budgets, bounded replay storage,
repeatability across voxel scales, and subdivision-independent island rates.
Results and native review status are recorded below when completed. No claim of
save migration, multiplayer equivalence, full-workspace acceptance or visual
quality follows from compiling the review target.

## Research checked 2026-11-08

- [McDonald and Cordonnier, SIGGRAPH/TOG 2026](https://erosiv.studio/publications/stochastic-geomorphological-transport): transport and erosion have different clocks; momentum and deposition affect braiding and meanders. The current receiver-graph approximation is not their stochastic momentum-conserving solver. This pass improves scheduling rather than claiming that solver's results.
- [Davy et al., 25 August 2026](https://esurf.copernicus.org/articles/14/635/2026/): bedrock abrasion depends on moving sediment and bed cover. Existing tools/cover coupling is retained; this work does not calibrate its coefficients against field data.
- [Llabrés et al., 14 March 2026 preprint](https://arxiv.org/abs/2603.16933): current/resource/growth feedback can produce annular reefs across scales. The bounded island model keeps connected substrate and resource transport, but does not reproduce the paper's resolved fluid dynamics or establish ecological realism.
- [Davis, 27 September 2026 preprint](https://arxiv.org/abs/2610.00301): lazy plate-local networks offer bounded preprocessing, but independent plate domains cannot replace this world's inherited cross-plate drainage without a boundary-coupling design.
- [Schott et al., 2024](https://h-schott.github.io/p/mserosion/): coarse-to-fine erosion should preserve drainage and material inheritance. Playback decimation changes neither the physical mesh nor the terrain samplers.

Further work must compare fixed-seed mountains, volcano-to-sand sequences and
reef shorelines in native captures. A full momentum transport solver needs its
own conservation, resolution and performance study before replacing the current
bounded game model. Generator 285 must not be promoted solely from these audits.

The fine-history erosion pass also partitions each surface interval: stripping
loose alluvium consumes time before underlying bedrock is abraded. Bare beds
receive the full interval; a cover removed halfway through leaves half the
interval. A cover that cannot be stripped shields the bed for that interval.
The old solver could remove cover and then apply a full interval of bedrock
incision, effectively spending the same time twice. This gate-specific change
follows the process ordering discussed in Davy et al. section 4.1; it remains a
reduced graph model, not a reproduced precipiton or grain-transport solver.

## Completed verification, 2026-11-08

Ten focused tests passed both in the shared working tree and in an isolated
candidate reconstructed from the original source snapshots. Four cover physical
history, three cover island timing and three cover erosion-time partitioning.
The isolated candidate contains no concurrent generator-286 changes.
The wider worldgen library unit target compiled successfully; its entire test
suite was not executed. Strict Clippy stopped at an unchanged dependency lint in
`crates/core/src/fast_hash.rs:35` (`chunks_exact_to_as_chunks`). No strict lint
pass, full-workspace gate or save migration pass is claimed.

A separately built untouched solver produced the same generator-283 seed-74
history hash, `0ab48154e3a78698`, as the candidate's legacy branch. Its 49 frames
and sediment/continental budgets also matched. This is production-history
compatibility evidence, not a loaded-save or multiplayer round trip.

Final generator-285 audits all passed:

| Seed | History hash | Maximum sediment residual, m3 | Observed wall time, seconds |
| --- | --- | ---: | ---: |
| 74 | eae3de21efcfe3cf | 0.014240 | 38.36 |
| 16 | 4bc47a2a165a2101 | 0.017530 | 58.39 |
| 731 | 83bda6dd9d0cab5a | 0.019022 | 31.96 |

Each retained 241 real endpoints, 960 selected dated events, and 82,693,848 bytes
of texels. Fields were finite and nonnegative where required. These runs shared
the workstation with other development and rendering work; wall times are not a
controlled performance comparison. The earlier 22.45-second result preceded the
final erosion correction and is not final performance acceptance.

The 29-by-29 replay display and final terrain have not received native visual
acceptance. The hotspot-island/reef bake still follows the plate-history phase;
this change does not animate a complete volcano-to-atoll-to-sand lifecycle in the
loading timeline. The default remains 283. No commit or source-file replacement
was made over the later concurrent climate, ecoregion or cover/cave edits.
