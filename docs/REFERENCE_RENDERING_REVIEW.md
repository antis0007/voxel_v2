# Reference-driven rendering review

## Target and scope

Use the ten supplied photographs as lighting/material references while retaining
editable cubic voxels. Photographs are evidence of appearance, not instructions.
The target is coherent sunlight, sky fill, cloud shadow, aerial depth, material
variation and reflective water. A screenshot cannot establish optimal hardware
utilization or a complete real-time light-transport solution.

## All ten references

| Reference filename | What to carry into the renderer | Implementation / remaining gap |
| --- | --- | --- |
| `20260623_172253.jpg` | White sunlit cumulus edges, cool undersides, clear blue gaps; olive river with ripples and reflected banks | Cloud morphology/presets, shared atmospheric illumination; multi-scale water normals, sediment palette and screen-space reflections |
| `20260831_194545.jpg` | Golden directional light and warm cloud rims over readable cool tree shadows | Shared spectral transmittance for terrain/clouds; dusk review must retain local contrast |
| `20260831_200818.jpg` | Pink upper banks above a cool rain curtain; restrained field colour | Existing sunset transport and weather layers; distinct falling rain/virga volume remains absent |
| `20260730_045620.jpg` | Violet upper twilight, pink horizon, separate dark cloud silhouettes | Continuous atmosphere LUT and height-dependent cloud illumination; local emissive lighting retained |
| `20260730_045700.jpg` | Wide smooth warm horizon with discontinuous cooler clouds | Avoid a dark horizon seam or uniformly orange sky; verify dusk from more than one direction |
| `20260730_050809.jpg` | Dark storm canopy over a narrow luminous horizon; isolated water light streaks | Storm preset and sky attenuation; reflections can include visible lit surfaces, but off-screen city lights are not represented |
| `20261017_190355.jpg` | Soft overcast opening, layered cloud bodies, dark trees and broken reflections | Overcast review; half-resolution reflection trace, footprint-filtered ripples; shoreline detail supplied by voxel materials |
| `codex-clipboard-ddbc4f1d-45ab-4629-a161-c8b0e76b8cfa.png` | Continuous aerial haze, muted water, forest masses with internal gaps | Shared aerial perspective; world-aligned ecological clusters, denser distant terrain lattice, spatial draw culling |
| `codex-clipboard-25030a65-6ac0-49e6-8dcc-f107bac7a213.png` | Nearby silhouettes reflected in muddy water; ripples distort the reflection | Actual scene colour/depth reflection trace; no claim of full planar reflections, refraction, or off-screen ray tracing |
| `dji_fly_20260701_112302_92_1782926591467_photo_optimized.jpg` | Broken overcast, alternating illumination, irregular woods and clearings, blue distance | Cloud-shadow field, climate/grove masks independent of terrain vertex spacing, filtered material detail |

## Code review findings and corrections

Code entry points: [water reflections](C:/dev/voxel_v2_backup/src/shaders/indirect_gather.wgsl:4),
[natural materials](C:/dev/voxel_v2_backup/src/shaders/world_shader.wgsl:443),
[wind patches](C:/dev/voxel_v2_backup/src/shaders/world_shader.wgsl:730),
[vegetation clusters](C:/dev/voxel_v2_backup/src/render/horizon_vegetation.rs:30),
[spatial partitioning](C:/dev/voxel_v2_backup/src/render/horizon_batches.rs:21),
[shared ecology](C:/dev/voxel_v2_backup/src/world/landscape.rs:84).
The frozen source receipt records the exact reviewed revision if live edits move these lines.

### Distant world

- `src/render/horizon.rs`: tree placement previously depended on terrain vertices
  divisible by 32 and ended at 1.8 km. Coarser rings implicitly removed trees.
  `horizon_vegetation.rs::append` now uses independent world-aligned patch grids,
  climate/grove masks, clustered irregular crowns, mixed sizes and distance tiers
  out to 6.2 km. `ground` interpolates the actual mesh triangle to anchor them.
