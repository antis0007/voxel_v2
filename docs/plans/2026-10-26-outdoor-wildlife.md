# Outdoor movement and passive wildlife

## Scope and ownership

This pass follows the outdoor-lag report and the supplied generator-55 brief.
HEAD at start: `56bde2b7`. Work stayed local; no hosted CI or player-save
migration was run. Builds, disposable worlds and captures live under
`D:/codex-builds/outdoor-playability-20261026-01a17648/`.
The shared `target/` was read only. Existing dirty work was preserved.

Concurrent architecture work advanced the current generator to 56 during this
pass. Its geometry and art are separate work. This pass does not claim that the
six-place overhaul or modded-Minecraft parity is accepted.

## Acceptance conditions

- CPU streaming stalls must not lower scene resolution. Missing, stale or
  mismatched GPU readbacks must not be reused as fresh evidence.
- Cached terrain previews must match the existing sampler; exact asynchronous
  results replace previews and configuration/interest changes clear old entries.
- Passive animals must appear in generated outdoor grass, retain metre-sized
  bodies at 1, 0.5 and 0.25 m voxels, avoid solid walls, fluids and large drops,
  and flee without entering the hostile attack path.
- Spawn identities, position, health and deaths survive saving and unloading;
  version 36 loads with an empty wildlife ledger.
- Native captures must exercise the actual animal AI and renderer at each scale.

## Implementation

`quality.rs` now uses fresh GPU timings for automatic quality decisions.
CPU stalls no longer count as evidence that resolution or lighting must fall.
The controller still lowers settings under sustained GPU overload and does not
automatically restore previously lowered settings. GPUs without timestamp
support retain the selected quality.

`streaming/columns.rs` caches preview column ranges while exact planning is
pending. Interest trimming and world-configuration resets bound the cache.
The voxel sampler, generated terrain and draw ownership are unchanged.

`adventure/wildlife/` adds sheep and goats using the existing actor list,
collision queries and batched model stream. Their stats are in
`assets/bestiary.ron`. Herd candidates use the world seed and 48 m patches;
each patch has two stable slots. Up to 12 passive actors are active independently
of the hostile cap. They graze, wander near home and flee close players or damage.
Locomotion checks the full body and foot support, allows one-metre steps and
refuses water, lava and larger drops. No per-animal draw pipeline was added.

The wildlife ledger retains up to 4,096 visited animals, including dead ones.
Inactive animals retain their last state; they do not simulate off screen.
Once the ledger is full, new animals stop appearing in previously unseen patches.
Animals do not contribute guardian-clear records, hostile alerts or combat XP.
Breeding, taming, feeding, wool/meat production and predator ecology remain open.

## Persistence

Save 37 appends a bounded, checked wildlife block after chip data. Save 36 and
older omit the block. Live actors supply their current position, health and yaw
when saving. Decoding rejects duplicate identities, invalid species, oversized
ledgers and non-finite state. Existing generator gates and saved generator
versions are untouched by this change.

The shared source already contained save-35/36 manufacturing changes. Those
remain separate dependencies and were included in the frozen validation build;
they are not attributed to this pass.

## Outdoor measurements

One paired flyover: seed 2738958700, Hills, 1920x1080, 28 chunks, full scene
resolution, fixed weather, 20 s warm-up and 30 s measurement. Both automatic
quality controllers were disabled. RTX 4090 / Vulkan. See `baseline-moving/`
and `candidate-moving/` for executable hashes, arguments and CSV samples.

| Metric | Before | After preview caching |
| --- | ---: | ---: |
| Mean FPS | 59.47 | 60.52 |
| Median frame, ms | 16.216 | 15.985 |
| p95 frame, ms | 22.969 | 22.672 |
| p99 frame, ms | 26.943 | 26.891 |
| Maximum observed frame, ms | 104.181 | 81.041 |
| Mean streaming CPU, ms | 7.044 | 6.853 |

This is a small change, not evidence that outdoor lag is solved. Background
streaming makes resident counts differ between runs. Terrain shading and cloud
raymarching each still cost about 6 ms in this scene. The maximum-frame result
is one observation, not a guaranteed reduction in stutter. Near-camera captures
in `baseline-near/` and `candidate-near/` were inspected; no geometry or detail
was deliberately removed for speed.

## Validation record

Focused wildlife, quality, preview-cache, content and save compatibility checks
are recorded in the task evidence directory. Native wildlife reviews use the
existing enemy-review platform with actual AI/collision and a separate save
directory at each voxel scale. They are not proof of structure traversal or
large-population performance. Final counts and executable hashes are recorded
in `validation.json` there.

## Remaining place-quality blockers

The supplied brief remains the acceptance target: coarse one-metre roofs,
town enclosure, mine/pit sightlines, library circulation, underground colour
separation and tree-LOD continuity need matched native evidence and physical
traversal across seeds/scales. The R7 documentation and captures are available;
its named executable was absent when checked. Existing save and source-level
checks do not substitute for those missing acceptance results.
