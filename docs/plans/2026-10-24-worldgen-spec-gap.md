# Worldgen spec gap map (W-GEO/HYDRO/BIO vs current code)

Status (2026-10-24): gap analysis only. No SDD, no code, no generator gate.
Input: [W-GEO/HYDRO/BIO](../specs/W_GEO_HYDRO_BIO.md) (decision [0023](../decisions/0023-worldgen-spec-input.md)).
Rows W11–W19 in [MASTER_PLAN](../MASTER_PLAN.md) §2. Sequencing in §6 is unchanged.

## Current source facts

- `CURRENT_GENERATOR_VERSION = 51` in `crates/voxel/src/config.rs`. MASTER_PLAN §1
  says 48 and CLAUDE.md says 47; both are stale. A peer reserved gate 52 for
  worldgen trees and dungeon work, so the next worldgen gate depends on that.
- All worldgen state is `f32`. `drainage.rs` uses `Vec2` in metres.
- `drainage.rs`: jittered nodes on a 192 m lattice; generator 50 adds eight-way
  steepest descent and meander. Generator 50 is the interim fix that
  decision 0023 says not to extend. Heads are pinned to generator 41 to avoid
  recursion. A node with head ≤ 12 m is ocean. A local minimum is `ClosedBasin`
  and becomes a small sink, not a lake.
- `lib.rs:549` `water_table(cfg, x, z)` exists. It is a surface for cave flooding,
  with no recharge, permeability or distance-to-channel model.
- `landscape.rs` lines ~325–340 derive volcanism as a scalar intensity from plate
  convergence and divergence bands. There are no discrete cones, calderas or lava
  fields. Lines ~443 add glacial cirque bowls on upper flanks.
- `valley.rs` marks lake cells by a flag on a profile. It is not a priority-flood
  result with spill level and single outlet.
- `rainfall` comes from `landscape::visual_ecology`, a per-point value. It is not
  an accumulation over a flow network.

## Stage map

| Spec stage | Row | Exists | Missing |
|---|---|---|---|
| P0 plates | W11 | Convergence/divergence bands in `landscape.rs` | Voronoi plates with velocities, hotspot tracks, cube-sphere mapping (0024) |
| P1 lithology, folds, faults | W11 | Lithology appears in `landscape.rs`, `terrain_variety.rs`, `seam_route.rs`; not a stratum stack | Stratum stack with K and permeability per column; folds; faults; hogbacks |
| P1 volcanism | W11 | Scalar intensity | Cones, calderas, lava fields, geothermal |
| P2 climate | W12 | Metre-space climate module (`landscape.rs` header); `architecture*.rs` contains insolation/circulation/orographic words, not read | Latitude-based insolation, Hadley/Ferrel/polar cells, orographic lift and rain shadow, seasons, snowpack |
| P3 stream-power erosion | W13 | Interim generator-50 meander | Implicit stream power, priority-flood lakes, glacial and periglacial erosion, landslides |
| P4 network, Q, Strahler, groundwater | W13 | Node graph with heads and a per-point rainfall | Flow accumulation (D∞), Q with seasonal curve, Strahler order, lake outlets, recharge-driven table |
| P5 coasts, deltas, reefs, tides | W14 | Coast bands; estuary and delta words in `landscape.rs` | Exposure × tide × sediment classes, Galloway deltas, reefs, tidal runtime |
| P6 channel planform | W15 | Meander and width-varying channels (generator 50) | Step-pool, pool-riffle, braid, anastomosis, cutoffs, confluence mixing, waterfalls |
| P7 karst, caves, springs | W16 | Caves in several generators | Groundwater-driven karst, springs, sinkholes |
| P8 soils, microclimate | W17 | Soil words in dressing code | Soil depth and type planes feeding fertility and moisture |
| P9 biomes and vegetation | W18 | Biome strata in `landscape.rs`; `trees.rs` | Tolerance-curve community assembly, succession state |
| P10 wonders, names, roles | W19 | Nothing found by grep for "wonder" | Scheduler, catalogue, framing, phonotactic names, role registry, variety validator |

## Constraints the SDDs must resolve

From decision 0023:

- Fixed-point planning outputs (cm, L/s) are required by §12. Current code is `f32`.
- The §0.7 budget of ≤ 4 ms per 32³ chunk is unmeasured. The committed terrain
  bench baseline is about 10 ms per chunk on the desktop. Hardware measurement is
  open.
- Every older generator version must keep generating identically. Any new W-row
  output needs a new gate after 51 and must not edit an earlier gate.