- `src/world/landscape.rs::visual_ecology` reuses the climate field without
  requesting erosion tiles. This is visual ecology, not a new saved generator.
- `horizon.rs` increases the lattice from 65 to 97 vertices per side and stops
  emitting outer cells beyond the complete distance fade. Geometry is still a
  surface approximation between the editable chunks, not remote volumetric data.
- `horizon_batches.rs::partition` groups triangles into index ranges off-thread.
  `DrawBatch::visible` and the horizon draw loop in `render.rs` cull camera-relative
  AABBs without per-frame allocation. Exact edited faces and water use the same
  culling path. Edit reconciliation and checkpoint identity are preserved.

### Materials and water

- `world_shader.wgsl::natural_surface`: distant terrain previously evaluated
  the complete climate/material function twice. Kind zero now shades the shared
  grass/rock mixture once. Subpixel grain/clump noise is skipped when filtered
  away; close grain snaps to a consistent 16-texel/metre grid. Heath, soil and
  mineral variation interrupt uniform turf without adding contour bands.
- `block_surface`: distant trunks previously entered the generic grass/rock
  backdrop branch. Material IDs now retain their intended shading. Far foliage
  and trunks skip close bark/leaf detail; water skips unrelated material noise.
- `horizon.rs`: the two world-sized water triangles had no depth variation and
  could not be usefully culled. Water now reuses the nested terrain lattice,
  emits only potentially wet triangles, and carries approximate bed depth.
- `world_shader.wgsl::fs_main`: four analytically filtered wave scales retain
  broad lake ripples while suppressing subpixel sparkle. Depth and broad sediment
  variation replace the uniform blue base. Cave sky visibility remains applied.
- `indirect_gather.wgsl::water_reflection`: reuse the existing half-resolution
  lighting field for at most 32 ray steps and five intersection refinements.
  Scene colour and depth supply reflected visible banks, vegetation and lights.
  Misses retain the atmospheric reflection. No extra scene render or full-size
  reflection target is allocated; gather reads scene, resolve writes scene.
- `indirect_composite.wgsl`: depth- and material-aware resolve keeps reflection
  corrections from leaking across the water/land boundary. Negative receiver
  alpha distinguishes water from the existing diffuse-lighting receivers.

### Integrated sky and lighting

The integrated build also includes the current cloud owner's wind-aligned
cell shear/twist, advected height warp, cloud-height variation and revised partly
cloudy layers (`cloud_morphology.wgsl`, `world.rs`). Existing transmittance,
multi-scattering, sky-view and aerial-perspective LUTs, volumetric cloud temporal
reconstruction, cloud shadows, cascaded sun shadows, local torch lighting and
screen-space diffuse exchange remain part of the engine.

This is not complete voxel global illumination: screen-space transport misses
off-screen/occluded sources, water has no transmission/refraction, and distant
vegetation proxies are not individually simulated trees. Sparse radiance volumes,
hardware ray queries, reflection fallback probes and an active-volume physics
budget require separate implementation and measured acceptance. No claim of
state-of-the-art or maximum efficiency is supported by compilation alone.

## Verification

Initial release ran on the NVIDIA RTX 4090 / Vulkan at 1600 × 900, Hills seed
2738958700, 16-chunk view, 2 GiB terrain cache. `target/world-detail-review/noon`
and `water` contain inspected renderer PNGs. All 3,190 requested chunks in the
noon view and 3,098 in the lake view were resident before the measured interval.
No tests or hosted CI were used.

The matched same-executable culling toggle produced identical visible land in
the inspected noon captures. Submitted triangles fell from 2,667,859 to
2,368,873: 298,986 fewer triangles per frame. Of the 463,272-triangle backdrop,
64.5% was rejected in this view. This exchanges one horizon draw for 107 bounded
index-range draws; total draws were 667 without culling and 773 with it.
The initial backdrop built in 223 ms and contained 276 spatial batches.
This proves work rejection, not a clean FPS gain: other chats had review/build
activity, so their interference disqualifies these timings as isolated benchmarks.

