# Integrated lighting and streaming review — 2026-10-18

The current integrated release is launched by `run-voxel-gameplay.cmd`.
Its frozen source, build log, native captures and receipt are under
`target/integrated-world-review`. No tests or CI were added or run.

## Lighting fixes

- Indirect lighting now reconstructs each ray at the center of the full-resolution
  pixel whose depth it actually loaded. Previously the ray and depth described
  different positions, producing horizontal obscurance stripes on flat ground.
- The half-resolution resolve rejects individual depth/material samples before
  filtering them. The previous bilinear read mixed different surfaces before
  rejection. GI and ambient occlusion remain enabled.
- Sun-shadow filtering evaluates the receiver plane separately at each PCF tap.
  Cascades retain their actual texture/matrix pair while coverage remains valid;
  changed receiver coverage refreshes immediately. Night shading skips PCF when
  its contribution is zero, retaining the shadow debug view.
- Surface noise uses an integer cell hash to avoid correlated lattice patterns.
- Near water reuses its existing depth walk to sample the bed's actual color.
  Light through shallow water is attenuated by depth and viewing angle; deep
  water keeps its scattering color. The redundant night reflection dimmer is
  removed. Cave sky visibility remains active. This is an analytic bed-color
  approximation, not screen-space refraction, and adds no vertex fields or pass.

The reported save and camera were reproduced from a **copied** checkpoint and
region directory. Original player data was not used for simulation writes.
`night-normal`, `night-no-indirect`, `night-no-shadows` and `night-repaired`
contain the native isolation captures. The horizontal bands disappear in the
corrected GI-enabled capture. Disabling sun shadows alone leaves the bands.
The remaining black foreground shapes are water, visible as dark green surfaces
in `day-repaired/saved-world.png`; they also remain with sun shadows disabled.
The final `night-water` and `day-water` captures verify the water correction,
with GI enabled and zero missing chunks in both views. Deep water remains dark
at night; it is not replaced with emissive illumination.

## Streaming and crash fixes

- Edited-horizon clipping preserves indexed triangles away from actual edits.
- A worker remaps large backdrops into pages no larger than 64 MiB per buffer
  (or the device limit, if smaller), preserving all triangles and spatial ranges.
- Horizon staging writes at most 8 MiB per frame. The previous complete backdrop
  remains available until every replacement page is uploaded.
- Full and nearby world snapshots have separate caches. They no longer replace
  each other's maps twice per frame. The horizon only requests a full snapshot
  when it passes its worker, movement and edit-check gates.

The old edited-horizon failure requested a 343,953,568-byte vertex buffer against
a 268,435,456-byte device limit. The integrated edited-courtyard review completed
with 52,685,584 vertex bytes, 16,328,304 index bytes, and a nine-frame upload.
The moving physics review exercised four-page replacement backdrops, including
total geometry larger than a single device buffer, without a buffer crash.

## Actual measurements and limits

RTX 4090, Vulkan. These are actual game runs, not automated tests.

| Workload | Result |
| --- | --- |
| Integrated release before final water-only change, copied saved world, 1600×900, view 16, physics and HUD on | 90.67 FPS; P99 20.703 ms; maximum 28.026 ms; 2328 requested/resident chunks, zero missing at the end |
| Corrected saved night, 1600×900, view 16, physics off | 68.27 FPS; P99 23.868 ms; 3069 requested/resident chunks, zero missing |
| Same settled save snapshot stage | 0.002 ms; the earlier isolation run spent 1.900 ms rebuilding snapshots |
| Quarter-metre moving Hills, 1280×720, view 16, physics on, 12 m/s | 118.36 FPS; P99 14.417 ms; maximum 84.596 ms |
| Dusk moving Hills, 1280×720, view 16, physics off | 65.85 FPS; P99 23.452 ms; 174 missing at the final moving pose |

The full physics travel profile still outran generation: 4164 of 6798 requested
chunks were missing at second 90. This is a remaining streaming limitation,
not a VRAM budget success. Repeated dirty-chunk cancellation during simulation
is a candidate for follow-up coalescing; it has not been rewritten or verified
in this lighting release. Cloud raymarching also remains a substantial GPU cost.

The launcher's default disables the additional local-mist layer, following a
same-executable review that found about 2.35 ms extra terrain GPU cost for little
visible benefit. Atmospheric aerial perspective, fog and clouds remain active.
An explicit `VOXEL_LOCAL_MIST=1` opts back in.

The final saved-world physics profile and executable identity are recorded in
`target/integrated-world-review/release-receipt.json`.
That profile also uploaded 468,318,320 bytes of replacement horizon geometry
across seven pages and 56 frames; the largest vertex page was 67,108,832 bytes.
Simulation recovery remained active with about 2.39 million pending cells, so
this proves bounded rendering during recovery, not completion of recovery.

The delivered executable is `voxel-lighting-water.exe`, SHA-256
`4CE2874F0F30BC69C895D59E59AAAEE3BECAE1B99F7F9C79EA952147013D7201`.
Its source is `water-source`, built from the reviewed `release-source` with the
bed/reflection correction. Later concurrent streaming and water-wave changes
in the shared checkout are separate from this frozen artifact. Final water
reviews each used a 16-second warmup and four-second static measurement, with
physics off; their timings do not replace the longer physics profile above.
