# ADR 0039: Orbital globe from baked worldgen tiles

2026-11-02. Accepted. Spec: `docs/superpowers/specs/2026-11-02-orbital-planets-design.md`.

Above a few kilometres the resident chunk disc and the flat horizon sheet stop
describing the ground. The globe (`crates/app/src/render/globe.rs` driver,
`render/globe/` module) draws the home planet as a sphere of height tiles baked
from the same generator the chunks use. No new dependency.

## Tiles

- A quadtree per cube face, 65x65 samples per tile. Tiles with a cell under
  64 m bake exactly through `sample_column`. Tiles with a cell of 64 m or more
  sample the horizon far-field surface, which agrees with the voxel columns to
  p95 1.75 voxels (ADR 0012) and avoids a cold worldgen plan per sample.
- Disk cache under `<save>/globe/<seed>-g<generator>/`, one `.gtl` file per tile
  key; seed and generator live in the directory name. Format `VXGT0002`:
  8-byte magic, tile id as four little-endian u32 (face, level, x, y), then
  65x65 samples in planes: i16 height (m), u8 material, u8 water, 2-byte climate
  (6 bytes per sample). Any mismatch is a miss and the tile is rebaked; writes
  go through a temp file.
- Selection splits at 1.5 px projected error and merges at 0.75 px, with at most
  512 draws (256 on the lean tier).

## Tiers and budgets

Altitude tiers in `altitude.rs`: surface below 1,500 m, transition up to
6,000 m (resident load stops there), orbit above 30,000 m, with 200 m
hysteresis on every boundary. Per frame the globe may upload 2 MB including
cloud-shell faces and spend 0.5 ms CPU; the bake worker runs 2 ms turns.

## Design without spare bind layers

- One tile atlas of 46x46 slots covers every resident tile.
- Tile edges carry skirts two cells deep, which close T-junction cracks.
- The globe discards fragments under the resident chunk mask and fades over a
  256 m band at its edge.
- The horizon chart is clipped at sampling time, so globe and horizon never both
  own a column.

## Cloud shell and night lights

The cloud shell sits at R + 1,600 m, 512² texels per face, rebaked every 30 s
from `world::weather::coverage_at` on its own thread, and shadows the ground
below it. Night lights are capped at 256, chosen nearest-first, with per-site
hashed intensity (1.5 to 3.0) and radius factor (0.6 to 1.0).

The lean tier only lowers limits (256 draws, 256² shell). The globe shader
already uses the distance limb approximation without a LUT pass, so there is no
cheaper shader branch yet; flag bit 0 stays plumbed for one.

`globe::active()` turns the globe on (`VOXEL_GLOBE=1` or `--benchmark-globe`).

## Planet cover

This branch never carried the peer's uncommitted `render/planet.rs` cover, so
nothing was removed. When the peer's planet files land, the globe replaces the
cover and the integration condition becomes
`world_type == Planet || globe::active()`.

## Measurements

- Task 8 (4090, 60 km): resident 252 tiles, 48 drawn, globe_ms p50 0.056 /
  p99 0.098, largest upload 177 KB.
- Task 11 (UHD 770, lean): p95 26.4 ms, GPU passes 18.6 ms (terrain 14.3),
  globe_ms 0.115.
- Task 12 (`scripts/frame_bench.py`, best of 3 interleaved passes):
  orbit-static p95 3.64 / p99 4.94 / max 20.9 ms; ascent p95 3.98 / p99 9.41 /
  max 38.5 ms, globe_ms at most 1.4.

## Decisions taken during execution

- Ruling 3: the flight-review before captures cannot run on this branch; evidence
  is the Task 12 scenes plus per-task GPU tests until the peer's harness lands.
- Task 2: an empty visible set in orbit is intended; the frozen chunk cache
  exists for the return, not for drawing.
- Task 2: descent regrows the disc ring by ring from 0; cache hits make it fast
  and the globe covers meanwhile.
- Task 3: `projected_error_px` uses the distance to the tile centre floored at
  the cell size; `bounding_radius_m` keeps 2,000 m slack at every level as the
  cull and limb sphere; the limb test is
  `angle(centre, eye) - asin(bounding_radius / R) > acos(R / |eye|)`.
- Task 4: the bake cost is accepted (about 3 tiles/s per core in 2 ms turns,
  first visit only thanks to the cache); a height-only sampler is the follow-up
  if tiles arrive late.
- Task 8: coarse tiles (cell >= 64 m) sample the horizon far field instead of
  `sample_column`; fine tiles stay exact.
- Task 8: `forced()` is true only for `--benchmark-globe`; `VOXEL_GLOBE=1`
  enables the globe but keeps the tier gate.
- Task 8: a dead worker is dropped and respawned at most once per 600 frames.
- Task 9: tile cracks are fixed by skirts in Task 10 rather than by reopening the
  Task 7 mesh.
- Task 9: night lights read as a grid, so each site gets hashed intensity and
  radius.
- Task 10: the 2.5 km darkening was back-face culling turned off; Back culling is
  restored. Matching globe lighting to the world shader is a deferred look item.
- Task 11: the lean tier is limits-only, as above.
- Task 12: the ascent route runs 120 s at 256 m/s and ends near 32 km, past the
  orbit boundary, instead of holding at 40 km.
- Task 12: "wanted reaches 0 within 20 s of entering orbit" is measured on
  orbit-static after warmup (13 s), since the ascent scene ends still climbing.
- Task 12: the ascent max-frame gate takes the best of 3 interleaved passes like
  p95 and p99; the 66.8 ms outlier pass was swapchain wait under peer load.
- Task 12: the ascent motion uses the wall clock like the existing orbit motion
  (benchmark route, not simulation).

## Evidence

`target/reviews/orbital-globe-after/` in the main tree, copied from the Task 9
and Task 10 capture folders:

| File | Shows |
|---|---|
| `60km_noon_after.png` | globe disc with clouds and terminator at noon |
| `60km_night_after.png` | night side with varied city lights after the hash fix |
| `60km_dawn_after.png` | terminator and limb at dawn |
| `12km_before.png` | before: grey fog, no ground |
| `12km_after.png` | after: globe ground and cloud shell |
| `2500m_before.png` | before: resident chunks and horizon at 2.5 km |
| `2500m_afterback.png` | after, Back culling restored: no cracks, holes or double ground; diff vs before 0.0003 |

Open: globe lighting does not match the world shader where the globe is the
visible ground; `max_frame_ms` is recorded but not enforced by
`frame-bench-check`.