The first low-angle water review exposed straight periodic ripple stripes and
blue reflections under grey clouds. Revision two adds moving wind patches,
reduces fine-wave slope and uses a local overcast environment for missed rays.
Visible-bank reflections were observed; off-screen reflection remains absent.

Startup inspection also caught a runtime WGSL reserved-name error (`filter`)
that the initial build workflow did not report. Renamed to `wave_filter`; the
subsequent renderer launched successfully. `main.rs` now logs startup errors
instead of silently exiting with no diagnostic. Final captures and build receipt
are recorded below after the second visual iteration.

The revision-two water capture was inspected at water level: reflections of
shoreline trees are visible, the overcast water is less blue, and ripple stripes
are less dominant. Its compact tool panel leaves the centre of the view clear.
The revision-three sky has more connected cloud banks, but repeated horizontal
waists remain visible. This is an outstanding art issue, not accepted realism.

### Final inspected candidate

- Executable: `target/world-detail-review/voxel-world-final.exe`.
- Launch: [run-reference-rendering.cmd](C:/dev/voxel_v2_backup/run-reference-rendering.cmd).
- SHA-256: `F4FEB8300CCB710CCDEF1302AB05A05E9D2C853D938596CCF78B3268C904426D`.
- Frozen source and 120-file hash receipt: `target/world-detail-review/source`,
  `final-source-receipt.json`, `final-build-receipt.json`.
- Final optimized build: `build-reviewed-final.log`; all three final runs exited
  normally and their logs contain no startup, validation or panic errors.
- [Daylight capture](C:/dev/voxel_v2_backup/target/world-detail-review/final-noon/hills-detail/None/Hills-00-PartlyCloudy-noon-3.png)
  at `(0,66,0)`, partly cloudy noon, settled world.
- [Dusk capture](C:/dev/voxel_v2_backup/target/world-detail-review/final-dusk/hills-detail/None/Hills-00-PartlyCloudy-noon-3.png)
  faces the setting sun at solar time `0.745`. The generic filename says noon;
  the log records the actual dusk time. Warm rims and cool cloud undersides are
  visible, with a dark backlit foreground; the effect is still exaggerated.
- [Moving overcast water capture](C:/dev/voxel_v2_backup/target/world-detail-review/final-water-motion/hills-detail/None/Hills-01-Overcast-noon-3.png)
  records translation and yaw after the initial 15-second settle. Both earlier
  and later views were inspected; shoreline reflections track the changed view.
  All 3,098 requested chunks were resident at the end of this run.

The final snapshot also integrates the coordinated horizon edit fingerprint
optimization and finite BRDF/edited-face torch fixes. It deliberately retains a
coherent pre-local-mist renderer: subsequent live mist, GPU auto-budget, local
ground geometry and material-editor work belong to separately changing code and
are not represented by this receipt. Existing live work was preserved.

Remaining visual weaknesses: grass still reads as a broad noisy green surface,
distant ecology is a visual approximation, cloud waists repeat, and screen-space
reflections cannot recover scenery outside the frame. The images support a
rendering improvement, not the requested finished photorealistic or fully
simulated Noita-scale 3D world. Full all-weather 16 km sky acceptance and finer
voxel-scale art acceptance remain separate from these three inspected views.

### Dusk fill follow-up (isolated runtime inspected)

The inspected dusk foreground loses too much detail. The live
`src/shaders/world_shader.wgsl` now adds bounded cool irradiance from the zenith
and opposite-sun sky, with a small authored ambient fallback. It fades in over
sun heights -0.16 to -0.02 and out over 0.10 to 0.28; midday and deep night receive
no added term. Existing cavity, weather and sky-access attenuation still apply.
This is an artistic environment-light approximation, not voxel global illumination.

