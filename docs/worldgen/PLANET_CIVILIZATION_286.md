# Planet and civilization integration, 8 November 2026

This continues [the orbital surface pass](ORBITAL_SURFACE_286.md) and integrates
the [regional communities work](../design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md).
The civilization owner finished the pending physical-economy integration. A
second agent reviewed generated roads, and another reviewed the asynchronous
landmark discovery change. Build and native observations are recorded together
here rather than inferred from the individual source changes.

## Changes

- Planet 286 migration and campaign connectivity now uses the same generated,
  terrain-validated road that history and road voxels use, for the exact pair of
  original settlements. Previously it resampled a straight shortcut. Other pairs
  retain their old behavior. There is no new terrain, settlement placement or
  continent rewrite; travel duration and edited-bridge connectivity remain limited.
- Reliquary surveys and reward-kind lookup run in a bounded background queue.
  Polling does not wait. Obsolete work is cancelled after a position/configuration
  change, and stale replies are discarded. Actual placement, saved reward flags,
  doors and stock stay on the live World. The query state stays with its realm
  during portal exchange, while multiplayer players retain independent requests.
- Newly initialized communities use finite physical stock and the existing work
  executor. Recruitment/return, saved custody, recorded deaths and market stock
  are reconciled. Early recruitment cannot duplicate a town worker; community
  barter requires the resident to be at the actual stockpile. The ordered queue
  services at most eight events per fixed tick and reuses executor storage.
- The civilization report now includes individual town stages, stock locations,
  bread/planks, worker/enlisted/dead IDs and deadlines. The sidecar validates
  ownership and complete wake records. Old nonphysical ledgers retain their
  previous economy. See the civilization document for scope and capacity limits.

## Generated place and build

The metadata-only `settlement_survey` example reads the real generator without
placing a review fixture or performing assertions. Seed 74, Planet 286, one-metre
voxels, no custom recipe: the 767 m window near the earlier valley found three
inhabited settlements. The native visit targets settlement **269859896** at
**(323508, 19.02, 1744)**, with six generated households. A neighboring settlement
at (323660, 20.02, 1744) has three. These coordinates were surveyed, not imposed.
The terrain checksum remains `53e34beb3a962eac`.

The integrated `release-fast` build passed in 2m17s. Its 23 relevant source/asset
hashes are in `target/civilization-planet-review/source.json`; no listed source
changed after the build began. Executable: `axiom-286-civilization.exe`, SHA256
`717B3870D1610CB834EF36C0B91E9237DADED53C1471B97A7ECA8AE8DFAA2FC3`.
This is a local optimized candidate, not a shipping-release certification.

## Native review scope

The town comparison uses isolated fresh saves/caches, the same seed, camera,
daylight, weather, 1280x720, view distance 12, enabled physics and a 65-second
measurement. Read-only `/civilization-status` snapshots come from the running
native game every ten seconds. Captures are unedited. The other native run
repeats the previous 110-second orbit/face-crossing/descent route.

Benchmark mode intentionally does not service full world saves. An early
`/save-all` request was acknowledged but produced no main save; it is not a
successful checkpoint. This review therefore cannot establish save/reload,
recruitment/death edge cases, or crash recovery. No unit tests or hosted CI ran.

The town's repeated flat-roof houses remain visually plain. There is no claim
of improved settlement architecture or complete civilian growth/politics from
this integration.

## Observed results and limits

The integrated visit initialized two real generated towns with nine distinct
resident IDs. It serviced 1,132 recorded events with a maximum recorded delay of
one tick. The final snapshot had nine scheduled future events and no overdue
events. Finite chest bread decreased from 12 to 9 and 6 to 4; planks remained 16
in each town. This establishes consumption and scheduled service, not completed
harvests, crafting batches or long-term self-sufficiency. Top-level job counters
describe the player crew and must not be read as community production counters.

The first town and flight reviews both measured only 3.84 FPS. Repeating the
previous binary under the same host conditions gave 3.90 FPS, including stalls
before terrain or civilization became active. With the display active again,
the previous binary returned to 133.97 FPS. Those presentation-limited runs are
retained and do not establish a civilization performance regression or win.
See [the continuity follow-up](VISUAL_CONTINUITY_286.md) for the diagnosis and
the user's subsequent LOD-gap correction. Native save/reload remains unverified.
