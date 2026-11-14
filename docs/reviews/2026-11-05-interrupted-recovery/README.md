# Interrupted Claude work recovery

Reviewed the recent Claude transcripts in the local voxel project, including
the cave-performance, water-motion, plant, planet, weather, icon and event-log
sessions. This is a source/integration audit; rows below are not gameplay acceptance.

## Changes in this recovery

- Connected water-height history to cached-source patches, sealed it at normal
  and urgent mesh dispatch, published the shader clock and stamped uploaded
  vertices. Shared snapshots retain their own history when the next patch arrives.
- Removed heap allocations from corner-current evaluation. Accounted for motion
  history in source memory, and release it before returning its allocation charge.
- Reconstruct previous wet/dry corner state when interpolating a changing surface.
- Stop a waterfall's second gravity transfer from emptying its intervening row.
  Both transfers still use the same conservative volume transaction.
- Finished the visible-height publication optimization from session `627c886b`:
  the drainage dirty sets, exact-source observations, edit proxies and quality
  proxies now share its 1/32-cell height threshold. Water creation/removal remains
  a separate visible state. Exact simulation, persistence and network amounts are
  retained; this threshold controls remeshing only.
- Sheltered water uses inexpensive current-driven ripples instead of the outdoor
  wind-wave spectrum. Skip sunlight caustics under a closed roof. The comparison
  override `VOXEL_CAVE_WATER_FAST=0` retains the former shading path.
- Fixed stalk caps for a one-cell height limit and defer growth at unloaded cells.
  Wild bamboo reuses the sampled terrain column, rejects cells outside its height
  range early, and has its missing gate-139 registry entry and benchmark admission.
- Recorded both existing meanings of gate 136. Custom planets already write and
  reload that number, so renumbering it would break existing recipe interpretation.
  The ordinary generator default is unchanged.
- Recovered the finished ladder, vines, sapling, bore and gold-pan sprites from
  session `8de38c9b`'s stranded `misc.py`, and connected them to the item atlas.
  Authored sprite preview: `target/reviews/interrupted-recovery-20261105/recovered-icons.png`.
- Clarified the requested continuous-liquid/volumetric-gas physics target in
  `docs/ENGINE_CORE.md`. The current cellular solver remains an approximation.

## Other interrupted areas

| Area | State found during this audit |
| --- | --- |
| Cave visibility | Default-on culling and flood fixture already completed in `7f665a6c` |
| Water shader interpolation | Concurrent chat "Build living forest ecosystems" is completing the shader consumer; preserve its work alongside this recovery |
| Forest crowns, river corridors | Concurrent forest work owns these; incision is already called from `geo::ground` |
| Weather and terrain textures | Concurrent chat "Recover space and finish terrain" owns the active recovery; do not copy stale scratch weather variants over it |
| Station interiors and life support | Concurrent chat "Add staffed elevator stations" is implementing these |
| Moon handoff | Anchor movement and the foreign-ground mesh are already connected to the player/model paths; runtime landing acceptance remains separate |
| Status effects and potions | Timers, brewing, loot, controller effects and HUD consumers are present; the last follow-up was new feature direction, not a missing module |
| Event notices | Event history, pause Log, detail hover and banner bearings are already connected |
| Viewmodel sway | Lens-relative arm fix and item-dependent sway rate are present |

## Verification

- No tests or hosted CI were added or run.
- An initial shared-tree app check and optimized build passed before later peer
  station/industry edits. Later attempts validated all 25 composed WGSL programs,
  but hit errors in those changing peer modules; see the adjacent build logs.
- A frozen source copy and SHA-256 manifest live under
  `target/reviews/interrupted-recovery-20261105/`. `frozen-check.log` records its
  successful independent app compile (6m 32s), without the shared target lock.
  The later shared optimized build still failed to resolve
  `engine_sim::atmosphere` although that export exists in the frozen source and
  compiled successfully there. Do not use the shared build as a current binary.
- `tools/Review-CaveWater.ps1` captures both shading modes from one executable,
  reverses alternate rounds, uses disposable saves and records concurrent load.
  It compares sheltered-water shading, not the entire fluid implementation.
- Native cave timing and fluid-motion acceptance remain unmeasured until a
  current executable can be captured without competing renderer activity.