The exact shader is frozen at `target/dusk-fill-review/world_shader.wgsl`, SHA-256
`F309313FB5CB32A7BECB14E706303F8A9472924DD7C956110A2002F7C23A4674`.
It is not included in `voxel-world-final.exe` above. An isolated copy of that
accepted source plus only this fill term was built as
`target/dusk-fill-review/voxel-dusk-fill.exe`, SHA-256
`7186B3D9ED222D58E63850E49097AA6D09B66BBC51F149870BEC31FC489FCA03`.
The 16-second dusk run exited normally, with no startup/validation/panic errors;
all 3,190 requested chunks were resident. Its log confirms solar time 0.745,
camera `(0,66,0)` and mouse released.

[Inspected dusk capture](C:/dev/voxel_v2_backup/target/dusk-fill-review/captures/hills-detail/None/Hills-00-PartlyCloudy-noon-3.png):
shaded rock and canopies are modestly cooler and more readable against the warm
sky. Repeated cloud bases and simple foliage silhouettes remain obvious.
The extra term is zero at midday by construction; no new daytime capture was
performed for this isolated follow-up. Live-source integration still requires
review because other chats are concurrently changing materials, mist and layout.
This isolated run does not validate those changes or establish a speedup.

## Integrated landscape correction

Candidate: `target/reference-landscape-review/voxel-reference-landscape-v3.exe`,
SHA-256 `EF3ECB10C2E9BBBDE872B39FC8DC8279D94937720160149FCE39F5DC1266E513`.
Its frozen source, source hashes and build log are in the same directory.

### Reference targets and implemented corrections

- Bridge/daylight photo 1: retain blue openings and cloud volume with shaded
  undersides. `cloud_morphology.wgsl` now preserves the negative field outside
  the condensation foot. Scaling it toward zero had allowed positive noise to
  inflate empty space into repeated pale skirts. The integrated cloud marcher
  also includes the coordinated full-interval density sampling correction.
- Sunset/rain photos 2--6: preserve warm illuminated cloud edges, cooler bodies
  and readable dark land. The bounded dusk fill is retained, not replaced with
  a global exposure increase.
- River dusk photo 7 and aerial haze photo 8: local mist now integrates an
  exponential world-height density analytically. No shadow lookup or marching
  loop runs per terrain fragment. Terrain and sky use the same radiance and
  extinction; cave sky-access and underwater guards remain active. This is
  diffuse haze, not a volumetric light-shaft solution.
- Flooded woodland photo 9 and aerial woodland photo 10: new v11 worlds have
  separated broadleaf branch masses and irregular conifer fans. Existing v10
  saves keep their old generated shapes. Intermediate trees use the actual
  cached spawn decisions and anchors out to 720 m, bridging the former
  128--512 m tree-free belt at quarter-metre scale. Forest soils can support
  trees even when surface detail substitutes dirt, mud or gravel for grass.
- Far crowns use irregular contour meshes instead of stacked boxes. Near
  anchors have priority; far cells are ordered by a stable spatial hash before
  enforcing a 1.6-million proxy-vertex ceiling (89.6 MB). The ceiling affects
  visual proxies only; editable trees and exact edited faces remain separate.

### Regression root cause

The first richer-tree integration exposed a second issue:
`horizon_edits.rs::apply` duplicated every indexed triangle into three new
vertices whenever any edited region existed. An edited courtyard reached
roughly 344 MB of vertices against the device's 256 MiB single-buffer limit.
The corrected path preserves shared indices for unaffected triangles and clips
only polygons intersecting edited volumes. A pre-upload size guard preserves
the previous usable backdrop if an exceptional mesh still exceeds device limits.

`VOXEL_LOCAL_MIST=0` provides a same-executable comparison. The startup log
records whether analytic mist is active. Runtime acceptance for this candidate
is recorded below once its fresh captures have been inspected.

### Runtime findings and subsequent silhouette revision