## Proposed next steps (not started)

1. **W13 SDD (Tier A, M3).** It is the first to need an SDD and it owns `hydro_basin`
   (spec §13). Its Q output is the waterwheel input. Open question: MASTER_PLAN §6
   fixes the next five sessions to M1 slice SDDs and spikes, and worldgen is not on
   that path. Pulling W13 forward is a sequencing decision for the human.
2. **Spike before SDD:** one offline priority-flood plus D∞ accumulation over a
   16 km window at 128 m cells, outside the game, to measure node count, runtime and
   whether `hydro_basin`'s monotone-Q check holds. This needs no generator gate. It
   also needs a Rust target build, which is blocked on disk space right now.
3. **W12 climate** can wait. W13 can start with the current per-point rainfall as
   an explicit placeholder that the SDD labels.

## Spike result (2026-10-25)

Offline, outside the repo: clean copy of HEAD 95155596 (generator 52) plus a standalone
crate. It samples `sample_column` height on a 125 × 125 grid (16 km, 128 m cells, 1 m
height steps held as integer cm), then runs Barnes epsilon priority-flood (1 cm), D∞
accumulation in descending filled elevation, D8 paths from the flood parents, lake
components (fill depth ≥ 1 m) and Strahler order. Source stays in the session scratch
directory; nothing here changes the repo or a gate.

| Measure | Result |
|---|---|
| Hydrology cost, 15.6k cells | flood 1–2 ms, D∞ accumulation 2–4 ms (28 ms once under a peer build) |
| Hydrology cost, 250k / 1M cells (mirror-tiled heights) | 30 + 24 ms / 187 + 150 ms |
| Height sampling, 16 km window | 29–34 s single thread (1.9–2.2 ms per column) |
| Height sampling, 4 km window at 32 m | 1.65 s (0.1 ms per column) |
| Height sampling, 8 worker threads | 57 s, slower than one thread; cause not measured |
| Largest catchment | 4–6 km² in a 256 km² window, 3 windows |
| Closed depressions ≥ 1 m | 304–387 per 256 km² window; 6–8 % of cells raised by the fill |
| Channel steps (≥ 0.25 km²) whose bed rises downstream | 110 of 529 to 206 of 759, every one inside filled cells |
| Channel climbs on unfilled bed | 0 |
| Channels reaching sea | 94–95 % at 0.25 km²; the rest leave the window edge |
| Q monotone, D8 accumulation | 0 drops |
| Q monotone, D∞ accumulation read along the D8 path | fails: 44–64 of 529–759 channel cells at 0.25 km², 4–12 at 1 km²; drops up to 99 %, mostly where the D8 parent is below the channel threshold |
| Q monotone, D∞ on hillslopes and whole flow to the D8 parent once a cell is a channel | 0 drops, 0 gaps; channel count within 1 % of pure D∞ at 0.25 km² |
| Lakes with one outlet cluster | 290 of 304 (far window), 64 of 72 (4 km window) |

Reading:

- Cost is not the risk. Priority-flood plus accumulation is about 0.35 µs per cell. The
  cost is the height input: `sample_column` at sparse spacing pays per erosion tile, so
  a 16 km window costs about 0.11 s per km² at both 128 m and 32 m cells. W13 needs its own
  heightfield pass, not per-cell `sample_column` calls.
- The gen-52 heightfield is not hydrologically coherent at 128 m. Catchments stay under
  6 km² and every window holds hundreds of pits. The river graph lives in `drainage.rs`
  nodes, not in the heights. Flow accumulation on current heights does not reproduce it,
  so the W13 erosion stage must carve heights from the network or replace both.
- `hydro_basin` "Q increases downstream" fails with plain D∞. The hybrid above passes by
  construction and moves the 0.25 km² channel count by under 1 %. The SDD should specify it.
- "One outlet per lake" holds by construction per flood basin (one pass cell). The 5–11 %
  failures come from the spike's lake definition (connected cells deeper than 1 m), which
  fuses nested basins and counts wide rims twice. The SDD should key lakes on basin
  identity. Not diagnosed per lake.
- Limits: heights quantised to 1 m make some pits and flats; window edges truncate
  catchments; 1M-cell timings use mirror-tiled data with 58 % ocean; timings were taken
  while peers were building, so read them as upper bounds.

## Not checked

- No runtime run, no build, no test in this pass.
- Grep hits for "lake", "karst", "biome" and "soil" were not read line by line.
  Treat "exists" above as "a name or code path is present", not as a feature check.
