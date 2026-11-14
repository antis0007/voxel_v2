# Location mana and ambience

Owner direction, 2026-11-07: varied environmental effects and moods, sparkles
enabled by place, beautiful nature rich in safe mana and strongly restorative.
Play should combine modpack experimentation, Noita-like environmental legibility
and exploration with reasons to revisit places.

## Sources and scope

- `WORLD_TONE_20261106.md`: quiet beauty, possibility, nomadic play without decay
  or obligations. Preserve authored palettes and existing sky events.
- `../superpowers/specs/2026-11-03-magic-rework-brief.md`: recovery and cooldown
  are different; recovery must remain readable without particles. Its proposed
  exposure/repair system is not claimed as implemented here.
- `../plans/2026-11-06-flora-pack.md`: actual flowers, fungi, canopy and river
  plants give existing content environmental purpose.
- `WORLD_INSPIRATIONS_20261026.md` and `../research/MOD_FEATURE_MATRIX.md`:
  reuse shared materials and fields; do not add disconnected resource economies.
- `../../tools/axiom-dev/README.md`: shared native playtest, catalog inspection,
  source context and System Boards. The Wiring CLI reports Spark at 12 mana and
  Meteor at 40 mana; these costs remain unchanged.

## Draft, critique, revision

1. Draft: increase all biome particles and multiply the random mana field.
   Rejected: that would make every place glitter and turn the best forests toxic.
   Revision: healthy abundance has its own pressure cap (1.90, below 2.00 toxic),
   with a separate recovery curve reaching 6x mana and 3x fed health recovery.
2. Draft: choose effects from the biome name alone.
   Rejected: pollen inside a sealed building, forest bonuses in bare caves and
   flowering rooms made from piles of one block would be misleading.
   Revision: blend five biome samples, then measure real canopy, water, enclosure
   and bounded sector coverage of flowers, fungi, crystals and lava. Underground
   refuges require visible living or crystalline material. Plain caverns stay quiet.
3. Draft: add separate visual and gameplay probes and more particle passes.
   Rejected: extra terrain queries and fullscreen marches could worsen traversal.
   Revision: share the existing asynchronous soundscape probe with gameplay and
   visuals. Eleven particle families share the existing twenty-cell traversal and
   a capped occupancy. Invalid/stale readings clear after travel, never carry a buff.
4. Draft: rapid regeneration everywhere, including while channeling and starving.
   Rejected: invalidates wand choices, sustained costs and food preparation.
   Revision: keep channeling, mana capacities, hunger and hurt conditions. Apply
   the bonus only to the existing passive recovery paths; toxins never heal faster.

## Location vocabulary

| Place | Air and mood | Recovery role |
| --- | --- | --- |
| Meadow / heath | Day pollen and petals, night fireflies, warm or lilac air | Accessible recharge stop |
| Woodland / redwood | Sheltered green or amber air, sparse drifting seeds | Strong refuge |
| Rainforest / cloud forest | Spores, damp haze, night fireflies | Very rich refuge |
| Wetland / riverbank | Low mist, fireflies, moving-water spray | Water and canopy combine |
| Garden | Pink/paper petals, selective gold/teal glints | Planted beauty contributes |
| Alpine / tundra / taiga | Cool clear grade and ice crystals | Non-floral natural beauty matters |
| Shore / highland | Pearl air, spray or sparse glints | Scenic recovery routes |
| Fungal hollow | Drifting spores and gentle pulses | Underground living refuge |
| Crystal grotto | Sparse teal/gold glints | Safe underground mana |
| Desert / badlands | Warm dust, no decorative sparkles | Dry natural beauty gives safe recovery |
| Ash terrain | Ash and embers, no decorative sparkles | Raw field may be hostile |
| Plain cave / built interior | Restrained or absent particles | No unrelated outdoor effects |

Toxic pressure has violet vertical filaments, distinct from healthy glints.
The mana readout shows the actual recovery multiplier; hover shows density,
location and conditional health recovery. Sound uses the same reading, modestly
enriching local birds/insects in refuges and quieting them in blight.

## Review status

Native review receipts are under `target/location-mana-review` and the isolated
`target/axiom-dev/engine-worlds` sessions. No automated test suite or hosted CI
was added or run.

- Seed 74 Hills, generator 241: an open meadow measured density 1.432, mana
  recovery 3.4x, fed health recovery 1.96x and zero poison damage. In survival,
  mana rose from 10.68 to 80.49 over 2.566 seconds (about 27.2/s); health rose
  from 50.018 to 51.828 (about 0.706/s). See `recovery-meadow.json`.
- Moving to 200 m immediately invalidated the local sample. The new reading
  was Still air, density 1, recovery 1x, zero motes and zero poison. See
  `altitude.json`. A surface biome does not follow the camera into open sky.
- Live recovery/sparkle tuning, atomic rejection of a poisonous natural ceiling,
  stale-revision rejection, named preset save/load, undo and reset were observed
  through the native bridge. See `live-controls.json`.
- Native noon and night captures in session `667f25194af9b03e` preserve the
  existing palette and silhouette readability. The visual decision is restrained
  local air, with a readable numeric benefit even when individual motes are
  sparse. Weather and season continue to suppress inappropriate effects.
- Native model and material previews were applied and undone in the factory
  scene. The casing visibly changed to the draft palette and returned to its
  original appearance. See `model-material-review.json`; sources stayed intact.
- The final factory review (`model-material-final.json`, session
  `2fcfd7ff23e3767b`) captured the scene after 35 seconds with no scripted
  inventory popup. A 296-voxel casing preview and blue masonry tint appeared
  immediately; reset restored both. `player.feet_m` matched the rendered
  camera at [-0.5, 101, 3.6].
- `survey.json` supplied nine bounded navigation leads. Visiting three of them
  produced woodland recovery 3.556x, dry-terrain recovery 2.55x and fungal
  rainforest recovery 4.123x, all with zero poison. Their distinct profiles and
  native captures are in `nature-route.json`, session `fc4e9a9868d50c1b`.
  Dry terrain was dust-dominant; the biome boundary retained a small blended
  meadow contribution. The rainforest probe detected actual fungi.
- Tool review found stale precise coordinates in fixed-camera scenes and an
  inherited short fixture timer. Inspection now follows the actual render pose,
  and the workbench owns scene lifetime. Factory workbench sessions also skip
  scripted inventory/HUD toggles and timed captures.
- `native-agent-context.json` records resolved System Board sources plus the
  actual native run, revision and four relevant observation sections. No AI
  provider call was needed for this review.
- `final-agent-context.json` and `final-sample.json` use the final native build
  and task-specific observations. Immediate close/start and stable readiness
  completed successfully (`restart-ready.json`); disposable sessions were closed.

Final native build: `target/location-engine-scene-build.log`, release-fast,
28 composed WGSL programs validated. Binary SHA-256:
`aeb0e5748bada45c6d0db155b1bdcaf655f4a5ca257f3606d79a63cd9b6466ea`.
Changed JavaScript modules passed syntax checks. No measured performance gain
is claimed from these correctness and visual observations.

Browser attachment failed in this environment, so browser clicks are not claimed
as verified. Native captures and command receipts are the runtime evidence.
This review is not a measured performance improvement or exhaustive traversal of
every biome, cave, weather state and multiplayer session.