V3 passed the edited courtyard reproducer: 59,171,728 vertex bytes, 756 visible
chunks, zero missing after settling, normal exit. Quarter-metre moving woodland
also passed with 69,002,192 vertex bytes and zero settled missing chunks. I
inspected its final lake view: the middle-distance woodland gap is filled.
Evidence lives under `target/woodland-budget-review`.

The v3 far-hills capture still showed three visibly stacked cones per distant
conifer. V4 replaces those with one irregular tapered crown, reducing its coarse
vertex count from 84 to 68, and lowers v11 near-conifer branches from 55% to 18%
of trunk height. Its inspected quarter-metre hills capture at 16 km has 8,856
resident chunks and continuous woodland. This improves silhouettes; the proxies
remain visibly simplified compared with the photographic forest references.

V4 dusk and overcast lake captures exited normally. The same-executable lake
comparison logged mist active/inactive and terrain GPU times of 0.226/0.231 ms.
The haze gives a modest cool veil without hiding the shoreline. However, dusk
mist runs recorded 31--58 ms terrain times versus 5.77 ms with mist off while a
separate interactive review was active. Those results do not support accepting
general mist performance. V5 removes the extra atmosphere evaluation inside
mist and reuses the already-computed terrain/sky radiance; a fresh dusk pair is
required before promoting that revision.

### V5 inspected result

Correction entry points in the working tree:

- [Preserve unaffected indexed terrain](C:/dev/voxel_v2_backup/src/render/horizon_edits.rs:89).
- [Bound vegetation geometry and reuse actual tree anchors](C:/dev/voxel_v2_backup/src/render/horizon_vegetation.rs:30).
- [Lower conifer branch line](C:/dev/voxel_v2_backup/src/world/trees.rs:81) and
  [open broadleaf crown](C:/dev/voxel_v2_backup/src/world/trees.rs:172).
- [Preserve negative cloud support outside its base](C:/dev/voxel_v2_backup/src/shaders/cloud_morphology.wgsl:125).
- [Analytic mist with caller-supplied radiance](C:/dev/voxel_v2_backup/src/shaders/local_mist_shared.wgsl:49).

The frozen rendering candidate remains at
`target/reference-landscape-review/voxel-reference-landscape-v5.exe`.
Its SHA256 is `957EBDF08CCC791B7257F81F629603DA648AB690A916A8620400C276262E6C89`.
This is the reviewed landscape snapshot, not the concurrently evolving full-game
integration. The earlier v3 recovery executable remains available.

Two fresh 24-second runs of that same executable used the same hills camera,
1280 x 720 resolution, partly cloudy weather and solar time 0.745. Both exited
normally, reported 252 visible chunks with zero missing after settling, and
logged the expected mist enabled/disabled state. Process inventories showed
only the relevant review instance during these runs.

| Dusk GPU pass | Mist enabled | Mist disabled |
| --- | ---: | ---: |
| Terrain | 2.773 ms | 0.425 ms |
| Indirect lighting | 0.017 ms | 0.018 ms |
| Cloud ray march | 5.134 ms | 5.121 ms |
| All measured passes | 7.953 ms | 5.593 ms |

The extra atmosphere evaluation was a substantial regression. Reusing radiance
removes the earlier 31--58 ms terrain-pass outcome, but the remaining 2.35 ms
mist cost is material and is not a claim of optimal rendering. CPU frame-rate
comparisons are unsuitable here because another integration build was running.
These are short stationary rendering samples with physics disabled, not travel
or full-game performance acceptance.

Both actual dusk images were opened and inspected:
[mist enabled](C:/dev/voxel_v2_backup/target/reference-landscape-review/v5-dusk-mist/captures/hills-detail/None/Hills-01-PartlyCloudy-noon-1.png),
[mist disabled](C:/dev/voxel_v2_backup/target/reference-landscape-review/v5-dusk-no-mist/captures/hills-detail/None/Hills-01-PartlyCloudy-noon-1.png).
Logs are `target/reference-landscape-review/v5-dusk-mist.log` and
`v5-dusk-no-mist.log`; exact source hashes are in `v5-source-receipt.json`.

