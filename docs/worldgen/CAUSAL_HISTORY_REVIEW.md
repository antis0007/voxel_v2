# Causal settlement history preview

2026-11-06. Generator 254, history model revision 1. Review-only; ordinary world
defaults and saved-world admission are unchanged. This is a two-hold regional
model, not a city economy or a complete historical civilization simulation.

## Source audit and causal boundary

The current planet already advances crust through 240 Myr in 5 Myr steps
(`planetary/drift.rs`). Spherical hydrology routes runoff on the same physical
mesh (`planetary/hydrology.rs`). The explicit landscape-history previews use
six climate stages, glacial quarrying/till, thaw slope failure and bank scour
(`planetary/epochs.rs`). Their compressed erosion stages do not establish
calibrated dates for individual floods or deposits. Regional canyon/erosion
work remains owned by the terrain task.

`civilization.rs` surveys solved ground, samples actual bedrock, rejects wet or
steep foundations, and selects livelihoods from flood exposure and lithology.
Its existing generator 242+ runs 24 twenty-year steps. Occupancy versus peak
households controls intact houses versus ruined lots in `civilization/fabric.rs`.
The buried storehouse has an actual SurveyCache; `ruins::cache_loot` and
`world/containers.rs` connect it to found text. Previously that text only had
aggregate population and the last hazard date.

The settlement task is separately changing cultural streets and construction
under `world/farming/jobs/quarter`. This change does not edit those layouts,
road rules, the terrain solver, or rendering shaders.

## Implemented increment

Gate 254 samples the physical plan at one-metre survey resolution for all
voxel scales, then advances 50 twenty-year steps to year 1000. One seeded
regional weather draw affects the two sites according to their surveyed flood
exposure or talus. The model records harvest shortfalls, bounded food aid,
paired migration, growth, shortage departures and abandonment. Each event
includes before/after stocks and households, unmet demand, and discarded
surplus, so transfers and losses can be audited. A shortage departure is not
asserted to be a death. Food units are abstract cohort supplies, not runtime
inventory items.

The same resulting `History` drives existing physical house/ruin generation,
resident counts and archive text. Archive claims read those records. Present
sediment and the first modeled flood are reported separately; neither dates
the burial itself. No new factions, named figures or lore concepts are added.

`generation_record` exposes immutable region records with seed, generator,
planet recipe fingerprint, solved landscape checksum, model revision, region
coordinates and history horizon. Site IDs retain the existing stable encoding;
event IDs combine site ID and year, scoped by the generation provenance.
An explicit integer digest includes provenance, surveyed inputs and events.
`Chronicle::replay` reconstructs history from recorded inputs without terrain
sampling and rejects unsupported model revisions.

The model runs on a plan cache miss, not per voxel or per rendered frame.
At most 100 transition records exist per region. The existing bounded
per-worker plan cache remains; scale changes reuse the same physical plan.
There is no new ongoing simulation or second owner for live inventory,
residents, edits or regional settlement ledgers.

## Reproduction and limits

```powershell
cargo test --offline --profile release-fast -p engine_worldgen --test chronicle -- --nocapture
cargo run --offline --profile release-fast -p engine_worldgen --example chronicle_review -- OUTPUT_DIRECTORY 16
```

The review tool also accepts centre X/Z metres after the seed. It writes the
actual surveyed sites, per-step transitions, discoverable archive text and a
manifest with timings/checksums. It checks record replay, native cache/loot
sampling and inhabited roof agreement with skyline plans at 1, 0.5 and 0.25 m.
The app benchmark allowlist admits Planet gate 254; ordinary save/load does
not yet admit this preview. Legacy generation follows the original branch.

This record is an immutable generation baseline, currently cached in memory;
it is not a new persisted macro-world snapshot or multiplayer protocol. A
later persistence change must pin the recipe/model revision and keep runtime
ownership, journals and edits authoritative. Cross-platform floating-point
geology equivalence, multiplayer, offscreen runtime evolution, cross-region
trade, wars, resettlement, historical river migration and substantial cities
are not proved by this increment. The flood/rockfall events are bounded
history-model outcomes conditioned on surveyed terrain, not dated hydraulic
replays. Regional road passability is still the existing straight survey.

## Validation on 2026-11-06

Five native integration tests passed: 64-seed household/food accounting and
paired transfers, a flood-exposure counterfactual, deterministic replay and
disconnected routes, archive evidence, and the work bound. One thousand
regional replays including digests took 15.505 ms; retained event capacity
was at most 8,800 bytes per region. These are CPU diagnostics on a shared
workstation, not game-frame benchmarks.

| Seed / centre metres | Sites | Regions | Events | Cache checks across three scales | Inhabited roof checks | Cold regional survey | Cached record lookup |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 / 0, 0 | 18 | 14 | 900 | 54 | 138 | 191.882 ms | 0.036 us |
| 731 / 0, 0 | 24 | 15 | 1148 | 72 | 201 | 116.152 ms | 0.035 us |
| 74 / 141500, -14600 | 12 | 10 | 610 | 36 | 72 | 191.010 ms | 0.042 us |

Every checked cache was produced by `sample_block`, entered the production
loot route and returned the record's archive text. Native roof voxels matched
the existing skyline plans and the final occupied household counts. Each
record replayed exactly without terrain sampling, and each scale returned
the same physical record. These are native library checks; no new rendered
game capture or app UI playtest is claimed.

The existing geological preparation took 40.723, 19.518 and 25.037 seconds
respectively, outside the regional survey times above. This work does not
resolve that preparation cost.

A separate executable built from the original snapshot and the changed
executable produced byte-identical generator-243 settlement CSVs for seeds
16, 74 and 731 at the origin. Seed 74 has no admitted sites in that window;
the new preview's seed-74 review uses the coastal centre shown above. No
save-format migration or default generator change was made.

The broader library-test build is blocked by existing failures in
`planet.rs:626` (missing `planet` field), `plants.rs:140/149` (removed `new`
constructor) and `trees/structure/tests.rs:142` (missing `lobes`). Clippy is
blocked by existing approximate-TAU constants in `trees/relict.rs:88/100`.
The dedicated `--test chronicle` target and both review examples compile.
Native app compilation, save/join round trips and visual acceptance remain
unverified.