Against `20260831_194545.jpg` and `20260730_045700.jpg`, the inspected sky has
warm rims, cool cloud bodies and an open warm horizon without a discrete mist
stripe. Against the aerial river and DJI woodland references, forest coverage
is continuous with varied height, but broadleaf proxies remain too round and
coarse. The conifer daylight inspection has single tapered crowns instead of
three stacked cones; close rock terraces and the distant silhouette transition
remain obvious. Cloud spacing and bright rims also remain more regular than
the photographs. Those limitations are retained explicitly rather than treating
this regression repair as finished photographic quality.

### Latest combined build: saved-scene terrain correction

[run-reference-landscape.cmd](C:/dev/voxel_v2_backup/run-reference-landscape.cmd)
now launches the combined `target/integrated-world-review/voxel-lighting-repaired.exe`,
SHA256 `BD8C704FF4E31B743F1C328B43D79D23550A9BC8EC4CE64D665AF3BDD75660A0`.
Its frozen source and receipt are `stripe-source` and `stripe-source-manifest.json`
in that review directory. The landscape v5 snapshot is retained as a fallback.

The engine integration review reproduced an additional terrain regression in
the user's copied saved scene. Disabling indirect lighting removed narrow
horizontal bands while disabling sun shadows did not remove the large dark
foreground shapes. Inspection of the corrected night and day images confirms
the narrow bands are gone with indirect lighting retained. The daylight image
identifies the large dark shapes as water; that water colour remains a visual
limitation, not a fixed foliage shadow.

The repair snaps depth and reconstructed camera rays to the same full-resolution
pixel centre in `src/shaders/indirect_shared.wgsl::view_position`. Gather and
bilateral resolve were corrected together in `indirect_gather.wgsl` and
`indirect_composite.wgsl`. These integration changes were made by the engine
chat after the saved-scene A/B findings were shared.

I opened and inspected both actual images:
[corrected night](C:/dev/voxel_v2_backup/target/integrated-world-review/night-repaired/saved-world.png)
and [daylight](C:/dev/voxel_v2_backup/target/integrated-world-review/day-repaired/saved-world.png).
Both logs settle at 3,069 visible/resident chunks with no pending streaming work.
This review launcher disables the extra local mist by default after the measured cost;
the main atmospheric aerial perspective remains active. `VOXEL_LOCAL_MIST=1`
retains the analytic option for further optimization. No claim is made that the
saved-scene view validates every biome or all moving-camera transitions.

The horizon review then ran that same `voxel-lighting-repaired.exe` for a
32-second quarter-metre dusk camera sweep at 1280 x 720, 16 view chunks.
I opened its final
[landscape capture](C:/dev/voxel_v2_backup/target/combined-engine-review/dusk/captures/hills-detail/None/Hills-01-PartlyCloudy-noon-3.png).
The newer integrated cloud height warp is included in this image. The broad
pale bases are removed and woodland coverage is continuous, but rounded crowns,
strong uniform rims and close terrain contouring remain visibly stylized.
The measured sample was 65.85 FPS average, 23.452 ms p99 frame time and 10.977 ms
summed GPU passes on the RTX 4090. Physics was disabled. At 32 seconds the stream
still reported 174 missing chunks; this is evidence of the rendering result,
not a clean streaming-performance acceptance. Logs remain under
`target/combined-engine-review/dusk`. No tests or hosted CI were added or run.

The capture owner confirmed the frame capture gate waited for ready views;
the 174 missing chunks belong to the later moving-camera state. The edited
fixture also settled at 769 visible chunks with zero missing. Fast quarter-metre
travel with physics still accumulated a backlog. A subsequent engine build is
being prepared for that issue; the launcher above intentionally identifies the
reviewed BD8C704 candidate, not an unvalidated newer executable.
